//! Audible preview of a library sound (UI spec section 8).
//!
//! **Clip source.** Each sound's clip is rendered offline, once, on a worker thread: the sound's
//! own patch is compiled into a private `PatchEngine`, a short category phrase is played into
//! its keyboard (a sequence sound just runs its clocks), and the result is levelled to one
//! loudness. The clip is cached for the session. This works for every sound, factory or user,
//! needs no shipped audio and never goes stale; the cost is a render of at most `MAX_SECS`
//! seconds, started when a sound is selected, long before most people press play.
//!
//! **Output.** The clip is handed to the audio callback of the app's own output stream
//! (`Voice`, mixed on top of the rack), so it uses the same device, level meter and recorder
//! as everything else. It never touches the rack's engine.

use crate::library::Meta;
use basedrop::{Handle, Shared};
use kabl_core::PatchState;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::Transport;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

/// Longest preview, seconds (spec 8: at most 4 s).
pub const MAX_SECS: f32 = 4.0;
/// Peaks kept for the waveform well.
pub const WAVE_BINS: usize = 160;
/// Every clip is levelled to this RMS (dBFS) over its sounding part ...
const TARGET_RMS_DB: f32 = -20.0;
/// ... unless that would push its peak above this.
const CEILING: f32 = 0.89;
/// Renders at once (each is one short-lived thread).
const MAX_WORKERS: usize = 2;
/// Rendered clips kept (about 1.5 MB each at 48 kHz); the least recently used go first.
pub const MAX_CACHED: usize = 24;

/// A rendered preview: stereo interleaved samples plus what the UI draws.
pub struct Clip {
    pub rate: u32,
    pub samples: Vec<f32>,
    /// Peak per bin, 0..=1 of the loudest bin.
    pub wave: Vec<f32>,
    /// Gain applied to match loudness, dB.
    pub gain_db: f32,
    /// Tempo of the sound's clock, when it has one.
    pub bpm: Option<f32>,
}

impl Clip {
    pub fn secs(&self) -> f32 {
        self.samples.len() as f32 / 2.0 / self.rate as f32
    }

    /// The preview's length as the detail pane says it: bars and tempo for a clocked sound
    /// (4/4), seconds otherwise.
    pub fn length_text(&self) -> String {
        match self.bpm {
            Some(bpm) => {
                let bars = (self.secs() * bpm / 240.0).round().max(1.0) as u32;
                format!(
                    "{bars} {} · {bpm:.0} BPM",
                    if bars == 1 { "bar" } else { "bars" }
                )
            }
            None => format!("{:.1} s", self.secs()),
        }
    }
}

/// One note of a phrase: seconds from the start, length, MIDI note.
type Note = (f32, f32, u8);

/// The phrase played into a keyboard sound, one per shelf (a fallback for any sound).
fn phrase(shelf: Option<&str>) -> &'static [Note] {
    match shelf {
        Some("Bass") => &[
            (0.0, 0.5, 36),
            (0.6, 0.3, 36),
            (1.0, 0.5, 43),
            (1.6, 0.3, 41),
            (2.0, 0.5, 36),
            (2.6, 0.3, 39),
            (3.0, 0.4, 34),
        ],
        Some("Lead") => &[
            (0.0, 0.4, 67),
            (0.5, 0.4, 72),
            (1.0, 0.4, 71),
            (1.5, 0.4, 67),
            (2.0, 0.8, 74),
            (3.0, 0.5, 72),
        ],
        Some("Keys") | Some("Pluck") => &[
            (0.0, 0.5, 60),
            (0.0, 0.5, 64),
            (0.0, 0.5, 67),
            (1.0, 0.5, 62),
            (1.0, 0.5, 65),
            (1.0, 0.5, 69),
            (2.0, 1.0, 60),
            (2.0, 1.0, 64),
            (2.0, 1.0, 67),
            (2.0, 1.0, 72),
        ],
        Some("Pad") | Some("Voice") => &[
            (0.0, 3.0, 57),
            (0.0, 3.0, 60),
            (0.0, 3.0, 64),
            (0.0, 3.0, 67),
        ],
        _ => &[
            (0.0, 0.8, 60),
            (0.0, 0.8, 64),
            (0.0, 0.8, 67),
            (1.2, 0.8, 55),
            (1.2, 0.8, 59),
            (1.2, 0.8, 62),
            (2.4, 1.2, 60),
            (2.4, 1.2, 64),
            (2.4, 1.2, 67),
        ],
    }
}

/// Renders the clip of the sound in `file`. Runs on a worker thread; touches no shared state
/// (a sound that carries code is not rendered: loading it asks first).
pub fn render(file: &Path, meta: &Meta, rate: f32) -> Result<Clip, String> {
    let (log, _, code) =
        kabl_core::format::load_kabl_full(file).map_err(|e| format!("unreadable: {e:?}"))?;
    if !code.is_empty() {
        return Err("runs custom code: load it to hear it".into());
    }
    render_state(log.state(), meta.shelf(), rate)
}

/// The render itself, from a state (tests call this).
pub fn render_state(state: &PatchState, shelf: Option<&str>, rate: f32) -> Result<Clip, String> {
    let collector = basedrop::Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, rate, 8)
        .map_err(|e| format!("does not compile: {e}"))?;
    let (keys, sequence) = Meta::play_of(state);
    if !keys && !sequence {
        return Err("makes no sound on its own".into());
    }
    // (frame, on?, note)
    let mut events: Vec<(usize, bool, u8)> = Vec::new();
    if sequence {
        for (&id, m) in &state.modules {
            if m.kind == "clock" {
                engine.transport(id, Transport::Restart);
            }
        }
    }
    if keys {
        // Leave the last 0.5 s for the release tail.
        for &(t, len, note) in phrase(shelf) {
            let on = (t * rate) as usize;
            let off = (((t + len).min(MAX_SECS - 0.5)) * rate) as usize;
            events.push((on, true, note));
            events.push((off, false, note));
        }
        events.sort_by_key(|e| (e.0, e.1));
    }
    let total = (MAX_SECS * rate) as usize / BLOCK * BLOCK;
    let mut samples = Vec::with_capacity(total * 2);
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut next = 0;
    let mut at = 0;
    while at < total {
        while next < events.len() && events[next].0 < at + BLOCK {
            let (_, on, note) = events[next];
            let ev = if on {
                KeyEvent::On { note, velocity: 96 }
            } else {
                KeyEvent::Off { note }
            };
            engine.key_at(MidiEvent::new(Source::Preview, 0, ev), 0);
            next += 1;
        }
        engine.process_block(&mut l, &mut r);
        for i in 0..BLOCK {
            samples.push(l[i]);
            samples.push(r[i]);
        }
        at += BLOCK;
    }
    let bpm = state
        .modules
        .values()
        .find(|m| m.kind == "clock")
        .map(|m| m.params.get("bpm").copied().unwrap_or(120.0));
    finish(samples, rate as u32, bpm)
}

/// Levels and fades a raw render and draws its peaks.
fn finish(mut samples: Vec<f32>, rate: u32, bpm: Option<f32>) -> Result<Clip, String> {
    if samples.iter().any(|s| !s.is_finite()) {
        return Err("rendered a non-finite sample".into());
    }
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak < 1e-4 {
        return Err("rendered silence".into());
    }
    // RMS over the 50 ms windows above -50 dBFS: the sounding part, not the gaps.
    let win = (rate as usize / 20) * 2;
    let (mut sum, mut n) = (0f64, 0usize);
    for w in samples.chunks(win) {
        let ms = w.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / w.len() as f64;
        if 10.0 * ms.max(1e-20).log10() > -50.0 {
            sum += ms;
            n += 1;
        }
    }
    let rms_db = if n == 0 {
        20.0 * peak.log10()
    } else {
        10.0 * (sum / n as f64).log10() as f32
    };
    let mut gain = 10f32.powf((TARGET_RMS_DB - rms_db) / 20.0).clamp(0.05, 8.0);
    gain = gain.min(CEILING / peak);
    for s in &mut samples {
        *s *= gain;
    }
    // A 60 ms fade so a hard 4 s cut never clicks.
    let fade = (rate as usize * 6 / 100).min(samples.len() / 2);
    let len = samples.len();
    for i in 0..fade {
        let g = i as f32 / fade as f32;
        samples[len - 2 - 2 * i] *= g;
        samples[len - 1 - 2 * i] *= g;
    }
    let per = (samples.len() / 2).div_ceil(WAVE_BINS).max(1);
    let mut wave: Vec<f32> = samples
        .chunks(per * 2)
        .map(|c| c.iter().fold(0f32, |m, s| m.max(s.abs())))
        .collect();
    let top = wave.iter().fold(0f32, |m, v| m.max(*v)).max(1e-6);
    for v in &mut wave {
        *v /= top;
    }
    Ok(Clip {
        rate,
        samples,
        wave,
        gain_db: 20.0 * gain.log10(),
        bpm,
    })
}

// ---- the audio side -------------------------------------------------------------------------

/// What the audio callback reports back; read by the UI, written by the callback only. One per
/// channel: several instances in one process (a plugin host) never share it.
#[derive(Default)]
pub struct Status {
    /// Frames played of the current clip.
    pub pos: AtomicU32,
    /// Frames in the current clip (0 = none).
    pub len: AtomicU32,
    /// Peak of the last callback, left and right, as f32 bits.
    pub level: [AtomicU32; 2],
    /// Which play request is sounding (`Player::play`'s generation).
    pub gen: AtomicU32,
    pub active: AtomicBool,
}

pub enum Msg {
    Play(Shared<Vec<f32>>, u32),
    Stop,
}

/// The audio callback's half: plays at most one clip, on top of the rack.
pub struct Voice {
    rx: rtrb::Consumer<Msg>,
    clip: Option<Shared<Vec<f32>>>,
    pos: usize,
    peak: [f32; 2],
    status: Arc<Status>,
}

impl Voice {
    /// Start of a callback: take new requests (no allocation).
    pub fn poll(&mut self) {
        while let Ok(m) = self.rx.pop() {
            match m {
                Msg::Play(c, gen) => {
                    self.status
                        .len
                        .store((c.len() / 2) as u32, Ordering::Relaxed);
                    self.status.pos.store(0, Ordering::Relaxed);
                    self.status.gen.store(gen, Ordering::Relaxed);
                    self.status.active.store(true, Ordering::Relaxed);
                    self.clip = Some(c);
                    self.pos = 0;
                }
                Msg::Stop => self.stop(),
            }
        }
        self.peak = [0.0; 2];
    }

    fn stop(&mut self) {
        self.clip = None;
        self.status.active.store(false, Ordering::Relaxed);
    }

    /// Ends the clip now (a host whose transport runs gets no preview). No allocation.
    pub fn silence(&mut self) {
        self.stop();
    }

    /// One frame of the clip (silence when none plays).
    pub fn frame(&mut self) -> (f32, f32) {
        let Some(c) = &self.clip else {
            return (0.0, 0.0);
        };
        let i = self.pos * 2;
        if i + 1 >= c.len() {
            self.stop();
            return (0.0, 0.0);
        }
        let f = (c[i], c[i + 1]);
        self.pos += 1;
        self.peak[0] = self.peak[0].max(f.0.abs());
        self.peak[1] = self.peak[1].max(f.1.abs());
        f
    }

    /// End of a callback: report position and level.
    pub fn report(&self) {
        self.status.pos.store(self.pos as u32, Ordering::Relaxed);
        for k in 0..2 {
            self.status.level[k].store(self.peak[k].to_bits(), Ordering::Relaxed);
        }
    }
}

/// The UI side of the channel to a `Voice`.
pub struct Player {
    tx: rtrb::Producer<Msg>,
    handle: Handle,
    gen: u32,
    status: Arc<Status>,
}

/// A connected pair: the `Player` stays with the UI, the `Voice` moves into the callback.
pub fn channel(handle: Handle) -> (Player, Voice) {
    let (tx, rx) = rtrb::RingBuffer::new(8);
    let status = Arc::new(Status::default());
    (
        Player {
            tx,
            handle,
            gen: 0,
            status: status.clone(),
        },
        Voice {
            rx,
            clip: None,
            pos: 0,
            peak: [0.0; 2],
            status,
        },
    )
}

impl Player {
    /// What the audio side reports for this channel.
    pub fn status(&self) -> &Arc<Status> {
        &self.status
    }

    /// Plays `clip` from its start; the generation to recognise it by.
    pub fn play(&mut self, clip: &Clip) -> Option<u32> {
        self.gen = self.gen.wrapping_add(1);
        let shared = Shared::new(&self.handle, clip.samples.clone());
        self.tx.push(Msg::Play(shared, self.gen)).ok()?;
        Some(self.gen)
    }

    pub fn stop(&mut self) {
        let _ = self.tx.push(Msg::Stop);
    }
}

// ---- the UI side ----------------------------------------------------------------------------

enum Slot {
    Rendering(std::thread::JoinHandle<Result<Clip, String>>),
    Ready(Arc<Clip>),
    Failed(String),
}

/// Rendered clips, the playing one, and the player. Lives in the browser state.
#[derive(Default)]
pub struct Previews {
    slots: HashMap<String, (Option<SystemTime>, Slot)>,
    player: Option<Player>,
    rate: u32,
    /// The sound playing (or about to), the request's generation and when it was sent.
    playing: Option<(String, u32, Instant)>,
    /// Play this sound as soon as its clip is ready.
    want: Option<String>,
    /// The selection the browser last acted on (a new one ends the old preview).
    pub seen: Option<String>,
    /// Last use of each cached id (a counter), for eviction.
    used: HashMap<String, u64>,
    tick: u64,
}

pub enum ClipLook<'a> {
    Rendering,
    Ready(&'a Clip),
    Failed(&'a str),
    /// No audio output here (a plugin, or no device).
    NoOutput,
    Idle,
}

impl Previews {
    pub fn attach(&mut self, player: Option<Player>) {
        self.player = player;
        self.playing = None;
    }

    pub fn available(&self) -> bool {
        self.player.is_some()
    }

    /// Makes sure `id`'s clip is rendering or rendered (cheap to call every frame).
    pub fn ensure(&mut self, id: &str, file: &Path, meta: &Meta, rate: f32) {
        if self.rate != rate as u32 {
            self.slots.clear();
            self.rate = rate as u32;
        }
        self.touch(id);
        let stamp = std::fs::metadata(file).and_then(|m| m.modified()).ok();
        if let Some((s, _)) = self.slots.get(id) {
            if *s == stamp {
                return;
            }
            self.slots.remove(id);
        }
        let busy = self
            .slots
            .values()
            .filter(|(_, s)| matches!(s, Slot::Rendering(_)))
            .count();
        if busy >= MAX_WORKERS {
            return;
        }
        let (file, meta) = (file.to_path_buf(), meta.clone());
        let handle = std::thread::Builder::new()
            .name("kabl-sound-preview".into())
            .spawn(move || render(&file, &meta, rate))
            .expect("spawn one preview render worker");
        self.slots
            .insert(id.to_string(), (stamp, Slot::Rendering(handle)));
    }

    /// Collects finished renders; starts a wanted preview; clears an ended one. Every frame.
    pub fn poll(&mut self) {
        let done: Vec<String> = self
            .slots
            .iter()
            .filter(|(_, (_, s))| matches!(s, Slot::Rendering(h) if h.is_finished()))
            .map(|(k, _)| k.clone())
            .collect();
        for id in done {
            let (stamp, slot) = self.slots.remove(&id).unwrap();
            let Slot::Rendering(h) = slot else { continue };
            let slot = match h.join() {
                Ok(Ok(c)) => Slot::Ready(Arc::new(c)),
                Ok(Err(e)) => Slot::Failed(e),
                Err(_) => Slot::Failed("the render failed".into()),
            };
            self.slots.insert(id, (stamp, slot));
        }
        self.evict();
        if let Some(id) = self.want.clone() {
            match self.slots.get(&id) {
                Some((_, Slot::Ready(_))) => {
                    self.want = None;
                    self.play(&id);
                }
                Some((_, Slot::Failed(_))) | None => {
                    // Failed, or never started (all workers busy and the user moved on).
                    if matches!(self.slots.get(&id), Some((_, Slot::Failed(_)))) {
                        self.want = None;
                    }
                }
                _ => {}
            }
        }
        if let Some((_, gen, since)) = &self.playing {
            let live = self.player.as_ref().is_some_and(|p| {
                p.status.active.load(Ordering::Relaxed)
                    && p.status.gen.load(Ordering::Relaxed) == *gen
            });
            if !live && since.elapsed().as_secs_f32() > 0.3 {
                self.playing = None;
            }
        }
    }

    fn touch(&mut self, id: &str) {
        self.tick += 1;
        self.used.insert(id.to_string(), self.tick);
    }

    /// Drops the least recently used ready clips beyond `MAX_CACHED` (never the playing one).
    fn evict(&mut self) {
        while self.slots.len() > MAX_CACHED {
            let playing = self.playing().map(str::to_string);
            let victim = self
                .slots
                .iter()
                .filter(|(k, (_, s))| {
                    !matches!(s, Slot::Rendering(_)) && Some(*k) != playing.as_ref()
                })
                .min_by_key(|(k, _)| self.used.get(*k).copied().unwrap_or(0))
                .map(|(k, _)| k.clone());
            let Some(k) = victim else { break };
            self.slots.remove(&k);
            self.used.remove(&k);
        }
    }

    /// Forgets the clip of `id` (the sound was saved or changed).
    pub fn forget(&mut self, id: &str) {
        self.slots.remove(id);
        self.used.remove(id);
    }

    pub fn look(&self, id: &str) -> ClipLook<'_> {
        match self.slots.get(id) {
            Some((_, Slot::Ready(c))) => ClipLook::Ready(c),
            Some((_, Slot::Rendering(_))) => ClipLook::Rendering,
            Some((_, Slot::Failed(e))) => ClipLook::Failed(e),
            None if self.player.is_none() => ClipLook::NoOutput,
            None => ClipLook::Idle,
        }
    }

    /// Plays `id` now if its clip is ready, else as soon as it is.
    pub fn play(&mut self, id: &str) {
        let Some(player) = self.player.as_mut() else {
            return;
        };
        match self.slots.get(id) {
            Some((_, Slot::Ready(c))) => {
                let c = c.clone();
                if let Some(gen) = player.play(&c) {
                    self.playing = Some((id.to_string(), gen, Instant::now()));
                }
                self.want = None;
            }
            _ => self.want = Some(id.to_string()),
        }
    }

    pub fn stop(&mut self) {
        self.want = None;
        if self.playing.take().is_some() {
            if let Some(p) = self.player.as_mut() {
                p.stop();
            }
        }
    }

    /// The sound sounding now (or whose play was asked and not yet ended).
    pub fn playing(&self) -> Option<&str> {
        self.playing.as_ref().map(|p| p.0.as_str())
    }

    /// Whether `id` is playing, or waiting for its render to play.
    pub fn busy(&self, id: &str) -> bool {
        self.playing() == Some(id) || self.want.as_deref() == Some(id)
    }

    /// Playhead 0..=1 of the playing clip.
    pub fn progress(&self) -> f32 {
        let Some(s) = self.status() else { return 0.0 };
        let len = s.len.load(Ordering::Relaxed).max(1) as f32;
        (s.pos.load(Ordering::Relaxed) as f32 / len).clamp(0.0, 1.0)
    }

    /// Level 0..=1 of the last audio callback.
    pub fn level(&self) -> f32 {
        let Some(s) = self.status() else { return 0.0 };
        let l = f32::from_bits(s.level[0].load(Ordering::Relaxed));
        let r = f32::from_bits(s.level[1].load(Ordering::Relaxed));
        l.max(r).clamp(0.0, 1.0)
    }

    /// What the audio side reports (`None` without a player).
    pub fn status(&self) -> Option<&Arc<Status>> {
        self.player.as_ref().map(Player::status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> Arc<Clip> {
        Arc::new(Clip {
            rate: 48000,
            samples: vec![0.0; 4],
            wave: vec![0.0],
            gain_db: 0.0,
            bpm: None,
        })
    }

    #[test]
    fn channels_do_not_share_status() {
        // Two plugin instances in one host: one's preview never shows as the other's.
        let collector = basedrop::Collector::new();
        let (mut a, mut voice_a) = channel(collector.handle());
        let (b, mut voice_b) = channel(collector.handle());
        let clip = Clip {
            rate: 48000,
            samples: vec![0.1; 200],
            wave: vec![1.0],
            gain_db: 0.0,
            bpm: None,
        };
        a.play(&clip).unwrap();
        voice_a.poll();
        voice_b.poll();
        voice_a.report();
        voice_b.report();
        assert!(a.status().active.load(Ordering::Relaxed));
        assert!(!b.status().active.load(Ordering::Relaxed));
        assert_eq!(a.status().len.load(Ordering::Relaxed), 100);
        assert_eq!(b.status().len.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn the_cache_is_bounded_and_drops_the_least_recently_used() {
        let mut p = Previews::default();
        for i in 0..MAX_CACHED + 6 {
            let id = format!("s{i}");
            p.touch(&id);
            p.slots.insert(id, (None, Slot::Ready(clip())));
        }
        p.touch("s0");
        p.evict();
        assert_eq!(p.slots.len(), MAX_CACHED);
        assert!(p.slots.contains_key("s0"), "recently used stays");
        assert!(!p.slots.contains_key("s1"));
        p.forget("s0");
        assert!(!p.slots.contains_key("s0"));
    }
}
