//! A virtual MIDI controller for scripted real-app runs (Linux/ALSA): opens an output port named
//! `kabl-player` and plays commands read from stdin, one per line:
//!
//!     cc CH NUM VAL              channel 1..16, controller, value
//!     ramp CH NUM V0 V1 SECONDS  a turn from V0 to V1, 50 messages per second (in the background)
//!     on NOTE VEL | off NOTE     channel 1
//!     quit
//!
//! `cargo run --release -p kabl-ui --example midi_player`, then start kabl-ui with
//! `--midi kabl-player`. `docs/rack-migration/drive.py` starts it when `KABL_PLAYER` is set.
//!
//! With `KABL_MIDI_PIPE=PATH` (a machine without an ALSA sequencer, e.g. a container) it
//! creates the fifo PATH instead and writes each message there as hex bytes, one per line;
//! kabl-ui started with the same variable lists it as the input `kabl-pipe`.

use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(unix)]
fn main() {
    use midir::os::unix::VirtualOutput;
    type Out = Box<dyn FnMut(&[u8]) + Send>;
    let out: Out = match std::env::var("KABL_MIDI_PIPE") {
        Ok(path) => {
            let _ = std::fs::remove_file(&path);
            let made = std::process::Command::new("mkfifo").arg(&path).status();
            assert!(made.is_ok_and(|s| s.success()), "mkfifo {path}");
            eprintln!("midi_player: fifo {path}, waiting for kabl-ui");
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .expect("fifo");
            Box::new(move |msg: &[u8]| {
                use std::io::Write;
                let line: Vec<String> = msg.iter().map(|b| format!("{b:02x}")).collect();
                let _ = writeln!(f, "{}", line.join(" "));
            })
        }
        Err(_) => {
            let out = midir::MidiOutput::new("kabl-player").expect("MIDI output");
            let mut conn = out.create_virtual("kabl-player").expect("virtual port");
            Box::new(move |msg: &[u8]| {
                let _ = conn.send(msg);
            })
        }
    };
    let conn = Arc::new(Mutex::new(out));
    let send = |conn: &Arc<Mutex<Out>>, msg: &[u8]| {
        (conn.lock().unwrap())(msg);
    };
    eprintln!("midi_player: port kabl-player open");
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let w: Vec<&str> = line.split_whitespace().collect();
        let n = |i: usize| w.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
        match w.first().copied() {
            Some("cc") => send(
                &conn,
                &[
                    0xB0 | (n(1) as u8 - 1),
                    n(2) as u8,
                    n(3).clamp(0.0, 127.0) as u8,
                ],
            ),
            Some("ramp") => {
                let (ch, num, v0, v1, secs) = (n(1) as u8 - 1, n(2) as u8, n(3), n(4), n(5));
                let conn = conn.clone();
                std::thread::spawn(move || {
                    let steps = (secs * 50.0).max(1.0) as usize;
                    let mut last = None;
                    for k in 0..=steps {
                        let v = (v0 + (v1 - v0) * k as f32 / steps as f32).round() as u8;
                        if last != Some(v) {
                            (conn.lock().unwrap())(&[0xB0 | ch, num, v.min(127)]);
                            last = Some(v);
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                });
            }
            Some("on") => send(&conn, &[0x90, n(1) as u8, n(2) as u8]),
            Some("off") => send(&conn, &[0x80, n(1) as u8, 0]),
            Some("quit") => break,
            _ => eprintln!("midi_player: ? {line}"),
        }
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("midi_player needs ALSA/CoreMIDI virtual ports");
}
