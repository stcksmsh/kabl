//! D06 offline render: plays a timestamped MIDI file through the production `Timeline`
//! adapter and writes a stereo 32-bit float WAV, with its hash, peak and active duration.
//!
//!     cargo run --release -p kabl-engine --example render_midi -- PATCH EVENTS OUT.wav \
//!         [--frames N | --frames irregular] [--rate HZ] [--tail SECONDS] [--policy grid|pre-d06]
//!
//! EVENTS: one message per line, `SECONDS HEX...` (e.g. `1.25 90 3c 64`), `#` comments. The
//! render lasts until the last event plus the tail (default 4 s), then drops the adapter's
//! fixed `LATENCY` frames from the start so sample 0 is performance time 0.
//!
//! `--policy pre-d06` emulates the standalone app before D06 at 256-frame callbacks: an event
//! is applied at the start of the next callback after it arrives (0–5.3 ms late at 48 kHz,
//! varying with arrival phase), for listening comparisons only.
//!
//! Active duration: 50 ms windows whose RMS (both channels) is above -50 dBFS.

use basedrop::Collector;
use kabl_engine::keyboard::{MidiEvent, Source};
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::timeline::{Timeline, LATENCY};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (patch, events, out) = (&args[0], &args[1], &args[2]);
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .map(|i| args[i + 1].clone())
    };
    let rate: f32 = opt("--rate").map_or(48000.0, |r| r.parse().unwrap());
    let tail: f32 = opt("--tail").map_or(4.0, |t| t.parse().unwrap());
    let frames = opt("--frames").unwrap_or("256".into());
    let pre_d06 = opt("--policy").as_deref() == Some("pre-d06");

    let mut script: Vec<(u64, MidiEvent)> = std::fs::read_to_string(events)
        .expect("events file")
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let t: f64 = w.next()?.parse().ok()?;
            let bytes: Vec<u8> = w.filter_map(|b| u8::from_str_radix(b, 16).ok()).collect();
            let e = MidiEvent::parse(Source::Controller, &bytes)?;
            let mut at = (t * rate as f64).round() as u64;
            if pre_d06 {
                at = (at / 256 + 1) * 256;
            }
            Some((at, e))
        })
        .collect();
    script.sort_by_key(|e| e.0);
    let end = script.last().map_or(0, |e| e.0) + (tail * rate) as u64 + LATENCY as u64;

    let log = kabl_core::load(std::path::Path::new(patch)).expect("patch");
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), log.state(), rate, 8).expect("compiles");
    let mut timeline = Box::new(Timeline::new());
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut size = || match frames.as_str() {
        "irregular" => {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % 700) as usize + 1
        }
        n => n.parse().unwrap(),
    };
    let (mut left, mut right) = (vec![0f32; 4096], vec![0f32; 4096]);
    let mut samples: Vec<f32> = Vec::with_capacity(end as usize * 2);
    let (mut host, mut next) = (0u64, 0usize);
    while host < end {
        let n = size().min(4096).min((end - host) as usize);
        let mut batch = Vec::new();
        while next < script.len() && script[next].0 < host + n as u64 {
            batch.push(((script[next].0 - host) as usize, script[next].1));
            next += 1;
        }
        timeline.process(
            &mut engine,
            &mut left[..n],
            &mut right[..n],
            &batch,
            |_, _| {},
        );
        for i in 0..n {
            samples.push(left[i]);
            samples.push(right[i]);
        }
        host += n as u64;
    }
    let samples = &samples[LATENCY * 2..];

    let (mut h, mut peak) = (0xcbf2_9ce4_8422_2325u64, 0f32);
    for s in samples {
        h = (h ^ s.to_bits() as u64).wrapping_mul(0x100_0000_01b3);
        peak = peak.max(s.abs());
    }
    let window = (0.05 * rate) as usize * 2;
    let active = samples
        .chunks(window)
        .filter(|w| {
            let ms = w.iter().map(|s| (s * s) as f64).sum::<f64>() / w.len() as f64;
            10.0 * ms.max(1e-20).log10() > -50.0
        })
        .count() as f32
        * 0.05;
    write_wav(out, samples, rate as u32);
    println!(
        "{out}: {:.2} s at {rate} Hz, frames {frames}, policy {}, hash {h:016x}, peak {:.1} dBFS, \
         active {active:.2} s, {} events, adapter {:?}",
        samples.len() as f32 / 2.0 / rate,
        if pre_d06 { "pre-d06" } else { "grid" },
        20.0 * peak.max(1e-9).log10(),
        script.len(),
        timeline.stats()
    );
}

fn write_wav(path: &str, samples: &[f32], rate: u32) {
    let data = samples.len() as u32 * 4;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).expect("write wav");
}
