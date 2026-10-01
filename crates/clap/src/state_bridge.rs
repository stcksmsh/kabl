//! CLAP entry: production transactional state and MIDI-only dialect. The pinned wrapper
//! owns GUI/lifecycle/parameter events; its non-atomic state loader is never used.
use super::{Instrument, Shared};
use crate::sound_state::{SoundState, MAX_STATE_BYTES};
use std::cell::RefCell;
use std::sync::Weak;
thread_local! { static CAPTURE: RefCell<Option<Weak<Shared>>> = const { RefCell::new(None) }; }
pub(super) fn capture(shared: &Arc<Shared>) {
    CAPTURE.with(|c| *c.borrow_mut() = Some(Arc::downgrade(shared)));
}
use clap_sys::{
    entry::clap_plugin_entry,
    ext::{
        note_ports::{
            clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS,
            CLAP_NOTE_DIALECT_MIDI,
        },
        params::{clap_host_params, CLAP_EXT_PARAMS, CLAP_PARAM_RESCAN_VALUES},
        state::{clap_host_state, clap_plugin_state, CLAP_EXT_STATE},
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    plugin::{clap_plugin, clap_plugin_descriptor},
    stream::{clap_istream, clap_ostream},
    version::CLAP_VERSION,
};
use nice_plug::wrapper::{
    clap::{PluginDescriptor, Wrapper},
    setup_logger,
};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

type GetExtension = unsafe extern "C" fn(*const clap_plugin, *const c_char) -> *const c_void;
type Destroy = unsafe extern "C" fn(*const clap_plugin);

#[derive(Clone)]
struct Bridge {
    get_extension: GetExtension,
    destroy: Destroy,
    host: usize,
    shared: Arc<Shared>,
    on_main: unsafe extern "C" fn(*const clap_plugin),
}
static BRIDGES: OnceLock<Mutex<HashMap<usize, Bridge>>> = OnceLock::new();
static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

fn bridges() -> &'static Mutex<HashMap<usize, Bridge>> {
    BRIDGES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn bridge(plugin: *const clap_plugin) -> Option<Bridge> {
    bridges()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(plugin as usize))
        .cloned()
}
fn descriptor() -> &'static PluginDescriptor {
    DESCRIPTOR.get_or_init(PluginDescriptor::for_plugin::<Instrument>)
}

unsafe extern "C" fn count(_: *const clap_plugin_factory) -> u32 {
    1
}
unsafe extern "C" fn describe(
    _: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 {
        descriptor().clap_plugin_descriptor()
    } else {
        ptr::null()
    }
}
unsafe extern "C" fn create(
    _: *const clap_plugin_factory,
    host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    if host.is_null() || id.is_null() || unsafe { CStr::from_ptr(id) } != descriptor().clap_id() {
        return ptr::null();
    }
    let wrapper = unsafe { Wrapper::<Instrument>::new(host) };
    let mut plugin = wrapper.clap_plugin.borrow_mut();
    let (Some(get_extension), Some(destroy)) = (plugin.get_extension, plugin.destroy) else {
        return ptr::null();
    };
    let shared = CAPTURE
        .with(|c| c.borrow_mut().take())
        .and_then(|s| s.upgrade())
        .expect("instance capture");
    shared
        .host
        .store(host as u64, std::sync::atomic::Ordering::Release);
    let on_main = plugin.on_main_thread.expect("main callback");
    plugin.on_main_thread = Some(on_main_bridge);
    plugin.get_extension = Some(get_extension_bridge);
    plugin.destroy = Some(destroy_bridge);
    let ptr = &*plugin as *const clap_plugin;
    bridges().lock().unwrap_or_else(|e| e.into_inner()).insert(
        ptr as usize,
        Bridge {
            get_extension,
            destroy,
            host: host as usize,
            shared,
            on_main,
        },
    );
    drop(plugin);
    let _ = Arc::into_raw(wrapper); // Released by nice-plug's original destroy callback.
    ptr
}
unsafe extern "C" fn get_extension_bridge(
    plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if plugin.is_null() || id.is_null() {
        return ptr::null();
    }
    let Some(b) = bridge(plugin) else {
        return ptr::null();
    };
    if unsafe { CStr::from_ptr(id) } == CLAP_EXT_STATE {
        &STATE as *const _ as *const c_void
    } else if unsafe { CStr::from_ptr(id) } == CLAP_EXT_NOTE_PORTS {
        &NOTE_PORTS as *const _ as *const c_void
    } else {
        unsafe { (b.get_extension)(plugin, id) }
    }
}
unsafe extern "C" fn destroy_bridge(plugin: *const clap_plugin) {
    if let Some(b) = bridges()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&(plugin as usize))
    {
        unsafe { (b.destroy)(plugin) };
    }
}
unsafe extern "C" fn save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let Some(b) = bridge(plugin) else {
        return false;
    };
    let state = b.shared.snapshot();
    let Ok(data) = serde_json::to_vec(&state) else {
        return false;
    };
    if data.len() > MAX_STATE_BYTES {
        return false;
    }
    let Some(write) = (unsafe { stream.as_ref() }).and_then(|s| s.write) else {
        return false;
    };
    let header = (data.len() as u64).to_le_bytes();
    for bytes in [&header[..], &data[..]] {
        let mut offset = 0;
        while offset < bytes.len() {
            let n = unsafe {
                write(
                    stream,
                    bytes.as_ptr().add(offset).cast(),
                    (bytes.len() - offset) as u64,
                )
            };
            if n <= 0 || n as usize > bytes.len() - offset {
                return false;
            }
            offset += n as usize;
        }
    }
    true
}

// CLAP streams may read in short chunks. Enforce the cap before allocating the
// JSON buffer, and reject an invalid stream before nice-plug mutates any state.
unsafe fn read_exact(stream: *const clap_istream, dst: &mut [u8]) -> bool {
    let Some(read) = (unsafe { stream.as_ref() }).and_then(|s| s.read) else {
        return false;
    };
    let mut offset = 0;
    while offset < dst.len() {
        let n = unsafe {
            read(
                stream,
                dst.as_mut_ptr().add(offset).cast(),
                (dst.len() - offset) as u64,
            )
        };
        if n <= 0 || n as usize > dst.len() - offset {
            return false;
        }
        offset += n as usize;
    }
    true
}
fn rescan_params(plugin: *const clap_plugin) {
    let Some(b) = bridge(plugin) else {
        return;
    };
    let host = b.host as *const clap_host;
    let Some(get_extension) = (unsafe { host.as_ref() }).and_then(|h| h.get_extension) else {
        return;
    };
    let ext = unsafe { get_extension(host, CLAP_EXT_PARAMS.as_ptr()) } as *const clap_host_params;
    if let Some(rescan) = (unsafe { ext.as_ref() }).and_then(|p| p.rescan) {
        unsafe { rescan(host, CLAP_PARAM_RESCAN_VALUES) };
    }
}
unsafe extern "C" fn load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    let Some(b) = bridge(plugin) else {
        return false;
    };
    let mut header = [0u8; 8];
    if !unsafe { read_exact(stream, &mut header) } {
        return false;
    }
    let length = u64::from_le_bytes(header);
    if length == 0 || length > MAX_STATE_BYTES as u64 {
        return false;
    }
    let mut data = vec![0u8; length as usize];
    if !unsafe { read_exact(stream, &mut data) } {
        return false;
    }
    let Ok(state) = SoundState::decode(&data) else {
        return false;
    };
    if b.shared.load(state).is_err() {
        return false;
    }
    rescan_params(plugin);
    true
}

pub(super) fn request_main(shared: &Shared) {
    let host = shared.host.load(std::sync::atomic::Ordering::Acquire) as *const clap_host;
    if let Some(callback) = (unsafe { host.as_ref() }).and_then(|h| h.request_callback) {
        unsafe { callback(host) };
    }
}
unsafe extern "C" fn on_main_bridge(plugin: *const clap_plugin) {
    let Some(b) = bridge(plugin) else {
        return;
    };
    unsafe { (b.on_main)(plugin) };
    if b.shared
        .dirty
        .swap(false, std::sync::atomic::Ordering::AcqRel)
    {
        let host = b.host as *const clap_host;
        let Some(get) = (unsafe { host.as_ref() }).and_then(|h| h.get_extension) else {
            return;
        };
        let state = unsafe { get(host, CLAP_EXT_STATE.as_ptr()) } as *const clap_host_state;
        if let Some(mark) = (unsafe { state.as_ref() }).and_then(|s| s.mark_dirty) {
            unsafe { mark(host) };
        }
    }
}
unsafe extern "C" fn note_count(_: *const clap_plugin, input: bool) -> u32 {
    u32::from(input)
}
unsafe extern "C" fn note_get(
    _: *const clap_plugin,
    index: u32,
    input: bool,
    info: *mut clap_note_port_info,
) -> bool {
    if index != 0 || !input || info.is_null() {
        return false;
    }
    let mut port: clap_note_port_info = unsafe { std::mem::zeroed() };
    port.supported_dialects = CLAP_NOTE_DIALECT_MIDI;
    port.preferred_dialect = CLAP_NOTE_DIALECT_MIDI;
    for (to, from) in port.name.iter_mut().zip(b"MIDI input") {
        *to = *from as c_char;
    }
    unsafe {
        *info = port;
    }
    true
}
static NOTE_PORTS: clap_plugin_note_ports = clap_plugin_note_ports {
    count: Some(note_count),
    get: Some(note_get),
};
static STATE: clap_plugin_state = clap_plugin_state {
    save: Some(save),
    load: Some(load),
};
static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(count),
    get_plugin_descriptor: Some(describe),
    create_plugin: Some(create),
};
unsafe extern "C" fn init(_: *const c_char) -> bool {
    setup_logger::<Instrument>();
    true
}
unsafe extern "C" fn deinit() {}
unsafe extern "C" fn get_factory(id: *const c_char) -> *const c_void {
    if !id.is_null() && unsafe { CStr::from_ptr(id) } == CLAP_PLUGIN_FACTORY_ID {
        &FACTORY as *const _ as *const c_void
    } else {
        ptr::null()
    }
}
#[unsafe(no_mangle)]
#[used]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(init),
    deinit: Some(deinit),
    get_factory: Some(get_factory),
};
