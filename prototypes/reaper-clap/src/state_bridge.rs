//! Prototype CLAP entry shim around the pinned nice-plug wrapper.
//!
//! The upstream state loader trusts an unbounded length prefix and does not
//! notify CLAP hosts when parameter values change on load. This entry keeps
//! all other extensions and processing in nice-plug and intercepts state only.
use super::{Proof, VOICES};
use clap_sys::{
    entry::clap_plugin_entry,
    ext::{
        params::{clap_host_params, CLAP_EXT_PARAMS, CLAP_PARAM_RESCAN_VALUES},
        state::{clap_plugin_state, CLAP_EXT_STATE},
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    plugin::{clap_plugin, clap_plugin_descriptor},
    stream::{clap_istream, clap_ostream},
    version::CLAP_VERSION,
};
use kabl_core::PatchState;
use kabl_engine::compile::compile;
use nice_plug::{
    plugin::PluginState,
    wrapper::{clap::{PluginDescriptor, Wrapper}, setup_logger},
};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;
type GetExtension = unsafe extern "C" fn(*const clap_plugin, *const c_char) -> *const c_void;
type Destroy = unsafe extern "C" fn(*const clap_plugin);

#[derive(Clone, Copy)]
struct Bridge {
    get_extension: GetExtension,
    destroy: Destroy,
    host: usize,
}
static BRIDGES: OnceLock<Mutex<HashMap<usize, Bridge>>> = OnceLock::new();
static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

fn bridges() -> &'static Mutex<HashMap<usize, Bridge>> {
    BRIDGES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn bridge(plugin: *const clap_plugin) -> Option<Bridge> {
    bridges().lock().unwrap_or_else(|e| e.into_inner()).get(&(plugin as usize)).copied()
}
fn descriptor() -> &'static PluginDescriptor {
    DESCRIPTOR.get_or_init(PluginDescriptor::for_plugin::<Proof>)
}

unsafe extern "C" fn count(_: *const clap_plugin_factory) -> u32 { 1 }
unsafe extern "C" fn describe(_: *const clap_plugin_factory, index: u32) -> *const clap_plugin_descriptor {
    if index == 0 { descriptor().clap_plugin_descriptor() } else { ptr::null() }
}
unsafe extern "C" fn create(_: *const clap_plugin_factory, host: *const clap_host, id: *const c_char) -> *const clap_plugin {
    if host.is_null() || id.is_null() || unsafe { CStr::from_ptr(id) } != descriptor().clap_id() {
        return ptr::null();
    }
    let wrapper = unsafe { Wrapper::<Proof>::new(host) };
    let mut plugin = wrapper.clap_plugin.borrow_mut();
    let (Some(get_extension), Some(destroy)) = (plugin.get_extension, plugin.destroy) else { return ptr::null(); };
    plugin.get_extension = Some(get_extension_bridge);
    plugin.destroy = Some(destroy_bridge);
    let ptr = &*plugin as *const clap_plugin;
    bridges().lock().unwrap_or_else(|e| e.into_inner()).insert(ptr as usize, Bridge { get_extension, destroy, host: host as usize });
    drop(plugin);
    let _ = Arc::into_raw(wrapper); // Released by nice-plug's original destroy callback.
    ptr
}
unsafe extern "C" fn get_extension_bridge(plugin: *const clap_plugin, id: *const c_char) -> *const c_void {
    if plugin.is_null() || id.is_null() { return ptr::null(); }
    let Some(b) = bridge(plugin) else { return ptr::null(); };
    if unsafe { CStr::from_ptr(id) } == CLAP_EXT_STATE {
        &STATE as *const _ as *const c_void
    } else {
        unsafe { (b.get_extension)(plugin, id) }
    }
}
unsafe extern "C" fn destroy_bridge(plugin: *const clap_plugin) {
    if let Some(b) = bridges().lock().unwrap_or_else(|e| e.into_inner()).remove(&(plugin as usize)) {
        unsafe { (b.destroy)(plugin) };
    }
}
fn original_state(plugin: *const clap_plugin) -> Option<&'static clap_plugin_state> {
    let b = bridge(plugin)?;
    let ext = unsafe { (b.get_extension)(plugin, CLAP_EXT_STATE.as_ptr()) } as *const clap_plugin_state;
    unsafe { ext.as_ref() }
}
unsafe extern "C" fn save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let Some(save) = original_state(plugin).and_then(|s| s.save) else { return false; };
    unsafe { save(plugin, stream) }
}

// CLAP streams may read in short chunks. Enforce the cap before allocating the
// JSON buffer, and reject an invalid stream before nice-plug mutates any state.
unsafe fn read_exact(stream: *const clap_istream, dst: &mut [u8]) -> bool {
    let Some(read) = (unsafe { stream.as_ref() }).and_then(|s| s.read) else { return false; };
    let mut offset = 0;
    while offset < dst.len() {
        let n = unsafe { read(stream, dst.as_mut_ptr().add(offset).cast(), (dst.len() - offset) as u64) };
        if n <= 0 || n as usize > dst.len() - offset { return false; }
        offset += n as usize;
    }
    true
}
fn validate_state(data: &[u8]) -> bool {
    let Ok(state) = serde_json::from_slice::<PluginState>(data) else { return false; };
    let Some(encoded_patch) = state.fields.get("patch_v1") else { return false; };
    let Ok(patch) = serde_json::from_str::<PatchState>(encoded_patch) else { return false; };
    // Lifecycle preflight; activate still compiles at the active sample rate.
    compile(&patch, 48000.0, VOICES).is_ok()
}
struct Reader<'a> { data: &'a [u8], offset: usize }
unsafe extern "C" fn memory_read(stream: *const clap_istream, dst: *mut c_void, size: u64) -> i64 {
    let Some(r) = (unsafe { stream.as_ref() }).and_then(|s| unsafe { (s.ctx as *mut Reader<'_>).as_mut() }) else { return -1; };
    if dst.is_null() && size != 0 { return -1; }
    let n = r.data.len().saturating_sub(r.offset).min(usize::try_from(size).unwrap_or(usize::MAX));
    if n > 0 { unsafe { ptr::copy_nonoverlapping(r.data.as_ptr().add(r.offset), dst.cast::<u8>(), n) }; }
    r.offset += n;
    n as i64
}
fn rescan_params(plugin: *const clap_plugin) {
    let Some(b) = bridge(plugin) else { return; };
    let host = b.host as *const clap_host;
    let Some(get_extension) = (unsafe { host.as_ref() }).and_then(|h| h.get_extension) else { return; };
    let ext = unsafe { get_extension(host, CLAP_EXT_PARAMS.as_ptr()) } as *const clap_host_params;
    if let Some(rescan) = (unsafe { ext.as_ref() }).and_then(|p| p.rescan) {
        unsafe { rescan(host, CLAP_PARAM_RESCAN_VALUES) };
    }
}
unsafe extern "C" fn load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    if plugin.is_null() || stream.is_null() { return false; }
    let Some(load) = original_state(plugin).and_then(|s| s.load) else { return false; };
    let mut header = [0u8; 8];
    if !unsafe { read_exact(stream, &mut header) } { return false; }
    let Ok(length) = usize::try_from(u64::from_le_bytes(header)) else { return false; };
    if length == 0 || length > MAX_STATE_BYTES { return false; }
    let mut data = vec![0u8; length];
    if !unsafe { read_exact(stream, &mut data) } || !validate_state(&data) { return false; }
    let mut replay = Vec::with_capacity(8 + length);
    replay.extend_from_slice(&header);
    replay.extend_from_slice(&data);
    let mut reader = Reader { data: &replay, offset: 0 };
    let replay_stream = clap_istream { ctx: (&mut reader as *mut Reader<'_>).cast(), read: Some(memory_read) };
    let success = unsafe { load(plugin, &replay_stream) };
    if success { rescan_params(plugin); }
    success
}
static STATE: clap_plugin_state = clap_plugin_state { save: Some(save), load: Some(load) };
static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(count), get_plugin_descriptor: Some(describe), create_plugin: Some(create),
};
unsafe extern "C" fn init(_: *const c_char) -> bool { setup_logger::<Proof>(); true }
unsafe extern "C" fn deinit() {}
unsafe extern "C" fn get_factory(id: *const c_char) -> *const c_void {
    if !id.is_null() && unsafe { CStr::from_ptr(id) } == CLAP_PLUGIN_FACTORY_ID {
        &FACTORY as *const _ as *const c_void
    } else { ptr::null() }
}
#[unsafe(no_mangle)]
#[used]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION, init: Some(init), deinit: Some(deinit), get_factory: Some(get_factory),
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_or_invalid_patch_state_is_rejected_before_framework_load() {
        assert!(!validate_state(b"random"));
        let mut state = PluginState { version: "0.1.0".into(), params: Default::default(), fields: Default::default() };
        assert!(!validate_state(&serde_json::to_vec(&state).unwrap()));
        let mut patch = kabl_standalone::default_patch();
        state.fields.insert("patch_v1".into(), serde_json::to_string(&patch).unwrap());
        assert!(validate_state(&serde_json::to_vec(&state).unwrap()));
        patch.modules.get_mut(&2).unwrap().kind = "invalid.kind".into();
        state.fields.insert("patch_v1".into(), serde_json::to_string(&patch).unwrap());
        assert!(!validate_state(&serde_json::to_vec(&state).unwrap()));
    }
}
