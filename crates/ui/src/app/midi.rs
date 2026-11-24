//! MIDI input: ports, the callback sink, the test pipe, key and CC routing.

use super::*;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};

use kabl_ui::UiState;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A key event and when the MIDI thread received it (docs/midi-timing/design.md, section 4).
pub(crate) type Note = (std::time::Instant, MidiEvent);

/// A controller key event received now.
pub(crate) fn note_now(event: KeyEvent) -> Note {
    (
        std::time::Instant::now(),
        MidiEvent::new(Source::Controller, 0, event),
    )
}

/// What the MIDI callback owns: key events (notes, sustain pedal, All Notes Off / All Sound
/// Off) to the audio thread, where the engine's keyboards assign voices; every other CC to
/// the UI (learn, mappings, buttons).
pub(crate) struct MidiSink {
    pub(crate) notes: Option<rtrb::Producer<Note>>,
    /// CCs with their arrival time (`clock_s`).
    pub(crate) cc: rtrb::Producer<((u8, u8, u8), f64)>,
    pub(crate) ctx: Option<egui::Context>,
    /// The control thread (`pump`), woken for each CC.
    pub(crate) pump: Option<std::thread::Thread>,
}

/// CCs that are key events, never learnable: sustain pedal, All Sound Off, Reset All
/// Controllers, All Notes Off. CC 1 is both the wheel and learnable (design.md section 7).
pub(crate) fn is_key_cc(cc: u8) -> bool {
    matches!(cc, 64 | 120 | 121 | 123)
}

/// Keys held and voices sounding, as the audio thread publishes them.
pub(crate) fn pack_keys(held: usize, sounding: usize) -> u64 {
    ((held as u64) << 32) | sounding as u64
}

pub(crate) fn unpack_keys(v: u64) -> (u64, u64) {
    (v >> 32, v & 0xFFFF_FFFF)
}

/// One incoming MIDI message: a key event (notes, bend, wheel, pedal, notes off, reset) to the
/// audio thread with its arrival time; any other CC, and CC 1 as well, to the UI.
pub(crate) fn on_message(s: &mut MidiSink, data: &[u8]) {
    if let Some(e) = MidiEvent::parse(Source::Controller, data) {
        if let Some(notes) = s.notes.as_mut() {
            let _ = notes.push((std::time::Instant::now(), e));
        }
    }
    if data.len() >= 3 && data[0] & 0xF0 == 0xB0 && !is_key_cc(data[1]) {
        let _ = s.cc.push(((data[0] & 0x0F, data[1], data[2]), clock_s()));
        if let Some(p) = &s.pump {
            p.unpark();
        }
        if let Some(ctx) = &s.ctx {
            ctx.request_repaint();
        }
    }
}

/// Test hook for machines without an ALSA sequencer (a container): `KABL_MIDI_PIPE=PATH`
/// lists the input `kabl-pipe` while the fifo PATH exists (`examples/midi_player` makes it
/// and writes hex bytes, one message per line). Connecting reads it on a thread through the
/// same `on_message` as a real port; removing the fifo is an unplug.
pub(crate) const PIPE_PORT: &str = "kabl-pipe";

pub(crate) fn pipe_path() -> Option<String> {
    std::env::var("KABL_MIDI_PIPE").ok()
}

pub(crate) fn read_pipe(
    path: String,
    sink: Arc<Mutex<MidiSink>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    use std::io::BufRead;
    while !stop.load(Ordering::Acquire) {
        let Ok(f) = std::fs::File::open(&path) else {
            return;
        };
        for line in std::io::BufReader::new(f).lines() {
            let Ok(line) = line else { break };
            if stop.load(Ordering::Acquire) {
                return;
            }
            let data: Vec<u8> = line
                .split_whitespace()
                .filter_map(|b| u8::from_str_radix(b, 16).ok())
                .collect();
            if let Ok(mut s) = sink.lock() {
                on_message(&mut s, &data);
            }
        }
    }
}

/// The MIDI input connection. The sink outlives connections, so switching ports keeps the
/// queues; a switch releases every voice first so no note hangs.
pub(crate) struct Midi {
    pub(crate) sink: Arc<Mutex<MidiSink>>,
    pub(crate) conn: Option<midir::MidiInputConnection<Arc<Mutex<MidiSink>>>>,
    /// Stop flag of the `KABL_MIDI_PIPE` reader, when that is the input.
    pub(crate) pipe: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub(crate) port: Option<String>,
    /// Keys held and voices sounding (`pack_keys`), from the audio thread.
    pub(crate) keys: Arc<AtomicU64>,
    pub(crate) ports: Vec<String>,
    pub(crate) scanned: Option<std::time::Instant>,
    /// The port that disappeared while connected; reconnected when it comes back.
    pub(crate) lost: Option<String>,
}

pub(crate) fn input_names() -> Vec<String> {
    let mut names: Vec<String> = match midir::MidiInput::new("kabl-ui-scan") {
        Ok(m) => m
            .ports()
            .iter()
            .filter_map(|p| m.port_name(p).ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    if pipe_path().is_some_and(|p| std::path::Path::new(&p).exists()) {
        names.push(PIPE_PORT.into());
    }
    names
}

/// Incoming CC messages `((channel, controller, value), arrival time)` for the control thread.
pub(crate) type CcQueue = rtrb::Consumer<((u8, u8, u8), f64)>;

impl Midi {
    /// Also returns the CC queue's consumer, for the control thread.
    pub(crate) fn new(notes: rtrb::Producer<Note>) -> (Self, CcQueue) {
        let (cc, cc_rx) = rtrb::RingBuffer::new(1024);
        let m = Midi {
            sink: Arc::new(Mutex::new(MidiSink {
                notes: Some(notes),
                cc,
                ctx: None,
                pump: None,
            })),
            conn: None,
            pipe: None,
            port: None,
            keys: Arc::new(AtomicU64::new(0)),
            ports: Vec::new(),
            scanned: None,
            lost: None,
        };
        (m, cc_rx)
    }

    pub(crate) fn disconnect(&mut self) {
        if let Some(p) = &self.port {
            log::info!(target: "midi", "disconnect \"{p}\"");
        }
        if let Some(c) = self.conn.take() {
            c.close();
        }
        if let Some(stop) = self.pipe.take() {
            stop.store(true, Ordering::Release);
        }
        // Only a controller that was connected leaves notes to end.
        if self.port.take().is_some() {
            self.send(KeyEvent::SourceLost);
        }
    }

    /// The controller's notes, pedals and expression end; a running preview does not
    /// (docs/midi-timing/design.md, "Cleanup").
    pub(crate) fn send(&self, e: KeyEvent) {
        let mut s = self.sink.lock().unwrap();
        if let Some(notes) = s.notes.as_mut() {
            let _ = notes.push(note_now(e));
        }
    }

    /// Every keyboard releases every voice and forgets its keys and the pedal
    /// (docs/sound-palette-batch/keyboard.md, "Other events").
    pub(crate) fn release_all(&self) {
        self.send(KeyEvent::AllOff);
    }

    /// Notes from the computer keys and the key strip take the controller's path, channel 1,
    /// so a keyboard patch plays them as it would a MIDI keyboard.
    pub(crate) fn send_keys(&self, ui_state: &mut UiState) {
        if ui_state.keys.out.is_empty() {
            return;
        }
        let mut s = self.sink.lock().unwrap();
        // No queue: an audio restart is under way and ends every live note anyway.
        let Some(notes) = s.notes.as_mut() else {
            ui_state.keys.out.clear();
            return;
        };
        // What does not fit waits for the next frame, so no note-off is lost.
        let n = ui_state.keys.out.len().min(notes.slots());
        for e in ui_state.keys.out.drain(..n) {
            notes.push(note_now(e)).expect("checked free slots");
        }
    }

    /// Voices sounding and available, for the status bar.
    pub(crate) fn voices(&self) -> (u32, u32) {
        let (_, sounding) = unpack_keys(self.keys.load(Ordering::Relaxed));
        (sounding as u32, kabl_standalone::DEFAULT_VOICE_COUNT as u32)
    }

    pub(crate) fn suspend_notes(&self) {
        self.sink.lock().unwrap().notes = None;
        self.keys.store(0, Ordering::Relaxed);
    }

    pub(crate) fn resume_notes(&self, notes: rtrb::Producer<Note>) {
        self.sink.lock().unwrap().notes = Some(notes);
    }

    pub(crate) fn connect(&mut self, name: &str) -> Result<(), String> {
        self.disconnect();
        if let Some(path) = pipe_path().filter(|_| name == PIPE_PORT) {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let (sink, flag) = (self.sink.clone(), stop.clone());
            std::thread::spawn(move || read_pipe(path, sink, flag));
            log::info!(target: "midi", "listening for MIDI on \"{name}\" (test pipe)");
            self.pipe = Some(stop);
            self.port = Some(name.to_string());
            return Ok(());
        }
        let midi_in = midir::MidiInput::new("kabl-ui").map_err(|e| e.to_string())?;
        let port = midi_in
            .ports()
            .into_iter()
            .find(|p| midi_in.port_name(p).is_ok_and(|n| n == name))
            .ok_or_else(|| format!("MIDI input \"{name}\" is gone"))?;
        let conn = midi_in
            .connect(
                &port,
                "kabl-input",
                |_stamp, data, sink: &mut Arc<Mutex<MidiSink>>| {
                    if let Ok(mut s) = sink.lock() {
                        on_message(&mut s, data);
                    }
                },
                self.sink.clone(),
            )
            .map_err(|e| e.to_string())?;
        log::info!(target: "midi", "listening for MIDI on \"{name}\"");
        self.conn = Some(conn);
        self.port = Some(name.to_string());
        Ok(())
    }

    /// The first port whose name contains `filter` (case-insensitive), else the first that isn't
    /// ALSA's own loopback.
    pub(crate) fn connect_default(&mut self, filter: Option<&str>) {
        let names = input_names();
        let pick = match filter {
            Some(f) => names
                .iter()
                .find(|n| n.to_lowercase().contains(&f.to_lowercase())),
            None => names.iter().find(|n| !n.contains("Through")),
        };
        match pick {
            Some(n) => {
                if let Err(e) = self.connect(&n.clone()) {
                    log::warn!(target: "midi", "{e}");
                }
            }
            None => log::info!(target: "midi", "no MIDI input matched {filter:?}"),
        }
        self.ports = names;
    }
}

/// A MIDI port name without ALSA's trailing `client:port` numbers.
pub(crate) fn base_name(name: &str) -> &str {
    match name.rsplit_once(' ') {
        Some((head, tail))
            if tail.split(':').count() == 2
                && tail
                    .split(':')
                    .all(|p| p.chars().all(|c| c.is_ascii_digit())) =>
        {
            head
        }
        _ => name,
    }
}

/// Every MIDI mapping waits for pickup again (new or reconnected hardware).
pub(crate) fn reset_pickup(ui: &mut UiState) {
    ui.button_rearm = true;
    for t in ui.takeover.values_mut() {
        *t = Default::default();
    }
}
