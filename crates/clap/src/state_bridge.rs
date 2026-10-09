//! CLAP entry: production transactional state and MIDI-only dialect. The pinned wrapper
//! owns GUI/lifecycle/parameter events; its non-atomic state loader is never used.
use super::{Instrument, Shared};
use crate::sound_state::{SoundState, MAX_STATE_BYTES};
use std::cell::{RefCell, UnsafeCell};
use std::sync::Weak;
type Capture = (
    Weak<Shared>,
    rtrb::Producer<(u32, [u8; 3])>,
    rtrb::Producer<(u32, crate::schedule::Event)>,
);
thread_local! { static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) }; }
pub(super) fn capture(
    shared: &Arc<Shared>,
    midi: rtrb::Producer<(u32, [u8; 3])>,
    events: rtrb::Producer<(u32, crate::schedule::Event)>,
) {
    CAPTURE.with(|c| *c.borrow_mut() = Some((Arc::downgrade(shared), midi, events)));
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
    callback: nice_plug::wrapper::clap::strict::Owner<()>,
    bridge: Bridge,
    process: unsafe extern "C" fn(*const clap_plugin, *const clap_process) -> clap_process_status,
    midi: UnsafeCell<rtrb::Producer<(u32, [u8; 3])>>,
    param_ids: [u32; 18],
    events: UnsafeCell<rtrb::Producer<(u32, crate::schedule::Event)>>,
    start: unsafe extern "C" fn(*const clap_plugin) -> bool,
    reset: unsafe extern "C" fn(*const clap_plugin),
    cc_open: UnsafeCell<u32>,
    params: clap_plugin_params,
    flush: unsafe extern "C" fn(
        *const clap_plugin,
        *const clap_input_events,
        *const clap_output_events,
    ),
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
    let (shared, midi, events) = CAPTURE
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
    let mut param_ids = [u32::MAX; 18];
    for (index, id) in param_ids.iter_mut().enumerate() {
        let mut info: clap_param_info = unsafe { std::mem::zeroed() };
        if unsafe { parameters.get_info.unwrap()(&original, index as u32, &mut info) } {
            *id = info.id;
        }
    }
    let mut bounded_params = *parameters;
    bounded_params.flush = Some(flush_bridge);
    bounded_params.get_value = Some(get_value_bridge);
    let mut outer = Box::new(Outer {
        plugin: original,
        callback: nice_plug::wrapper::clap::strict::Owner::new(()),
        bridge: Bridge {
            get_extension,
            destroy,
            host: host as usize,
            shared,
            on_main: original.on_main_thread.expect("main callback"),
        },
        process,
        midi: UnsafeCell::new(midi),
        param_ids,
        events: UnsafeCell::new(events),
        start: original.start_processing.expect("start processing"),
        reset: original.reset.expect("reset"),
        cc_open: UnsafeCell::new(0),
        params: bounded_params,
        flush: parameters.flush.expect("parameter flush"),
    });
    outer.plugin.on_main_thread = Some(on_main_bridge);
    outer.plugin.get_extension = Some(get_extension_bridge);
    outer.plugin.destroy = Some(destroy_bridge);
    outer.plugin.process = Some(process_bridge);
    outer.plugin.start_processing = Some(start_bridge);
    outer.plugin.reset = Some(reset_bridge);
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
    } else if unsafe { CStr::from_ptr(id) } == CLAP_EXT_PARAMS {
        unsafe { &(*plugin.cast::<Outer>()).params as *const _ as *const c_void }
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

unsafe extern "C" fn get_value_bridge(
    plugin: *const clap_plugin,
    id: u32,
    value: *mut f64,
) -> bool {
    let Some(outer) = (unsafe { plugin.cast::<Outer>().as_ref() }) else {
        return false;
    };
    let Some(value) = (unsafe { value.as_mut() }) else {
        return false;
    };
    let Some(index) = outer
        .param_ids
        .iter()
        .position(|&candidate| candidate == id)
    else {
        return false;
    };
    *value = outer.bridge.shared.native_value(index, true) as f64;
    true
}
unsafe extern "C" fn reset_bridge(plugin: *const clap_plugin) {
    let outer = unsafe { &*plugin.cast::<Outer>() };
    let Some(_callback) = outer.callback.try_borrow_mut() else {
        return;
    };
    outer.bridge.shared.apply_recall();
    unsafe { (outer.reset)(plugin) };
}
unsafe extern "C" fn start_bridge(plugin: *const clap_plugin) -> bool {
    let outer = unsafe { &*plugin.cast::<Outer>() };
    let Some(_callback) = outer.callback.try_borrow_mut() else {
        return false;
    };
    outer.bridge.shared.apply_recall();
    outer
        .bridge
        .shared
        .start_reset
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let result = unsafe { (outer.start)(plugin) };
    outer
        .bridge
        .shared
        .start_reset
        .store(false, std::sync::atomic::Ordering::Relaxed);
    result
}

const MAX_HOST_EVENTS: u32 = 2048;
const MAX_PARAMS: usize = 128;
struct ParamEvents {
    events: [*const clap_event_header; MAX_PARAMS],
    len: usize,
}
impl ParamEvents {
    unsafe fn push(&mut self, event: *const clap_event_header) {
        // Reserve final value/modulation for every exposed parameter. Earlier events retain
        // order; saturation coalesces only matching identity and event type.
        const RESERVED: usize = 36;
        if self.len < MAX_PARAMS - RESERVED {
            self.events[self.len] = event;
            self.len += 1;
            return;
        }
        let key = |e: *const clap_event_header| unsafe {
            ((*e).type_, (*e.cast::<clap_event_param_value>()).param_id)
        };
        if let Some(index) =
            (MAX_PARAMS - RESERVED..self.len).find(|&i| key(self.events[i]) == key(event))
        {
            self.events[index] = event;
        } else if self.len < MAX_PARAMS {
            self.events[self.len] = event;
            self.len += 1;
        }
        self.events[MAX_PARAMS - RESERVED..self.len]
            .sort_unstable_by_key(|e| unsafe { (**e).time });
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
// CLAP flush is serialized with process; inactive flush runs on main. Keep only bounded
// native parameter input and retain exact slot values until the next processing callback.
unsafe extern "C" fn flush_bridge(
    plugin: *const clap_plugin,
    input: *const clap_input_events,
    output: *const clap_output_events,
) {
    let outer = unsafe { &*plugin.cast::<Outer>() };
    let Some(_callback) = outer.callback.try_borrow_mut() else {
        return;
    };
    let shared = &outer.bridge.shared;
    shared.apply_recall();
    let mut parameters = ParamEvents {
        events: [ptr::null(); MAX_PARAMS],
        len: 0,
    };
    if let Some(input) = unsafe { input.as_ref() } {
        if let (Some(size), Some(get)) = (input.size, input.get) {
            let count = unsafe { size(input) };
            for index in 0..count.min(MAX_HOST_EVENTS) {
                let event = unsafe { get(input, index) };
                let Some(header) = (unsafe { event.as_ref() }) else {
                    continue;
                };
                let bytes = match header.type_ {
                    CLAP_EVENT_PARAM_VALUE => std::mem::size_of::<clap_event_param_value>(),
                    CLAP_EVENT_PARAM_MOD => std::mem::size_of::<clap_event_param_mod>(),
                    _ => continue,
                };
                if header.space_id == CLAP_CORE_EVENT_SPACE_ID && header.size as usize >= bytes {
                    let id = unsafe { (*event.cast::<clap_event_param_value>()).param_id };
                    if outer.param_ids.contains(&id) {
                        unsafe { parameters.push(event) };
                    }
                }
            }
            shared.event_drops.fetch_add(
                u64::from(count > MAX_HOST_EVENTS),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }
    for &event in &parameters.events[..parameters.len] {
        let id = unsafe { (*event.cast::<clap_event_param_value>()).param_id };
        if let Some(slot) = outer.param_ids[1..17]
            .iter()
            .position(|&candidate| candidate == id)
        {
            if unsafe { (*event).type_ == CLAP_EVENT_PARAM_VALUE } {
                let value = unsafe { (*event.cast::<clap_event_param_value>()).value as f32 };
                if value.is_finite() {
                    shared.flushed_values[slot].store(
                        value.clamp(0.0, 1.0).to_bits(),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    shared
                        .value_notify
                        .fetch_or(1 << slot, std::sync::atomic::Ordering::Release);
                }
            } else {
                let value = unsafe { (*event.cast::<clap_event_param_mod>()).amount as f32 };
                if value.is_finite() {
                    shared.flushed_modulation[slot]
                        .store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
                    shared
                        .modulation_notify
                        .fetch_or(1 << slot, std::sync::atomic::Ordering::Release);
                }
            }
        }
    }
    let filtered = clap_input_events {
        ctx: (&mut parameters as *mut ParamEvents).cast(),
        size: Some(filtered_size),
        get: Some(filtered_get),
    };
    unsafe { (outer.flush)(plugin, &filtered, output) };
}
unsafe extern "C" fn process_bridge(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    if plugin.is_null() || process.is_null() {
        return CLAP_PROCESS_ERROR;
    }
    let outer = unsafe { &*plugin.cast::<Outer>() };
    let Some(_callback) = outer.callback.try_borrow_mut() else {
        return CLAP_PROCESS_ERROR;
    };
    let input = unsafe { &*process };
    let shared = &outer.bridge.shared;
    shared.apply_recall();
    shared
        .host_frames
        .store(input.frames_count, std::sync::atomic::Ordering::Relaxed);
    let host_events = unsafe { &mut *outer.events.get() };
    let position = crate::schedule::Position::from_clap(unsafe { input.transport.as_ref() });
    let mut event_drops = u64::from(
        host_events
            .push((0, crate::schedule::Event::Position(position)))
            .is_err(),
    );
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
                            if outer.param_ids.contains(&id) {
                                unsafe {
                                    parameters.push(event);
                                }
                            }
                        }
                    }
                    CLAP_EVENT_TRANSPORT
                        if header.size as usize >= std::mem::size_of::<clap_event_transport>()
                            && header.time < input.frames_count =>
                    {
                        let position = crate::schedule::Position::from_clap(Some(unsafe {
                            &*event.cast::<clap_event_transport>()
                        }));
                        event_drops += u64::from(
                            host_events
                                .push((header.time, crate::schedule::Event::Position(position)))
                                .is_err(),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    shared
        .midi_overflow
        .store(overflow, std::sync::atomic::Ordering::Relaxed);
    for &event in &parameters.events[..parameters.len] {
        let id = unsafe { (*event.cast::<clap_event_param_value>()).param_id };
        if let Some(slot) = outer.param_ids[1..17]
            .iter()
            .position(|&candidate| candidate == id)
        {
            let modulation = unsafe { (*event).type_ == CLAP_EVENT_PARAM_MOD };
            let value = if modulation {
                unsafe { (*event.cast::<clap_event_param_mod>()).amount }
            } else {
                unsafe { (*event.cast::<clap_event_param_value>()).value }
            };
            event_drops += u64::from(
                host_events
                    .push((
                        unsafe { (*event).time },
                        crate::schedule::Event::Value {
                            slot,
                            value: value as f32,
                            modulation,
                        },
                    ))
                    .is_err(),
            );
        }
    }
    shared.event_drops.fetch_add(
        event_drops + u64::from(overflow),
        std::sync::atomic::Ordering::Relaxed,
    );
    // Mapped physical/virtual CC gestures record with the editor closed. UI gestures already
    // go through the framework setter. Failed output pushes retain their notification.
    let notifications = shared
        .cc_notify
        .swap(0, std::sync::atomic::Ordering::Acquire);
    let mut remaining = notifications;
    let pending = notifications | unsafe { *outer.cc_open.get() };
    if let Some(output) = unsafe { input.out_events.as_ref() } {
        if let Some(push) = output.try_push {
            for i in 0..crate::automation::SLOTS {
                if pending & (1 << i) == 0 {
                    continue;
                }
                let bit = 1 << i;
                let epoch = shared.committed.load(std::sync::atomic::Ordering::SeqCst);
                let event_epoch = shared.cc_epochs[i].load(std::sync::atomic::Ordering::Acquire);
                let stale = event_epoch != epoch || !epoch.is_multiple_of(2);
                if stale {
                    remaining &= !bit;
                    if unsafe { *outer.cc_open.get() } & bit == 0 {
                        continue;
                    }
                } else if notifications & bit != 0
                    && epoch
                        != shared
                            .native_epoch
                            .load(std::sync::atomic::Ordering::Acquire)
                {
                    continue; // Recall not synchronized yet: retain exact pending value.
                }
                let exact =
                    f32::from_bits(shared.cc_values[i].load(std::sync::atomic::Ordering::Acquire));
                if !stale && shared.committed.load(std::sync::atomic::Ordering::SeqCst) != epoch {
                    continue;
                }
                let send_value = !stale && notifications & bit != 0;
                if send_value {
                    unsafe {
                        shared
                            .native_ptr(i + 1)
                            ._internal_set_normalized_value(exact);
                    }
                }
                let mut gesture = clap_event_param_gesture {
                    header: clap_event_header {
                        size: std::mem::size_of::<clap_event_param_gesture>() as u32,
                        time: 0,
                        space_id: CLAP_CORE_EVENT_SPACE_ID,
                        type_: CLAP_EVENT_PARAM_GESTURE_BEGIN,
                        flags: CLAP_EVENT_IS_LIVE,
                    },
                    param_id: outer.param_ids[i + 1],
                };
                let value = clap_event_param_value {
                    header: clap_event_header {
                        size: std::mem::size_of::<clap_event_param_value>() as u32,
                        type_: CLAP_EVENT_PARAM_VALUE,
                        ..gesture.header
                    },
                    param_id: gesture.param_id,
                    cookie: ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: exact as f64,
                };
                let open = unsafe { &mut *outer.cc_open.get() };
                let began = *open & bit != 0 || unsafe { push(output, &gesture.header) };
                if began {
                    *open |= bit;
                }
                let accepted = began && (!send_value || unsafe { push(output, &value.header) });
                gesture.header.type_ = CLAP_EVENT_PARAM_GESTURE_END;
                let ended = accepted && unsafe { push(output, &gesture.header) };
                if ended {
                    *open &= !bit;
                }
                if ended {
                    remaining &= !bit;
                }
            }
        }
    }
    shared
        .cc_notify
        .fetch_or(remaining, std::sync::atomic::Ordering::Release);
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

    struct OutputProbe {
        reject: u16,
        begins: u32,
        values: u32,
        ends: u32,
    }
    unsafe extern "C" fn probe_push(
        list: *const clap_output_events,
        event: *const clap_event_header,
    ) -> bool {
        let probe = unsafe { &mut *((*list).ctx.cast::<OutputProbe>()) };
        let kind = unsafe { (*event).type_ };
        if kind == probe.reject {
            return false;
        }
        match kind {
            CLAP_EVENT_PARAM_GESTURE_BEGIN => probe.begins += 1,
            CLAP_EVENT_PARAM_VALUE => probe.values += 1,
            CLAP_EVENT_PARAM_GESTURE_END => probe.ends += 1,
            _ => {}
        }
        true
    }
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
            assert_no_alloc(|| {
                assert!(p.start_processing.unwrap()(plugin));
            });
            assert_no_alloc(|| {
                assert!(!p.start_processing.unwrap()(plugin));
            });
            let params = &*(p.get_extension.unwrap()(plugin, CLAP_EXT_PARAMS.as_ptr())
                .cast::<clap_plugin_params>());
            let mut info: clap_param_info = std::mem::zeroed();
            assert!(params.get_info.unwrap()(plugin, 0, &mut info));
            assert_eq!(params.count.unwrap()(plugin), 18);
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
            let tail =
                &*(p.get_extension.unwrap()(plugin, clap_sys::ext::tail::CLAP_EXT_TAIL.as_ptr())
                    .cast::<clap_sys::ext::tail::clap_plugin_tail>());
            assert_eq!(
                tail.get.unwrap()(plugin),
                u32::MAX,
                "Mapped automation must not truncate tails"
            );
            events.clear();
            let mut probe = OutputProbe {
                reject: CLAP_EVENT_PARAM_GESTURE_BEGIN,
                begins: 0,
                values: 0,
                ends: 0,
            };
            let output_events = clap_output_events {
                ctx: (&mut probe as *mut OutputProbe).cast(),
                try_push: Some(probe_push),
            };
            process.out_events = &output_events;
            // The strict framework keeps saturated gestures ordered and retires stale values
            // across the same complete-state epoch that protects the DSP/session publication.
            let wrapper = &*p.plugin_data.cast::<Wrapper<Instrument>>();
            use nice_plug::wrapper::clap::OutputParamEvent;
            let before_handoff = shared.snapshot();
            let mut unsupported =
                serde_json::from_value(serde_json::json!({"params": {}, "fields": {}})).unwrap();
            assert!(!wrapper.set_state_inner(&mut unsupported));
            assert_eq!(
                serde_json::to_vec(&shared.snapshot()).unwrap(),
                serde_json::to_vec(&before_handoff).unwrap()
            );
            for _ in 0..684 {
                wrapper.queue_parameter_event(OutputParamEvent::BeginGesture {
                    param_hash: info.id,
                });
                wrapper.queue_parameter_event(OutputParamEvent::SetValue {
                    param_hash: info.id,
                    clap_plain_value: 0.2,
                });
                wrapper.queue_parameter_event(OutputParamEvent::EndGesture {
                    param_hash: info.id,
                });
            }
            let mut recalled = shared.snapshot();
            recalled.output_gain = 0.61;
            shared.load(recalled).unwrap();
            assert_no_alloc(|| p.reset.unwrap()(plugin));
            probe.reject = u16::MAX;
            assert_no_alloc(|| {
                p.process.unwrap()(plugin, &process);
            });
            assert_eq!(probe.begins, 683); // Fixed 2048-record budget, including retired values.
            assert_eq!(probe.ends, 682);
            assert_eq!(probe.values, 0);
            p.on_main_thread.unwrap()(plugin); // Overflow pump remains off audio.
            assert_no_alloc(|| params.flush.unwrap()(plugin, &input, &output_events));
            assert_eq!((probe.begins, probe.values, probe.ends), (684, 0, 684));
            assert!(params.get_value.unwrap()(plugin, info.id, &mut value));
            assert!(
                (value - 0.61).abs() < 1e-6,
                "old queued values must not overwrite recall"
            );
            // Failed host pushes retain the next complete gesture, not a detached End.
            wrapper.queue_parameter_event(OutputParamEvent::BeginGesture {
                param_hash: info.id,
            });
            wrapper.queue_parameter_event(OutputParamEvent::SetValue {
                param_hash: info.id,
                clap_plain_value: 0.7,
            });
            wrapper.queue_parameter_event(OutputParamEvent::EndGesture {
                param_hash: info.id,
            });
            probe.begins = 0;
            probe.values = 0;
            probe.ends = 0;
            for rejected in [
                CLAP_EVENT_PARAM_GESTURE_BEGIN,
                CLAP_EVENT_PARAM_VALUE,
                CLAP_EVENT_PARAM_GESTURE_END,
                u16::MAX,
            ] {
                probe.reject = rejected;
                assert_no_alloc(|| params.flush.unwrap()(plugin, &input, &output_events));
            }
            assert_eq!((probe.begins, probe.values, probe.ends), (1, 1, 1));
            shared.load(before_handoff).unwrap();
            assert_no_alloc(|| p.reset.unwrap()(plugin));
            probe.begins = 0;
            probe.values = 0;
            probe.ends = 0;
            shared.cc_epochs[0].store(shared.committed.load(Ordering::Acquire), Ordering::Release);
            shared.cc_notify.store(1, Ordering::Release);
            for rejected in [
                CLAP_EVENT_PARAM_GESTURE_BEGIN,
                CLAP_EVENT_PARAM_VALUE,
                CLAP_EVENT_PARAM_GESTURE_END,
                u16::MAX,
            ] {
                probe.reject = rejected;
                assert_no_alloc(|| {
                    p.process.unwrap()(plugin, &process);
                });
                assert_eq!(
                    shared.cc_notify.load(Ordering::Acquire),
                    u32::from(rejected != u16::MAX)
                );
            }
            assert_eq!(probe.begins, 1);
            assert_eq!(probe.ends, 1);
            assert_eq!(probe.values, 2);
            probe = OutputProbe {
                reject: CLAP_EVENT_PARAM_GESTURE_END,
                begins: 0,
                values: 0,
                ends: 0,
            };
            shared.cc_epochs[0].store(shared.committed.load(Ordering::Acquire), Ordering::Release);
            shared.cc_notify.store(1, Ordering::Release);
            assert_no_alloc(|| {
                p.process.unwrap()(plugin, &process);
            });
            // A state load discards stale controller values, but an accepted begin still needs end.
            let recalled = shared.snapshot();
            shared.load(recalled).unwrap();
            // Old callback can retry its consumed mask after publication. Epoch retires its
            // value, but the already accepted Begin must still receive exactly one End.
            shared.cc_notify.fetch_or(1, Ordering::Release);
            probe.reject = u16::MAX;
            assert_no_alloc(|| {
                p.process.unwrap()(plugin, &process);
            });
            assert_eq!((probe.begins, probe.values, probe.ends), (1, 1, 1));
            process.out_events = ptr::null();
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
            let original_fixture = shared.snapshot();
            let mut fixture = shared.snapshot();
            fixture.patch.cables.remove(&5); // Isolate VCA base from additive envelope CV.
            fixture
                .patch
                .modules
                .get_mut(&5)
                .unwrap()
                .params
                .insert("gain".into(), 0.5);
            for i in 0..crate::automation::SLOTS {
                fixture.slot_values[i] = fixture.lanes[i].target.as_ref().map_or(0.0, |target| {
                    crate::automation::normalized(&fixture.patch, target)
                });
            }
            shared.load(fixture).unwrap();
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
            events.clear();
            let slot = shared
                .control
                .lock()
                .unwrap()
                .lanes
                .iter()
                .position(|lane| {
                    lane.target
                        .as_ref()
                        .is_some_and(|t| t.kind == "vca" && t.param == "gain")
                })
                .unwrap();
            let slot_id = (*plugin.cast::<Outer>()).param_ids[slot + 1];
            let mut flushed = clap_event_param_value {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_param_value>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_PARAM_VALUE,
                    flags: 0,
                },
                param_id: slot_id,
                cookie: ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: 0.0,
            };
            let mut pointers = vec![&flushed.header as *const clap_event_header];
            let flush_input = clap_input_events {
                ctx: (&mut pointers as *mut Vec<*const clap_event_header>).cast(),
                size: Some(pointer_size),
                get: Some(pointer_get),
            };
            assert_no_alloc(|| params.flush.unwrap()(plugin, &flush_input, ptr::null()));
            // No process PARAM_VALUE echo or reactivation: the held note must become silent.
            for _ in 0..40 {
                assert_no_alloc(|| {
                    p.process.unwrap()(plugin, &process);
                });
            }
            assert!(left.iter().all(|s| s.abs() < 1e-6));
            flushed.value = 1.0;
            assert_no_alloc(|| params.flush.unwrap()(plugin, &flush_input, ptr::null()));
            let mut flushed_mod = clap_event_param_mod {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_param_mod>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_PARAM_MOD,
                    flags: 0,
                },
                param_id: slot_id,
                cookie: ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                amount: -1.0,
            };
            pointers[0] = &flushed_mod.header;
            assert_no_alloc(|| params.flush.unwrap()(plugin, &flush_input, ptr::null()));
            for _ in 0..40 {
                assert_no_alloc(|| {
                    p.process.unwrap()(plugin, &process);
                });
            }
            assert!(left.iter().all(|s| s.abs() < 1e-6));
            // An older flush must not overwrite newer process events at sample zero.
            p.reset.unwrap()(plugin);
            flushed.value = 0.0;
            pointers[0] = &flushed.header;
            assert_no_alloc(|| params.flush.unwrap()(plugin, &flush_input, ptr::null()));
            let mut newer = flushed;
            newer.value = 0.5;
            let mut newer_modulation = flushed_mod;
            newer_modulation.amount = 0.0;
            let note = clap_event_midi {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_midi>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_MIDI,
                    flags: 0,
                },
                port_index: 0,
                data: [0x90, 60, 100],
            };
            let mut newer_events = vec![
                &newer.header as *const clap_event_header,
                &newer_modulation.header,
                &note.header,
            ];
            let newer_input = clap_input_events {
                ctx: (&mut newer_events as *mut Vec<*const clap_event_header>).cast(),
                size: Some(pointer_size),
                get: Some(pointer_get),
            };
            process.in_events = &newer_input;
            assert_no_alloc(|| {
                p.process.unwrap()(plugin, &process);
            });
            assert!(left
                .iter()
                .zip(baseline)
                .all(|(&v, reference)| (v - reference).abs() < 1e-6));
            process.in_events = &input;
            flushed_mod.amount = 0.0;
            std::hint::black_box(&flushed_mod);
            assert_no_alloc(|| params.flush.unwrap()(plugin, &flush_input, ptr::null()));
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

            flushed.value = 0.5;
            pointers[0] = &flushed.header;
            assert_no_alloc(|| params.flush.unwrap()(plugin, &flush_input, ptr::null()));
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
            shared.load(original_fixture).unwrap();
            assert_no_alloc(|| p.stop_processing.unwrap()(plugin));
            assert_no_alloc(|| {
                assert_eq!(p.process.unwrap()(plugin, &process), CLAP_PROCESS_ERROR);
            });
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
    #[test]
    fn main_track_notification_can_pause_while_audio_owns_plugin() {
        use clap_sys::ext::track_info::*;
        use std::sync::{atomic::AtomicBool, Barrier};
        struct TrackHost {
            pause: AtomicBool,
            barrier: Barrier,
        }
        unsafe extern "C" fn get(host: *const clap_host, info: *mut clap_track_info) -> bool {
            let test = unsafe { &*((*host).host_data.cast::<TrackHost>()) };
            if test.pause.load(Ordering::Acquire) {
                test.barrier.wait();
                test.barrier.wait();
            }
            unsafe {
                (*info).flags = CLAP_TRACK_INFO_HAS_TRACK_NAME;
                (*info).name[0] = b'x' as c_char;
            }
            true
        }
        static TRACK: clap_host_track_info = clap_host_track_info { get: Some(get) };
        unsafe extern "C" fn extension(_: *const clap_host, id: *const c_char) -> *const c_void {
            if unsafe { CStr::from_ptr(id) } == CLAP_EXT_TRACK_INFO {
                (&TRACK as *const clap_host_track_info).cast()
            } else {
                ptr::null()
            }
        }
        let test = TrackHost {
            pause: AtomicBool::new(false),
            barrier: Barrier::new(2),
        };
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: (&test as *const TrackHost).cast_mut().cast(),
            name: c"track host".as_ptr(),
            vendor: c"kabl".as_ptr(),
            url: c"".as_ptr(),
            version: c"1".as_ptr(),
            get_extension: Some(extension),
            request_restart: Some(host_request),
            request_process: Some(host_request),
            request_callback: Some(host_request),
        };
        unsafe {
            let plugin = create(&FACTORY, &host, descriptor().clap_id().as_ptr());
            let p = &*plugin;
            assert!(p.init.unwrap()(plugin));
            (&*plugin.cast::<Outer>())
                .bridge
                .shared
                .stop
                .store(true, Ordering::Release);
            assert!(p.activate.unwrap()(plugin, 48000.0, 1, 256));
            assert!(!p.activate.unwrap()(plugin, 48000.0, 1, 256));
            assert_no_alloc(|| assert!(p.start_processing.unwrap()(plugin)));
            let track = &*(p.get_extension.unwrap()(plugin, CLAP_EXT_TRACK_INFO.as_ptr())
                .cast::<clap_plugin_track_info>());
            test.pause.store(true, Ordering::Release);
            let address = plugin as usize;
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    test.barrier.wait();
                    let plugin = address as *const clap_plugin;
                    let mut left = [0.0f32; 256];
                    let mut right = [0.0f32; 256];
                    let mut channels = [left.as_mut_ptr(), right.as_mut_ptr()];
                    let mut output = clap_audio_buffer {
                        data32: channels.as_mut_ptr(),
                        data64: ptr::null_mut(),
                        channel_count: 2,
                        latency: 0,
                        constant_mask: 0,
                    };
                    let mut events = Vec::new();
                    let input = clap_input_events {
                        ctx: (&mut events as *mut Vec<clap_event_midi>).cast(),
                        size: Some(event_size),
                        get: Some(event_get),
                    };
                    let process = clap_process {
                        steady_time: 0,
                        frames_count: 256,
                        transport: ptr::null(),
                        audio_inputs: ptr::null(),
                        audio_outputs: &mut output,
                        audio_inputs_count: 0,
                        audio_outputs_count: 1,
                        in_events: &input,
                        out_events: ptr::null(),
                    };
                    for _ in 0..64 {
                        assert_no_alloc(|| {
                            assert_ne!(
                                (*plugin).process.unwrap()(plugin, &process),
                                CLAP_PROCESS_ERROR
                            )
                        });
                    }
                    test.barrier.wait();
                });
                track.changed.unwrap()(plugin);
            });
            assert_no_alloc(|| p.stop_processing.unwrap()(plugin));
            p.deactivate.unwrap()(plugin);
            p.destroy.unwrap()(plugin);
        }
    }
}
