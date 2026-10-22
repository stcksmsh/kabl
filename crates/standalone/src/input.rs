//! Audio input for the standalone: the default input device, opened on request (`--input`),
//! feeding the engine's `audio.in` modules.
//!
//! The input callback pushes stereo frames into a lock-free queue; the output callback takes
//! one engine block from it before each block it renders. The device is untrusted: it may not
//! exist, may not offer the output's sample rate, may deliver nothing for a while, or may
//! vanish while playing. In every case the patch hears silence and nothing else changes: the
//! output keeps playing. The queue is trimmed to `MAX_LAG` frames so a late-starting or
//! stalled input does not leave the voice behind the keyboard.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Queue capacity, frames.
const CAPACITY: usize = 16384;
/// The most input kept waiting, frames (about 43 ms at 48 kHz).
pub const MAX_LAG: usize = 2048;

/// An open (or failed) input. Keep it alive for as long as the output stream runs.
pub struct AudioInput {
    /// `None` when no stream is running; `status` says why.
    pub stream: Option<cpal::Stream>,
    pub rx: rtrb::Consumer<[f32; 2]>,
    /// Set by the stream's error callback when the device goes away.
    pub lost: Arc<AtomicBool>,
    pub status: String,
}

impl AudioInput {
    /// A closed input: every block it feeds is silence.
    pub fn closed(status: String) -> Self {
        let (_, rx) = rtrb::RingBuffer::new(1);
        AudioInput {
            stream: None,
            rx,
            lost: Arc::new(AtomicBool::new(false)),
            status,
        }
    }

    /// Opens the default input device at `rate`. Never fails: a missing device, a rate it does
    /// not offer or a stream that will not start gives a closed input and a status line.
    pub fn open(rate: u32) -> Self {
        let host = cpal::default_host();
        let Some(device) = host.default_input_device() else {
            return Self::closed("no audio input device".into());
        };
        let name = device
            .description()
            .map_or("unknown input".into(), |d| d.name().to_string());
        let Some(config) = device.supported_input_configs().ok().and_then(|cs| {
            cs.filter(|c| c.sample_format() == cpal::SampleFormat::F32)
                .filter_map(|c| c.try_with_sample_rate(rate))
                .max_by_key(|c| (c.channels() == 2, c.channels()))
        }) else {
            return Self::closed(format!("input \"{name}\" has no f32 config at {rate} Hz"));
        };
        let channels = config.channels() as usize;
        let (mut tx, rx) = rtrb::RingBuffer::<[f32; 2]>::new(CAPACITY);
        let lost = Arc::new(AtomicBool::new(false));
        let lost_cb = lost.clone();
        let stream = device.build_input_stream(
            config.config(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Audio thread: no allocation. A full queue drops the frame.
                for frame in data.chunks(channels) {
                    let l = frame[0];
                    let _ = tx.push([l, frame.get(1).copied().unwrap_or(l)]);
                }
            },
            move |_| lost_cb.store(true, Ordering::Relaxed),
            None,
        );
        match stream.and_then(|s| s.play().map(|()| s)) {
            Ok(s) => {
                log::info!(target: "audio", "input \"{name}\" at {rate} Hz, {channels} ch");
                AudioInput {
                    stream: Some(s),
                    rx,
                    lost,
                    status: format!("input \"{name}\" at {rate} Hz, {channels} ch"),
                }
            }
            Err(err) => Self::closed(format!("input \"{name}\" failed to start: {err}")),
        }
    }
}

/// Fills `left`/`right` with the next frames of `rx`; whatever is missing (the input is late,
/// closed or gone) is silence. First drops frames older than `MAX_LAG`. No allocation.
pub fn take_block(rx: &mut rtrb::Consumer<[f32; 2]>, left: &mut [f32], right: &mut [f32]) {
    while rx.slots() > MAX_LAG {
        let _ = rx.pop();
    }
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let [a, b] = rx.pop().unwrap_or([0.0; 2]);
        *l = a;
        *r = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_queue_pads_with_silence_and_an_empty_one_is_silent() {
        let (mut tx, mut rx) = rtrb::RingBuffer::<[f32; 2]>::new(64);
        for i in 0..3 {
            tx.push([i as f32 + 1.0, -(i as f32) - 1.0]).unwrap();
        }
        let (mut l, mut r) = ([9.0; 5], [9.0; 5]);
        take_block(&mut rx, &mut l, &mut r);
        assert_eq!(l, [1.0, 2.0, 3.0, 0.0, 0.0]);
        assert_eq!(r, [-1.0, -2.0, -3.0, 0.0, 0.0]);
        take_block(&mut rx, &mut l, &mut r);
        assert_eq!((l, r), ([0.0; 5], [0.0; 5]));
    }

    #[test]
    fn a_backlog_is_trimmed_to_the_newest_frames() {
        let (mut tx, mut rx) = rtrb::RingBuffer::<[f32; 2]>::new(CAPACITY);
        for i in 0..CAPACITY {
            tx.push([i as f32, 0.0]).unwrap();
        }
        let (mut l, mut r) = ([0.0; 4], [0.0; 4]);
        take_block(&mut rx, &mut l, &mut r);
        assert_eq!(l[0], (CAPACITY - MAX_LAG) as f32);
    }

    #[test]
    fn a_closed_input_feeds_silence() {
        let mut input = AudioInput::closed("test".into());
        let (mut l, mut r) = ([1.0; 64], [1.0; 64]);
        take_block(&mut input.rx, &mut l, &mut r);
        assert!(l.iter().chain(&r).all(|&v| v == 0.0));
    }
}
