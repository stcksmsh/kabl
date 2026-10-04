//! CLAP entry: production transactional state and MIDI-only dialect. The pinned wrapper
//! owns GUI/lifecycle/parameter events; its non-atomic state loader is never used.
use super::{Instrument, Shared};
use crate::sound_state::{SoundState, MAX_STATE_BYTES};
use std::cell::{RefCell, UnsafeCell};
use std::sync::Weak;
type Capture = (Weak<Shared>, rtrb::Producer<(u32, [u8; 3])>);
thread_local! { static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) }; }
pub(super) fn capture(shared: &Arc<Shared>, midi: rtrb::Producer<(u32, [u8; 3])>) {
    CAPTURE.with(|c| *c.borrow_mut() = Some((Arc::downgrade(shared), midi)));
}
use clap_sys::{
    entry::clap_plugin_entry,
    events::*,
    ext::{
        note_ports::{
            clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS,
            CLAP_NOTE_DIALECT_MIDI,
        },
        params::{
            clap_host_params, clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS,
            CLAP_PARAM_RESCAN_VALUES,
        },
        state::{clap_host_state, clap_plugin_state, CLAP_EXT_STATE},
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    plugin::{clap_plugin, clap_plugin_descriptor},
    process::{clap_process, clap_process_status, CLAP_PROCESS_ERROR},
    stream::{clap_istream, clap_ostream},
    version::CLAP_VERSION,
};
use nice_plug::wrapper::{
    clap::{PluginDescriptor, Wrapper},
    setup_logger,
};
use std::{
    ffi::{c_char, c_void, CStr},
    ptr,
    sync::{Arc, OnceLock},
};

type GetExtension = unsafe extern "C" fn(*const clap_plugin, *const c_char) -> *const c_void;
type Destroy = unsafe extern "C" fn(*const clap_plugin);

struct Bridge {
    get_extension: GetExtension,
    destroy: Destroy,
    host: usize,
    shared: Arc<Shared>,
    on_main: unsafe extern "C" fn(*const clap_plugin),
}
// The first field is the host-facing CLAP object. Delegated wrapper callbacks continue to
// read its unchanged plugin_data. No global lookup, mutex or Arc clone enters process().
#[repr(C)]
struct Outer {
    plugin: clap_plugin,
    bridge: Bridge,
    process: unsafe extern "C" fn(*const clap_plugin, *const clap_process) -> clap_process_status,
    midi: UnsafeCell<rtrb::Producer<(u32, [u8; 3])>>,
    gain_id: u32,
}
static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();
fn bridge<'a>(plugin: *const clap_plugin) -> Option<&'a Bridge> {
    if plugin.is_null() {
        None
    } else {
        Some(unsafe { &(*(plugin.cast::<Outer>())).bridge })
    }
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
    let original = *wrapper.clap_plugin.borrow();
    let (Some(get_extension), Some(destroy), Some(process)) =
        (original.get_extension, original.destroy, original.process)
    else {
        return ptr::null();
    };
    let (shared, midi) = CAPTURE
        .with(|c| c.borrow_mut().take())
        .expect("instance capture");
    let shared = shared.upgrade().expect("live instance");
    shared
        .host
        .store(host as u64, std::sync::atomic::Ordering::Release);
    let parameters = unsafe {
        &*(get_extension(&original, CLAP_EXT_PARAMS.as_ptr()).cast::<clap_plugin_params>())
    };
    let mut gain: clap_param_info = unsafe { std::mem::zeroed() };
    if !unsafe { parameters.get_info.expect("gain info")(&original, 0, &mut gain) } {
        return ptr::null();
    }
    let mut outer = Box::new(Outer {
        plugin: original,
        bridge: Bridge {
            get_extension,
            destroy,
            host: host as usize,
            shared,
            on_main: original.on_main_thread.expect("main callback"),
        },
        process,
        midi: UnsafeCell::new(midi),
        gain_id: gain.id,
    });
    outer.plugin.on_main_thread = Some(on_main_bridge);
    outer.plugin.get_extension = Some(get_extension_bridge);
    outer.plugin.destroy = Some(destroy_bridge);
    outer.plugin.process = Some(process_bridge);
    let _ = Arc::into_raw(wrapper); // Released by the delegated destroy callback.
    Box::into_raw(outer).cast()
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
    if !plugin.is_null() {
        let outer = unsafe { Box::from_raw(plugin.cast_mut().cast::<Outer>()) };
        unsafe { (outer.bridge.destroy)(plugin) };
        drop(outer);
    }
}

const MAX_HOST_EVENTS: u32 = 2048;
const MAX_PARAMS: usize = 128;
struct ParamEvents {
    events: [*const clap_event_header; MAX_PARAMS],
    len: usize,
}
impl ParamEvents {
    unsafe fn push(&mut self, event: *const clap_event_header) {
        // Preserve ordinary automation. On saturation retain the final value and modulation
        // separately, in timestamp order, so the single host control always converges.
        if self.len < MAX_PARAMS - 2 {
            self.events[self.len] = event;
            self.len += 1;
            return;
        }
        let kind = unsafe { (*event).type_ };
        let index = (MAX_PARAMS - 2..self.len)
            .find(|&index| unsafe { (*self.events[index]).type_ == kind });
        if let Some(index) = index {
            self.events[index] = event;
        } else {
            self.events[self.len] = event;
            self.len += 1;
        }
        if self.len == MAX_PARAMS
            && unsafe { (*self.events[MAX_PARAMS - 2]).time > (*self.events[MAX_PARAMS - 1]).time }
        {
            self.events.swap(MAX_PARAMS - 2, MAX_PARAMS - 1);
        }
    }
}
unsafe extern "C" fn filtered_size(list: *const clap_input_events) -> u32 {
    unsafe { (&*((*list).ctx.cast::<ParamEvents>())).len as u32 }
}
unsafe extern "C" fn filtered_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    let list = unsafe { &*((*list).ctx.cast::<ParamEvents>()) };
    if (index as usize) < list.len {
        list.events[index as usize]
    } else {
        ptr::null()
    }
}
unsafe extern "C" fn process_bridge(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    if plugin.is_null() || process.is_null() {
        return CLAP_PROCESS_ERROR;
    }
    let outer = unsafe { &*plugin.cast::<Outer>() };
    let input = unsafe { &*process };
    let shared = &outer.bridge.shared;
    shared
        .host_frames
        .store(input.frames_count, std::sync::atomic::Ordering::Relaxed);
    let mut parameters = ParamEvents {
        events: [ptr::null(); MAX_PARAMS],
        len: 0,
    };
    let mut overflow = false;
    if let Some(events) = unsafe { input.in_events.as_ref() } {
        if let (Some(size), Some(get)) = (events.size, events.get) {
            let count = unsafe { size(events) };
            overflow = count > MAX_HOST_EVENTS;
            // CLAP serializes process calls for one instance; only audio owns this producer.
            let midi = unsafe { &mut *outer.midi.get() };
            for index in 0..count.min(MAX_HOST_EVENTS) {
                let event = unsafe { get(events, index) };
                let Some(header) = (unsafe { event.as_ref() }) else {
                    continue;
                };
                if header.space_id != CLAP_CORE_EVENT_SPACE_ID {
                    continue;
                }
                match header.type_ {
                    CLAP_EVENT_MIDI
                        if header.size as usize >= std::mem::size_of::<clap_event_midi>() =>
                    {
                        let event = unsafe { &*event.cast::<clap_event_midi>() };
                        if event.port_index == 0 && midi.push((header.time, event.data)).is_err() {
                            overflow = true;
                        }
                    }
                    CLAP_EVENT_PARAM_VALUE | CLAP_EVENT_PARAM_MOD => {
                        let minimum = if header.type_ == CLAP_EVENT_PARAM_VALUE {
                            std::mem::size_of::<clap_event_param_value>()
                        } else {
                            std::mem::size_of::<clap_event_param_mod>()
                        };
                        if header.size as usize >= minimum && header.time < input.frames_count {
                            let id = if header.type_ == CLAP_EVENT_PARAM_VALUE {
                                unsafe { (*event.cast::<clap_event_param_value>()).param_id }
                            } else {
                                unsafe { (*event.cast::<clap_event_param_mod>()).param_id }
                            };
                            if id == outer.gain_id {
                                unsafe {
                                    parameters.push(event);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    shared
        .midi_overflow
        .store(overflow, std::sync::atomic::Ordering::Relaxed);
    let filtered = clap_input_events {
        ctx: (&mut parameters as *mut ParamEvents).cast(),
        size: Some(filtered_size),
        get: Some(filtered_get),
    };
    let mut bounded = *input;
    bounded.in_events = &filtered;
    unsafe { (outer.process)(plugin, &bounded) }
}
unsafe extern "C" fn save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let Some(b) = bridge(plugin) else {
        return false;
    };
    let state = b.shared.snapshot();
    if state.validate().is_err() {
        return false;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use assert_no_alloc::assert_no_alloc;
    use clap_sys::audio_buffer::clap_audio_buffer;
    use clap_sys::ext::params::{clap_param_info, clap_plugin_params};
    use std::sync::atomic::Ordering;

    unsafe extern "C" fn event_size(list: *const clap_input_events) -> u32 {
        unsafe { (&*((*list).ctx.cast::<Vec<clap_event_midi>>())).len() as u32 }
    }
    unsafe extern "C" fn event_get(
        list: *const clap_input_events,
        index: u32,
    ) -> *const clap_event_header {
        unsafe { &(&*((*list).ctx.cast::<Vec<clap_event_midi>>()))[index as usize].header }
    }
    unsafe extern "C" fn pointer_size(list: *const clap_input_events) -> u32 {
        unsafe { (&*((*list).ctx.cast::<Vec<*const clap_event_header>>())).len() as u32 }
    }
    unsafe extern "C" fn pointer_get(
        list: *const clap_input_events,
        index: u32,
    ) -> *const clap_event_header {
        unsafe { (&*((*list).ctx.cast::<Vec<*const clap_event_header>>()))[index as usize] }
    }
    unsafe extern "C" fn host_extension(_: *const clap_host, _: *const c_char) -> *const c_void {
        ptr::null()
    }
    unsafe extern "C" fn host_request(_: *const clap_host) {}
    #[test]
    fn exported_callback_is_bounded_under_raw_midi_flood_and_lifecycle_reset() {
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: ptr::null_mut(),
            name: c"test host".as_ptr(),
            vendor: c"kabl".as_ptr(),
            url: c"".as_ptr(),
            version: c"1".as_ptr(),
            get_extension: Some(host_extension),
            request_restart: Some(host_request),
            request_process: Some(host_request),
            request_callback: Some(host_request),
        };
        unsafe {
            let plugin = create(&FACTORY, &host, descriptor().clap_id().as_ptr());
            assert!(!plugin.is_null());
            let p = &*plugin;
            assert!(p.init.unwrap()(plugin));
            let shared = &(*plugin.cast::<Outer>()).bridge.shared;
            shared.stop.store(true, Ordering::Release);
            assert!(p.activate.unwrap()(plugin, 48000.0, 1, 4096));
            assert!(p.start_processing.unwrap()(plugin));
            let params = &*(p.get_extension.unwrap()(plugin, CLAP_EXT_PARAMS.as_ptr())
                .cast::<clap_plugin_params>());
            let mut info: clap_param_info = std::mem::zeroed();
            assert!(params.get_info.unwrap()(plugin, 0, &mut info));
            assert_eq!(params.count.unwrap()(plugin), 1);
            let mut value = 0.0;
            assert!(params.get_value.unwrap()(plugin, info.id, &mut value));
            assert_eq!(value, 1.0);
            let mut events: Vec<_> = (0..4097)
                .map(|index| clap_event_midi {
                    header: clap_event_header {
                        size: std::mem::size_of::<clap_event_midi>() as u32,
                        time: index.min(255),
                        space_id: CLAP_CORE_EVENT_SPACE_ID,
                        type_: CLAP_EVENT_MIDI,
                        flags: 0,
                    },
                    port_index: 0,
                    data: if index == 4096 {
                        [0x80, 60, 0]
                    } else {
                        [0x90, 60, 100]
                    },
                })
                .collect();
            let input = clap_input_events {
                ctx: (&mut events as *mut Vec<clap_event_midi>).cast(),
                size: Some(event_size),
                get: Some(event_get),
            };
            let mut left = [0.0f32; 4096];
            let mut right = [0.0f32; 4096];
            let mut channels = [left.as_mut_ptr(), right.as_mut_ptr()];
            let mut output = clap_audio_buffer {
                data32: channels.as_mut_ptr(),
                data64: ptr::null_mut(),
                channel_count: 2,
                latency: 0,
                constant_mask: 0,
            };
            let mut process = clap_process {
                steady_time: -1,
                frames_count: 256,
                transport: ptr::null(),
                audio_inputs: ptr::null(),
                audio_outputs: &mut output,
                audio_inputs_count: 0,
                audio_outputs_count: 1,
                in_events: &input,
                out_events: ptr::null(),
            };
            assert_no_alloc(|| {
                assert_ne!(p.process.unwrap()(plugin, &process), CLAP_PROCESS_ERROR);
            });
            events.clear();
            process.frames_count = 4096;
            for _ in 0..40 {
                assert_no_alloc(|| {
                    assert_ne!(p.process.unwrap()(plugin, &process), CLAP_PROCESS_ERROR);
                });
            }
            assert!(
                left.iter().all(|&sample| sample.abs() < 1e-6),
                "overflow must release unseen note-offs: peak {}",
                left.iter().copied().map(f32::abs).fold(0.0, f32::max)
            );
            // Normal host note, then saturated gain automation + monophonic modulation.
            p.reset.unwrap()(plugin);
            events.push(clap_event_midi {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_midi>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_MIDI,
                    flags: 0,
                },
                port_index: 0,
                data: [0x90, 60, 100],
            });
            assert_no_alloc(|| {
                p.process.unwrap()(plugin, &process);
            });
            let baseline = left;
            assert!(baseline.iter().any(|sample| sample.abs() > 0.001));
            p.reset.unwrap()(plugin);
            let values: Vec<_> = (0..257)
                .map(|index| clap_event_param_value {
                    header: clap_event_header {
                        size: std::mem::size_of::<clap_event_param_value>() as u32,
                        time: 0,
                        space_id: CLAP_CORE_EVENT_SPACE_ID,
                        type_: CLAP_EVENT_PARAM_VALUE,
                        flags: 0,
                    },
                    param_id: info.id,
                    cookie: ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: if index == 256 { 0.4 } else { 0.7 },
                })
                .collect();
            let modulation = clap_event_param_mod {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_param_mod>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_PARAM_MOD,
                    flags: 0,
                },
                param_id: info.id,
                cookie: ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                amount: 0.2,
            };
            let mut pointers: Vec<*const clap_event_header> = values
                .iter()
                .map(|value| &value.header as *const _)
                .collect();
            pointers.push(&modulation.header);
            pointers.push(&events[0].header);
            let gain_events = clap_input_events {
                ctx: (&mut pointers as *mut Vec<*const clap_event_header>).cast(),
                size: Some(pointer_size),
                get: Some(pointer_get),
            };
            process.in_events = &gain_events;
            assert_no_alloc(|| {
                p.process.unwrap()(plugin, &process);
            });
            assert!((shared.snapshot().output_gain - 0.4).abs() < 1e-6);
            assert!(params.get_value.unwrap()(plugin, info.id, &mut value));
            assert!((value - 0.6).abs() < 1e-6);
            assert!(
                left.iter()
                    .zip(baseline)
                    .all(|(&sample, reference)| (sample - reference * 0.6).abs() < 1e-6),
                "gain modulation must reach audio"
            );
            events.clear();
            process.in_events = &input;
            assert_no_alloc(|| p.reset.unwrap()(plugin));
            p.stop_processing.unwrap()(plugin);
            p.deactivate.unwrap()(plugin);
            assert!(p.activate.unwrap()(plugin, 96000.0, 1, 4096));
            assert!(p.start_processing.unwrap()(plugin));
            assert_no_alloc(|| {
                assert_ne!(p.process.unwrap()(plugin, &process), CLAP_PROCESS_ERROR);
            });
            assert!(left.iter().all(|&sample| sample == 0.0));
            p.stop_processing.unwrap()(plugin);
            p.deactivate.unwrap()(plugin);
            p.destroy.unwrap()(plugin);
        }
    }
}
