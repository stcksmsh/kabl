//! The editor's own notes (the key strip, `kabl_ui::keys`) into the audio path: a small ring
//! from the UI thread, merged with the host's MIDI at the start of each block. They play as a
//! controller on channel 1, a source of their own, so a host note-off never ends a note the
//! strip holds and the other way round.

use crate::Instrument;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::timeline::Timeline;
use std::sync::atomic::Ordering;

/// Events the ring holds; the strip makes a handful per frame.
pub const CAPACITY: usize = 256;

pub fn ring() -> (rtrb::Producer<KeyEvent>, rtrb::Consumer<KeyEvent>) {
    rtrb::RingBuffer::new(CAPACITY)
}

/// UI thread: what the editor's keys played this frame. What does not fit (the audio thread
/// is not running) waits for the next frame, so no note-off is lost.
pub fn send(tx: &mut rtrb::Producer<KeyEvent>, view: &mut kabl_ui::UiState) {
    let n = view.keys.out.len().min(tx.slots());
    for e in view.keys.out.drain(..n) {
        tx.push(e).expect("checked free slots");
    }
}

/// Audio thread, no allocation: the queued notes at the start of this block.
pub fn merge(rx: &mut rtrb::Consumer<KeyEvent>, timeline: &mut Timeline, frames: usize) {
    for _ in 0..CAPACITY {
        let Ok(e) = rx.pop() else {
            break;
        };
        timeline.push(0, frames, MidiEvent::new(Source::Controller, 0, e));
    }
}

impl Instrument {
    /// Audio thread, no allocation: this block's host MIDI and the editor's key strip into
    /// the timeline; CCs to the control thread.
    pub(crate) fn take_midi(&mut self, frames: usize) {
        for _ in 0..1024 {
            let Ok((offset, bytes)) = self.midi.pop() else {
                break;
            };
            if bytes[0] & 0xF0 == 0xB0 && !matches!(bytes[1], 64 | 120 | 121 | 123) {
                if let Some(s) = &mut self.session {
                    // A full queue drops the CC: the control thread is behind and catches up.
                    let _ = s.cc.push((bytes[0] & 15, bytes[1], bytes[2]));
                }
            }
            if let Some(event) = MidiEvent::parse(Source::Host, &bytes) {
                self.timeline.push(offset as usize, frames, event);
            }
        }
        if self.shared.midi_overflow.swap(false, Ordering::Relaxed) {
            self.timeline.push(
                frames.saturating_sub(1),
                frames,
                MidiEvent::new(Source::Host, 0, kabl_engine::keyboard::KeyEvent::SourceLost),
            );
        }
        merge(&mut self.ui_notes, &mut self.timeline, frames);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::instrument;
    use crate::Control;
    use assert_no_alloc::assert_no_alloc;

    /// The editor's key strip plays through its own ring, merged with the host's MIDI in the
    /// audio thread without allocating; each source releases only its own notes.
    #[test]
    fn strip_notes_merge_with_host_midi() {
        let mut p = instrument();
        let (mut host, rx) = rtrb::RingBuffer::new(1024);
        p.midi = rx;
        host.push((0, [0x90, 60, 100])).unwrap();
        {
            let mut c = p.shared.control.lock().unwrap();
            let Control { view, ui_notes, .. } = &mut *c;
            view.keys.strip_to(Some(67));
            send(ui_notes, view);
            assert!(view.keys.out.is_empty());
        }
        let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
        let mut block = |p: &mut Instrument| {
            assert_no_alloc(|| {
                p.take_midi(256);
                p.render(&mut l, &mut r);
            });
            l.iter().map(|x| x.abs()).fold(0.0f32, f32::max)
        };
        let mut peak = 0.0f32;
        for _ in 0..8 {
            peak = peak.max(block(&mut p));
        }
        let held = |p: &Instrument| p.session.as_ref().unwrap().engine.keys().0;
        assert_eq!(held(&p), 2, "host key and strip key both held");
        assert!(peak > 1e-4, "silent: {peak}");
        {
            let mut c = p.shared.control.lock().unwrap();
            let Control { view, ui_notes, .. } = &mut *c;
            view.keys.strip_to(None);
            send(ui_notes, view);
        }
        block(&mut p);
        assert_eq!(held(&p), 1, "the strip's release leaves the host's key");
        host.push((0, [0x80, 60, 0])).unwrap();
        block(&mut p);
        assert_eq!(held(&p), 0);
    }
}
