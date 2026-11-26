//! What kabl's VST3 build (clap-wrapper over this CLAP entry) depends on that the CLAP hosts
//! never exercised: VST3 notes arrive as CLAP note events, not MIDI; and MIDI controllers
//! arrive as clap-wrapper turns its IMidiMapping parameters into MIDI.

use super::*;
use assert_no_alloc::assert_no_alloc;
use clap_sys::audio_buffer::clap_audio_buffer;
use std::sync::atomic::Ordering;

const BLOCK: usize = 512;
const RATE: f64 = 48000.0;

unsafe extern "C" fn size(list: *const clap_input_events) -> u32 {
    unsafe { (&*((*list).ctx.cast::<Vec<*const clap_event_header>>())).len() as u32 }
}
unsafe extern "C" fn get(list: *const clap_input_events, index: u32) -> *const clap_event_header {
    unsafe { (&*((*list).ctx.cast::<Vec<*const clap_event_header>>()))[index as usize] }
}
unsafe extern "C" fn no_extension(_: *const clap_host, _: *const c_char) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn request(_: *const clap_host) {}

fn header(kind: u16, size: usize) -> clap_event_header {
    clap_event_header {
        size: size as u32,
        time: 0,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_: kind,
        flags: 0,
    }
}

fn note(kind: u16, key: i16, velocity: f64) -> clap_event_note {
    clap_event_note {
        header: header(kind, std::mem::size_of::<clap_event_note>()),
        note_id: -1,
        port_index: 0,
        channel: 0,
        key,
        velocity,
    }
}

fn midi(data: [u8; 3]) -> clap_event_midi {
    clap_event_midi {
        header: header(CLAP_EVENT_MIDI, std::mem::size_of::<clap_event_midi>()),
        port_index: 0,
        data,
    }
}

static HOST: clap_host = clap_host {
    clap_version: CLAP_VERSION,
    host_data: ptr::null_mut(),
    name: c"test host".as_ptr(),
    vendor: c"kabl".as_ptr(),
    url: c"".as_ptr(),
    version: c"1".as_ptr(),
    get_extension: Some(no_extension),
    request_restart: Some(request),
    request_process: Some(request),
    request_callback: Some(request),
};

/// The exported plugin, active and processing, rendering through the C ABI as a host does.
struct Rig {
    plugin: *const clap_plugin,
    left: Vec<f32>,
}

impl Rig {
    /// `worker`: keep kabl's control thread, which applies CC mappings, running.
    fn new(state: Option<SoundState>, worker: bool) -> Self {
        unsafe {
            let plugin = create(&FACTORY, &HOST, descriptor().clap_id().as_ptr());
            assert!(!plugin.is_null());
            let p = &*plugin;
            assert!(p.init.unwrap()(plugin));
            let shared = &(*plugin.cast::<Outer>()).bridge.shared;
            if !worker {
                shared.stop.store(true, Ordering::Release);
            }
            if let Some(state) = state {
                shared.load(state).unwrap();
            }
            assert!(p.activate.unwrap()(plugin, RATE, 1, BLOCK as u32));
            assert!(p.start_processing.unwrap()(plugin));
            Rig {
                plugin,
                left: Vec::new(),
            }
        }
    }

    /// `blocks` blocks of audio, `events` in the first; `pause` between blocks gives the
    /// control thread the time a real host's callback interval does.
    fn run(&mut self, events: &[*const clap_event_header], blocks: usize, pause: bool) {
        let mut list: Vec<*const clap_event_header> = Vec::with_capacity(events.len());
        list.extend_from_slice(events);
        let input = clap_input_events {
            ctx: (&mut list as *mut Vec<*const clap_event_header>).cast(),
            size: Some(size),
            get: Some(get),
        };
        let mut left = [0.0f32; BLOCK];
        let mut right = [0.0f32; BLOCK];
        let mut channels = [left.as_mut_ptr(), right.as_mut_ptr()];
        let mut output = clap_audio_buffer {
            data32: channels.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let process = clap_process {
            steady_time: -1,
            frames_count: BLOCK as u32,
            transport: ptr::null(),
            audio_inputs: ptr::null(),
            audio_outputs: &mut output,
            audio_inputs_count: 0,
            audio_outputs_count: 1,
            in_events: &input,
            out_events: ptr::null(),
        };
        for _ in 0..blocks {
            unsafe {
                let p = &*self.plugin;
                assert_no_alloc(|| {
                    assert_ne!(
                        p.process.unwrap()(self.plugin, &process),
                        CLAP_PROCESS_ERROR
                    );
                });
                self.left
                    .extend_from_slice(std::slice::from_raw_parts(channels[0], BLOCK));
            }
            list.clear();
            if pause {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }

    fn last(&self, blocks: usize) -> &[f32] {
        &self.left[self.left.len() - blocks * BLOCK..]
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        unsafe {
            let p = &*self.plugin;
            p.stop_processing.unwrap()(self.plugin);
            p.deactivate.unwrap()(self.plugin);
            p.destroy.unwrap()(self.plugin);
        }
    }
}

fn peak(s: &[f32]) -> f32 {
    s.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}
fn rms(s: &[f32]) -> f64 {
    (s.iter().map(|&x| x as f64 * x as f64).sum::<f64>() / s.len() as f64).sqrt()
}
/// Rising zero crossings with hysteresis, as host.cpp measures them.
fn frequency(s: &[f32]) -> f64 {
    let threshold = 0.2 * peak(s);
    let (mut below, mut first, mut last, mut count) = (false, None, 0, 0);
    for (i, &v) in s.iter().enumerate() {
        if v < -threshold {
            below = true;
        } else if below && v > threshold {
            below = false;
            first.get_or_insert(i);
            last = i;
            count += 1;
        }
    }
    match first {
        Some(first) if count >= 2 => (count - 1) as f64 * RATE / (last - first) as f64,
        _ => 0.0,
    }
}

#[test]
fn clap_notes_become_the_midi_bytes_they_stand_for() {
    use crate::notes::clap_note_bytes;
    let on = note(CLAP_EVENT_NOTE_ON, 60, 0.8);
    assert_eq!(
        clap_note_bytes(CLAP_EVENT_NOTE_ON, &on),
        Some([0x90, 60, 102])
    );
    let quiet = note(CLAP_EVENT_NOTE_ON, 60, 0.0);
    assert_eq!(
        clap_note_bytes(CLAP_EVENT_NOTE_ON, &quiet),
        Some([0x90, 60, 1]),
        "a silent note on must not turn into a note off"
    );
    let mut off = note(CLAP_EVENT_NOTE_OFF, 61, 0.5);
    off.channel = 9;
    assert_eq!(
        clap_note_bytes(CLAP_EVENT_NOTE_OFF, &off),
        Some([0x89, 61, 64])
    );
    let wildcard = note(CLAP_EVENT_NOTE_OFF, -1, 0.0);
    assert_eq!(clap_note_bytes(CLAP_EVENT_NOTE_OFF, &wildcard), None);
    let mut other_port = note(CLAP_EVENT_NOTE_ON, 60, 0.8);
    other_port.port_index = 1;
    assert_eq!(clap_note_bytes(CLAP_EVENT_NOTE_ON, &other_port), None);
}

/// clap-wrapper hands every VST3 note to the plugin as a CLAP note event, whatever dialect
/// the note port declares. Before this they were dropped and kabl was silent as a VST3.
#[test]
fn clap_note_events_play_and_release_without_allocating() {
    let mut rig = Rig::new(None, false);
    rig.run(&[], 4, false);
    assert!(peak(rig.last(4)) < 1e-6, "silent before any note");
    let on = note(CLAP_EVENT_NOTE_ON, 60, 0.8);
    rig.run(&[&on.header], 20, false);
    let playing = peak(rig.last(20));
    assert!(playing > 1e-2, "the note on plays: peak {playing}");
    let off = note(CLAP_EVENT_NOTE_OFF, 60, 0.0);
    rig.run(&[&off.header], 200, false);
    let after = peak(rig.last(4));
    assert!(
        after < 0.01 * playing,
        "the note off releases it: peak {after} after {playing}"
    );
}

fn fixture() -> SoundState {
    let bytes = include_bytes!("../../../packaging/windows/vst3/host/midi-fixture.json");
    SoundState::decode(bytes).expect("fixture decodes")
}

/// The scenario the VST3 host check in CI plays (packaging/windows/vst3/host/host.cpp), with
/// the MIDI bytes clap-wrapper makes of its IMidiMapping parameters, so its thresholds hold
/// for the CLAP side on its own.
#[test]
fn the_vst3_host_scenario_holds_for_the_midi_clap_wrapper_sends() {
    const MEASURE: usize = 48;
    let mut rig = Rig::new(Some(fixture()), true);
    rig.run(&[], 40, true);
    let on = note(CLAP_EVENT_NOTE_ON, 60, 0.8);
    rig.run(&[&on.header], 30 + MEASURE, true);
    let base = frequency(rig.last(MEASURE));
    let level = rms(rig.last(MEASURE));
    assert!(base > 250.0 && base < 275.0, "C4: {base} Hz");

    let bend = midi([0xE0, 0x7F, 0x7F]);
    rig.run(&[&bend.header], 5 + MEASURE, true);
    let bent = frequency(rig.last(MEASURE)) / base;
    assert!(bent > 1.10 && bent < 1.15, "full bend: x{bent}");
    let centre = midi([0xE0, 0x00, 0x40]);
    rig.run(&[&centre.header], 5 + MEASURE, true);
    let centred = frequency(rig.last(MEASURE)) / base;
    assert!((centred - 1.0).abs() < 0.01, "centred bend: x{centred}");

    let wheel = midi([0xB0, 1, 127]);
    rig.run(&[&wheel.header], 5 + MEASURE, true);
    let wheeled = frequency(rig.last(MEASURE)) / base;
    assert!(wheeled > 1.04 && wheeled < 1.08, "full wheel: x{wheeled}");
    let unwheel = midi([0xB0, 1, 0]);
    rig.run(&[&unwheel.header], 5 + MEASURE, true);
    let unwheeled = frequency(rig.last(MEASURE)) / base;
    assert!(
        (unwheeled - 1.0).abs() < 0.01,
        "wheel at zero: x{unwheeled}"
    );

    let low = midi([0xB0, 74, 20]);
    rig.run(&[&low.header], 20, true);
    let high = midi([0xB0, 74, 64]);
    rig.run(&[&high.header], 100 + MEASURE, true);
    let moved = frequency(rig.last(MEASURE)) / base;
    assert!(moved > 1.5, "CC 74 on base_hz: x{moved}");

    let pedal = midi([0xB0, 64, 127]);
    rig.run(&[&pedal.header], 2, true);
    let off = note(CLAP_EVENT_NOTE_OFF, 60, 0.0);
    rig.run(&[&off.header], 100, true);
    let held = rms(rig.last(20));
    assert!(held > 0.25 * level, "pedal holds: rms {held} of {level}");
    let up = midi([0xB0, 64, 0]);
    rig.run(&[&up.header], 200, true);
    let released = rms(rig.last(20));
    assert!(
        released < 0.01 * level,
        "pedal up: rms {released} of {level}"
    );
}
