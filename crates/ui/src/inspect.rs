//! D03 signal inspection in the drawer: one selected output, its measurements from the audio
//! thread (`kabl_engine::probe`), and the observation-led "Why no sound?" aid. Rules:
//! docs/signal-inspection/design.md.
//!
//! View state only: selecting, opening and closing never edit the patch, the undo history,
//! the saved state, the transport or the voices. The only effect outside the UI is
//! `Command::Inspect`, which turns the measurement on, moves it or turns it off.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use egui::RichText;
use kabl_core::{ModuleId, PatchState, PortRef};
use kabl_engine::patch_engine::Command;
use kabl_engine::probe::{ProbeReport, ProbeStatus, ProbeTarget, GATE_THRESHOLD, PROBE_LANES};
use kabl_modules::{registry, PortDirection, PortType};

use crate::{explain, routing, PatchEditor, UiState};

/// Reports older than this are stale: the audio thread stopped delivering them.
pub const STALE_SECS: f64 = 0.5;
/// Interval the measurements and the diagnosis summarize.
pub const SUMMARY_SECS: f64 = 1.0;
/// Audio below this peak is reported as "below −90 dBFS", not as a level.
pub const QUIET: f32 = 3.2e-5;
/// Most modules the path search visits.
pub const MAX_VISIT: usize = 512;

/// The selected output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sel {
    pub id: ModuleId,
    pub port: String,
}

/// Inspection state. View only.
#[derive(Debug, Default)]
pub struct Inspect {
    /// The drawer shows the inspector (and measures while a selection exists).
    pub open: bool,
    /// Scroll the section into view on the next frame (just opened or selected).
    pub reveal: bool,
    pub sel: Option<Sel>,
    /// The "Why no sound?" aid is open.
    pub why: bool,
    token: u64,
    /// What the engine was last asked to measure, and when (egui time).
    sent: Option<(Option<ProbeTarget>, f64)>,
    /// When the current selection started (egui time), for "waiting".
    since: f64,
    editor: u64,
    /// Newest rebuild attempt (set by `main.rs` through `rebuilt`).
    pub generation: u64,
    /// Reports from graphs older than this are not the patch on screen.
    floor: u64,
    newest: u64,
    /// The newest rebuild failed: the playing graph is an older patch.
    pub compile_error: Option<String>,
    topology: Option<Vec<u64>>,
    /// Accepted reports for the current selection with their arrival time, newest last.
    pub reports: VecDeque<(f64, ProbeReport)>,
    last_seq: Option<u64>,
    /// Reports lost on the way (sequence gaps), this selection.
    pub dropped: u64,
    /// The audio engine is running (`main.rs`); without it nothing can be measured.
    pub audio: bool,
    /// `engine_output` for a topology, so the open aid compiles once per wiring change, not
    /// per frame.
    output: Option<(Vec<u64>, EngineOutput)>,
}

impl Inspect {
    /// Measures `sel` from now on; opens the inspector.
    pub fn select(&mut self, sel: Sel, now: f64) {
        if self.sel.as_ref() != Some(&sel) {
            self.token += 1;
            self.reports.clear();
            self.last_seq = None;
            self.dropped = 0;
            self.since = now;
            log::debug!(target: "inspect", "select module={} port={}", sel.id, sel.port);
        }
        self.sel = Some(sel);
        self.open = true;
        self.reveal = true;
    }

    /// Stops measuring (the inspector itself stays as it is).
    pub fn clear(&mut self) {
        if self.sel.is_some() {
            log::debug!(target: "inspect", "clear");
        }
        self.sel = None;
        self.token += 1;
        self.reports.clear();
        self.last_seq = None;
    }

    /// `main.rs`, after each rebuild attempt: `generation` is the new graph's; `error` when it
    /// did not compile. A failure or a topology change (modules or cables added or removed)
    /// makes older measurements not describe the patch on screen.
    pub fn rebuilt(
        &mut self,
        generation: u64,
        error: Option<String>,
        state: &PatchState,
        now: f64,
    ) {
        self.generation = generation;
        let topo = topology(state);
        if error.is_some() || self.topology.as_ref() != Some(&topo) {
            self.floor = generation;
            // What was measured before describes another patch: waiting again.
            let before = self.reports.len();
            self.reports.retain(|(_, r)| r.generation >= generation);
            if self.reports.len() != before {
                self.since = now;
            }
        }
        self.topology = Some(topo);
        self.compile_error = error;
    }

    /// A report from the audio thread, `age` seconds old on the audio clock (it may have
    /// waited in the queue). Kept when it answers the current selection from the current
    /// graph lineage; returns whether it was kept.
    pub fn accept(&mut self, r: ProbeReport, now: f64, age: f64) -> bool {
        if self.sel.is_none() || r.token != self.token {
            return false;
        }
        if r.generation < self.floor || r.generation < self.newest {
            return false;
        }
        if let Some(last) = self.last_seq {
            self.dropped += r.seq.saturating_sub(last + 1);
        }
        self.last_seq = Some(r.seq);
        self.newest = r.generation;
        self.reports.push_back((now - age.max(0.0), r));
        while self.reports.len() > 64
            || self
                .reports
                .front()
                .is_some_and(|(t, _)| now - t > SUMMARY_SECS + STALE_SECS)
        {
            self.reports.pop_front();
        }
        true
    }

    /// Once per frame: follows editor replacement and returns the command to send, if the
    /// engine should measure something else (or again, after waiting too long).
    pub fn frame(&mut self, editor: &PatchEditor, drawer_open: bool, now: f64) -> Option<Command> {
        if self.editor != editor.instance() {
            if self.editor != 0 {
                // Another sound: its ids may be reused; nothing carries over.
                self.clear();
                self.why = false;
                self.topology = None;
            }
            self.editor = editor.instance();
        }
        let want = self
            .target(editor.state())
            .filter(|_| drawer_open && self.open);
        let resend = match self.sent {
            None => want.is_some(),
            Some((t, at)) => {
                t != want
                    // Waiting with nothing arriving: the command may have been lost in a full
                    // queue, so ask again (idempotent).
                    || (want.is_some()
                        && now - at > STALE_SECS
                        && self
                            .reports
                            .back()
                            .is_none_or(|(t, _)| now - t > STALE_SECS))
            }
        };
        if !resend {
            return None;
        }
        self.sent = Some((want, now));
        Some(Command::Inspect(want))
    }

    fn target(&self, state: &PatchState) -> Option<ProbeTarget> {
        let sel = self.sel.as_ref()?;
        let info = registry::info_for(&state.modules.get(&sel.id)?.kind)?;
        let port = outputs(info).position(|p| p.name == sel.port)?;
        Some(ProbeTarget {
            token: self.token,
            module: sel.id,
            port: port as u8,
            kind: info.kind,
        })
    }

    /// What the panel can say about the selection right now.
    pub fn status(&self, state: &PatchState, now: f64) -> Status {
        let Some(sel) = &self.sel else {
            return Status::NoSelection;
        };
        if self.target(state).is_none() {
            return Status::Missing(sel.id);
        }
        if !self.audio {
            return Status::NoAudio;
        }
        if let Some(e) = &self.compile_error {
            return Status::NotCompiled(e.clone());
        }
        match self.reports.back() {
            None if now - self.since > STALE_SECS * 2.0 => Status::NotArriving,
            None => Status::Waiting,
            Some((_, r)) if r.status == ProbeStatus::NotInGraph => Status::NotInGraph,
            Some((t, _)) if now - t > STALE_SECS => Status::Stale(now - t),
            Some(_) => Status::Live,
        }
    }

    /// The accepted reports of the last `SUMMARY_SECS`, combined.
    pub fn summary(&self, now: f64) -> Option<Summary> {
        let mut s: Option<Summary> = None;
        for (t, r) in self.reports.iter().filter(|(t, _)| now - t <= SUMMARY_SECS) {
            if r.status != ProbeStatus::Measured {
                continue;
            }
            let x = s.get_or_insert_with(|| Summary::new(r));
            x.add(r);
            x.newest_age = now - t;
        }
        s
    }
}

/// Modules with kinds, and cable endpoints: what makes an older graph a different patch.
fn topology(state: &PatchState) -> Vec<u64> {
    let mut v = Vec::new();
    for (&id, m) in &state.modules {
        v.push(id);
        v.push(
            m.kind
                .bytes()
                .fold(0u64, |h, b| h.wrapping_mul(31) ^ b as u64),
        );
    }
    for (&id, c) in &state.cables {
        let h = format!("{:?}{:?}", c.from, c.to)
            .bytes()
            .fold(0u64, |h, b| h.wrapping_mul(131) ^ b as u64);
        v.extend([id, h]);
    }
    v
}

fn outputs(
    info: &'static kabl_modules::ModuleInfo,
) -> impl Iterator<Item = &'static kabl_modules::PortInfo> {
    info.ports
        .iter()
        .filter(|p| p.direction == PortDirection::Output)
}

/// The type of output `port` of module `id`.
pub fn port_type(state: &PatchState, id: ModuleId, port: &str) -> Option<PortType> {
    let info = registry::info_for(&state.modules.get(&id)?.kind)?;
    outputs(info).find(|p| p.name == port).map(|p| p.port_type)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    NoSelection,
    /// The selected module or port is not in the patch (deleted, undone).
    Missing(ModuleId),
    NoAudio,
    /// The newest edit did not compile; audio plays an older version.
    NotCompiled(String),
    /// Selected, no measurement yet.
    Waiting,
    /// Selected a while ago and nothing came.
    NotArriving,
    /// The playing graph has no such output (it is still being rebuilt).
    NotInGraph,
    /// The last measurement is this old (seconds).
    Stale(f64),
    Live,
}

/// Several windows combined.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub windows: u32,
    pub samples: u32,
    pub lanes: usize,
    pub lanes_total: usize,
    pub voiced: bool,
    pub generation: u64,
    pub fading: bool,
    pub peak: [f32; PROBE_LANES],
    pub rms: [f32; PROBE_LANES],
    pub min: f32,
    pub max: f32,
    pub rises: [u32; PROBE_LANES],
    /// Lanes above the gate threshold for every sample of the interval.
    pub held: [bool; PROBE_LANES],
    /// Samples above the gate threshold, per lane.
    pub high: [u32; PROBE_LANES],
    pub last: [f32; PROBE_LANES],
    pub nonfinite: u32,
    /// Age of the newest window (s).
    pub newest_age: f64,
    /// Length of one window (samples).
    pub window: u32,
}

impl Summary {
    fn new(r: &ProbeReport) -> Self {
        Summary {
            windows: 0,
            samples: 0,
            lanes: r.lanes as usize,
            lanes_total: r.lanes_total as usize,
            voiced: r.voiced,
            generation: r.generation,
            fading: false,
            peak: [0.0; PROBE_LANES],
            rms: [0.0; PROBE_LANES],
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            rises: [0; PROBE_LANES],
            held: [true; PROBE_LANES],
            high: [0; PROBE_LANES],
            last: [0.0; PROBE_LANES],
            nonfinite: 0,
            newest_age: 0.0,
            window: r.samples,
        }
    }

    fn add(&mut self, r: &ProbeReport) {
        self.windows += 1;
        self.samples += r.samples;
        self.generation = r.generation;
        self.fading = r.fading;
        self.lanes = self.lanes.min(r.lanes as usize);
        for l in 0..self.lanes {
            let s = &r.lane[l];
            self.peak[l] = self.peak[l].max(s.peak);
            self.rms[l] = self.rms[l].max(s.rms(r.samples));
            if s.min.is_finite() {
                self.min = self.min.min(s.min);
                self.max = self.max.max(s.max);
            }
            self.rises[l] += s.rises;
            self.held[l] &= s.high == r.samples;
            self.high[l] += s.high;
            self.last[l] = s.last;
            self.nonfinite += s.nonfinite;
        }
    }

    pub fn interval(&self, sample_rate: f32) -> f64 {
        self.samples as f64 / sample_rate as f64
    }

    pub fn peak_all(&self) -> f32 {
        self.peak[..self.lanes].iter().copied().fold(0.0, f32::max)
    }

    pub fn rises_all(&self) -> u32 {
        self.rises[..self.lanes].iter().sum()
    }

    /// Lanes with any activity: a level above `QUIET`, an edge, or high at all.
    pub fn active_lanes(&self) -> usize {
        (0..self.lanes)
            .filter(|&l| self.peak[l] > QUIET || self.rises[l] > 0)
            .count()
    }
}

pub fn db(x: f32) -> String {
    if x <= QUIET {
        "below −90 dBFS".into()
    } else {
        format!("{:.1} dBFS", 20.0 * x.log10())
    }
}

/// `+7.00 st (G4)`: semitones from MIDI note 60, the engine's pitch unit.
pub fn pitch_text(st: f32) -> String {
    let note = (60.0 + st).round();
    let name = if (0.0..=127.0).contains(&note) {
        crate::browser::note_name(note as u8)
    } else {
        "out of MIDI range".into()
    };
    format!("{st:+.2} st ({name})")
}

/// Module title as the drawer writes it: `ADSR #6`.
pub fn title(state: &PatchState, id: ModuleId) -> String {
    let kind = state.modules.get(&id).map_or("", |m| m.kind.as_str());
    let short = match routing::short_name(kind) {
        "?" => registry::info_for(kind).map_or("Module", |i| i.name),
        s => s,
    };
    format!("{short} #{id}")
}

fn lane_scope(s: &Summary) -> String {
    if s.voiced {
        let extra = if s.lanes_total > s.lanes {
            format!(" (first {} of {} measured)", s.lanes, s.lanes_total)
        } else {
            String::new()
        };
        format!("{} voice lanes, each measured apart{extra}", s.lanes_total)
    } else {
        "one shared instance".into()
    }
}

/// Measurement lines for a summary of output type `t`, the tap named `name`.
pub fn measurement_lines(s: &Summary, t: PortType, name: &str, sample_rate: f32) -> Vec<String> {
    let secs = s.interval(sample_rate);
    let mut out = Vec::new();
    let during = format!(
        "over the last {secs:.1} s ({} windows of {:.0} ms)",
        s.windows,
        s.window as f64 / sample_rate as f64 * 1e3
    );
    match t {
        PortType::Gate => {
            let rises = s.rises_all();
            let held = (0..s.lanes).filter(|&l| s.held[l]).count();
            if rises == 0 {
                out.push(format!(
                    "At {name}: no rising edge above {GATE_THRESHOLD} {during}."
                ));
            } else {
                let lanes = (0..s.lanes).filter(|&l| s.rises[l] > 0).count();
                out.push(format!(
                    "At {name}: {rises} rising edge{} above {GATE_THRESHOLD} {during}{}.",
                    if rises == 1 { "" } else { "s" },
                    if s.voiced {
                        format!(", in {lanes} lane{}", if lanes == 1 { "" } else { "s" })
                    } else {
                        String::new()
                    }
                ));
            }
            if held > 0 {
                out.push(format!(
                    "{held} lane{} held high the whole time (a held gate makes no new edge).",
                    if held == 1 { "" } else { "s" }
                ));
            }
        }
        PortType::Audio => {
            out.push(format!(
                "At {name}: peak {} {during}{}.",
                db(s.peak_all()),
                if s.voiced {
                    let n = s.active_lanes();
                    format!(
                        " (loudest lane; {n} lane{} active)",
                        if n == 1 { "" } else { "s" }
                    )
                } else {
                    String::new()
                }
            ));
        }
        PortType::Pitch => {
            let vals: Vec<String> = (0..s.lanes)
                .filter(|&l| !s.voiced || s.rises[l] > 0 || s.last[l] != 0.0 || l == 0)
                .map(|l| {
                    if s.voiced {
                        format!("lane {}: {}", l + 1, pitch_text(s.last[l]))
                    } else {
                        pitch_text(s.last[l])
                    }
                })
                .collect();
            out.push(format!("At {name}: now {}.", vals.join(", ")));
            out.push(format!(
                "Range {} to {} {during}. 0 st is a valid pitch (note 60).",
                pitch_text(s.min),
                pitch_text(s.max)
            ));
        }
        PortType::Cv | PortType::UnipolarCv => {
            let range = if t == PortType::Cv {
                "−1…1"
            } else {
                "0…1"
            };
            out.push(format!(
                "At {name}: from {:.3} to {:.3} {during} (nominal {range}).",
                s.min, s.max
            ));
        }
    }
    if s.nonfinite > 0 {
        out.push(format!(
            "{} samples were not numbers (NaN or infinite).",
            s.nonfinite
        ));
    }
    out
}

/// The diagnosis, three kinds of statement kept apart.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Diagnosis {
    /// Read from the patch and the engine's compile.
    pub facts: Vec<String>,
    /// Measured at a named tap over a stated interval.
    pub measured: Vec<String>,
    /// Where to look next; not a claim.
    pub possible: Vec<String>,
    /// Suggested next tap (module, output port).
    pub next: Option<(ModuleId, String)>,
}

/// What the aid reads besides the patch.
pub struct Context<'a> {
    /// Each clock's run state as the audio thread reported it.
    pub clock_running: &'a HashMap<ModuleId, bool>,
    /// Final output meter level (peak, decaying), when audio runs.
    pub output_level: Option<f32>,
    /// The inspected tap, its type and its summary when a current one exists.
    pub tap: Option<(&'a Sel, PortType, Option<Summary>)>,
    pub midi_input: Option<&'a str>,
    pub sample_rate: f32,
    /// `engine_output(state)`, when the caller has it cached; computed otherwise.
    pub output: Option<&'a EngineOutput>,
}

/// The played `out` module, or why the patch does not compile.
pub type EngineOutput = Result<Option<ModuleId>, String>;

/// The `out` module the engine plays for `state` (the last in its schedule), or the compile
/// error. Compiles the patch: cache it per topology (`Inspect::output`).
pub fn engine_output(state: &PatchState) -> EngineOutput {
    kabl_engine::compile::compile(state, 48000.0, 1)
        .map(|c| c.output_module())
        .map_err(|e| e.to_string())
}

/// The stored base value of `param` on module `id`, else its default.
fn base(state: &PatchState, id: ModuleId, param: &str) -> Option<f32> {
    let p = explain::param_of(state, id, param)?;
    let stored = &state.modules.get(&id)?.params;
    Some(
        stored
            .get(p.name)
            .or_else(|| stored.get(param))
            .copied()
            .unwrap_or(p.default),
    )
}

/// Active (not bypassed) modulation routes into `param` of `id`.
fn routes_into(state: &PatchState, id: ModuleId, param: &str) -> usize {
    state
        .cables
        .values()
        .filter(|c| {
            matches!(&c.to, PortRef::Param { id: t, param: p } if *t == id && p == param)
                && !c.params.get("bypass").is_some_and(|&b| b >= 0.5)
        })
        .count()
}

/// Why is there no sound? Facts from the graph, what was measured, and possibilities. The
/// focus is the inspected module, else `focus`.
pub fn diagnose(state: &PatchState, focus: Option<ModuleId>, cx: &Context) -> Diagnosis {
    let mut d = Diagnosis::default();
    let computed;
    let compiled = match cx.output {
        Some(o) => o,
        None => {
            computed = engine_output(state);
            &computed
        }
    };
    let out = match compiled {
        Err(e) => {
            d.facts.push(format!(
                "The patch does not compile ({e}); audio keeps playing the last version that did."
            ));
            return d;
        }
        Ok(o) => *o,
    };
    let outs: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "out")
        .map(|(&id, _)| id)
        .collect();
    let Some(out) = out else {
        d.facts
            .push("There is no Output module, so nothing reaches the audio device.".into());
        return d;
    };
    if outs.len() > 1 {
        let others: Vec<String> = outs
            .iter()
            .filter(|&&o| o != out)
            .map(|&o| format!("#{o}"))
            .collect();
        d.facts.push(format!(
            "{} Output modules: the engine plays only Output #{out} (the last in its processing \
             order); {} not heard.",
            outs.len(),
            others.join(", ") + if others.len() == 1 { " is" } else { " are" }
        ));
    }
    let plugged = |port: &str| {
        state
            .cables
            .values()
            .any(|c| matches!(&c.to, PortRef::Module { id, port: p } if *id == out && p == port))
    };
    match (plugged("left"), plugged("right")) {
        (false, false) => d.facts.push(format!(
            "Nothing is plugged into Output #{out} (left or right)."
        )),
        (true, false) => d.facts.push(format!(
            "Output #{out}: left is connected, right is not (right is silent)."
        )),
        (false, true) => d.facts.push(format!(
            "Output #{out}: right is connected, left is not (left is silent)."
        )),
        (true, true) => {}
    }

    let focus = cx.tap.as_ref().map(|(s, _, _)| s.id).or(focus);
    let mut path_modules: Vec<ModuleId> = Vec::new();
    if let Some(f) = focus.filter(|f| state.modules.contains_key(f) && *f != out) {
        match path_between(state, f, out) {
            Ok(Some(path)) => {
                d.facts.push(format!(
                    "Signal path from {} to the played output: {}.",
                    title(state, f),
                    explain::path_text(state, &path)
                ));
                path_modules = path;
            }
            Ok(None) => {
                d.facts.push(format!(
                    "No signal cable path leads from {} to Output #{out} (routes into knobs \
                     are modulation, not signal).",
                    title(state, f)
                ));
                path_modules = vec![f];
            }
            Err(explain::PathLimit) => d.facts.push(format!(
                "The path search stopped after {MAX_VISIT} modules: whether {} reaches the \
                 output is unknown.",
                title(state, f)
            )),
        }
    } else if focus.is_none() {
        // Without a focus: the stages feeding the output directly.
        path_modules = state
            .cables
            .values()
            .filter(|c| c.to.module_id() == out && matches!(c.to, PortRef::Module { .. }))
            .map(|c| c.from.module_id())
            .collect();
    }

    // Unconnected gate inputs on the path and one hop into it (an envelope on a VCA's cv).
    let mut near: BTreeSet<ModuleId> = path_modules.iter().copied().collect();
    for &m in &path_modules {
        for c in state.cables.values().filter(|c| c.to.module_id() == m) {
            near.insert(c.from.module_id());
        }
    }
    let mut open_gates = Vec::new();
    for &m in &near {
        let Some(info) = state
            .modules
            .get(&m)
            .and_then(|s| registry::info_for(&s.kind))
        else {
            continue;
        };
        for p in info
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input && p.port_type == PortType::Gate)
            .filter(|p| p.name == "gate")
        {
            let connected = state.cables.values().any(
                |c| matches!(&c.to, PortRef::Module { id, port } if *id == m && port == p.name),
            );
            if !connected {
                open_gates.push(format!("{} {}", title(state, m), p.name));
            }
        }
    }
    // Signal inputs a stage on the path needs to pass audio, with no cable (an unplugged input
    // reads silence). Optional inputs (cv, clock, a mixer's other channels, a stereo pair's
    // second side) are not listed: see `needed_inputs`.
    let mut missing: Vec<(ModuleId, String)> = Vec::new();
    for &m in &path_modules {
        let Some(kind) = state.modules.get(&m).map(|s| s.kind.as_str()) else {
            continue;
        };
        for group in needed_inputs(kind) {
            if !group.iter().any(|p| plugged_in(state, m, p)) {
                missing.push((m, group.join(" / ")));
            }
        }
    }
    for (m, ports) in &missing {
        d.facts.push(format!(
            "No cable into {} {ports}: that is its audio input, and an unplugged input reads \
             silence, so this stage passes nothing.",
            title(state, *m)
        ));
    }
    if let Some((m, _)) = missing.first() {
        let idle = idle_audio_outputs(state, *m);
        if idle.is_empty() {
            d.possible.push(format!(
                "Plug an audio source into {} to hear this path.",
                title(state, *m)
            ));
        } else {
            let names: Vec<String> = idle
                .iter()
                .map(|(id, port)| format!("{} {port}", title(state, *id)))
                .collect();
            d.possible.push(format!(
                "Audio outputs that feed nothing: {}. If one of them should feed {}, plug it \
                 there (Undo brings back a cable you just removed).",
                names.join(", "),
                title(state, *m)
            ));
        }
    }
    if !open_gates.is_empty() {
        d.facts
            .push(format!("No cable into: {}.", open_gates.join(", ")));
        d.possible.push(
            "An envelope whose gate has no cable never opens; a VCA it drives then stays shut."
                .into(),
        );
    }
    // Clocks the transport stopped that drive something.
    for (&id, m) in &state.modules {
        if m.kind == "clock"
            && cx.clock_running.get(&id) == Some(&false)
            && state.cables.values().any(|c| c.from.module_id() == id)
        {
            d.facts.push(format!(
                "{} is stopped (transport): what it drives gets no new steps.",
                title(state, id)
            ));
            d.possible
                .push("Press Start (or Run on the clock) to run it.".into());
        }
    }
    // Parameters that may hold the path at silence: the effective base (stored, else the
    // module default), and whether routes move it.
    for &m in &path_modules {
        let Some(ms) = state.modules.get(&m) else {
            continue;
        };
        let cv_in = state
            .cables
            .values()
            .any(|c| matches!(&c.to, PortRef::Module { id, port } if *id == m && port == "cv"));
        match ms.kind.as_str() {
            "vca" if base(state, m, "gain").is_some_and(|g| g <= 0.0) => {
                let routes = routes_into(state, m, "gain");
                d.possible.push(if cv_in || routes > 0 {
                    format!(
                        "{} gain is 0 at its base: it opens only as far as its cv input or the \
                         routes into Gain move it. Inspect what drives it.",
                        title(state, m)
                    )
                } else {
                    format!(
                        "{} gain is 0 and nothing drives its cv or Gain: it passes nothing.",
                        title(state, m)
                    )
                })
            }
            "filter.svf" | "filter.ladder"
                if base(state, m, "cutoff_hz").is_some_and(|c| c < 60.0) =>
            {
                let routes = routes_into(state, m, "cutoff_hz");
                d.possible.push(format!(
                    "{} cutoff is very low at its base ({:.0} Hz){}: a low-pass there removes \
                     most of the sound.",
                    title(state, m),
                    base(state, m, "cutoff_hz").unwrap_or(0.0),
                    if routes > 0 {
                        format!("; {routes} route(s) move it")
                    } else {
                        String::new()
                    }
                ))
            }
            _ => {}
        }
    }

    // Measurements.
    if let Some((sel, t, summary)) = &cx.tap {
        let name = format!("{} {}", title(state, sel.id), sel.port);
        match summary {
            Some(s) => {
                d.measured
                    .extend(measurement_lines(s, *t, &name, cx.sample_rate));
                let quiet = match t {
                    // A held gate (high, no new edge) is not a missing trigger.
                    PortType::Gate => {
                        s.rises_all() == 0 && s.high[..s.lanes].iter().all(|&h| h == 0)
                    }
                    PortType::Audio => s.peak_all() <= QUIET,
                    _ => false,
                };
                let module_kind = state.modules.get(&sel.id).map(|m| m.kind.as_str());
                if quiet && *t == PortType::Gate {
                    match module_kind {
                        Some("midi.in") => d.possible.push(format!(
                            "No key started a note in that time. If you played, check the MIDI \
                             input (now: {}); kabl cannot see whether a device sends.",
                            cx.midi_input.map_or("none".into(), |p| format!("\"{p}\""))
                        )),
                        _ => d.possible.push(
                            "A clock that is stopped or not connected sends no steps; inspect \
                             the clock or divider feeding it."
                                .into(),
                        ),
                    }
                }
                let at = path_modules.iter().position(|&m| m == sel.id);
                if quiet && *t == PortType::Audio {
                    // Upstream: whatever feeds the tapped module's first connected audio
                    // input. A CV source (an envelope on a VCA's cv) says nothing about
                    // whether audio arrives, so it is never suggested here.
                    if let Some((id, port)) = upstream(state, sel.id) {
                        d.possible.push(format!(
                            "Nothing measured here: inspect {} {port} to see whether signal \
                             arrives before this stage.",
                            title(state, id)
                        ));
                        d.next = Some((id, port));
                    } else if let Some((id, port)) = missing
                        .iter()
                        .find(|(m, _)| *m == sel.id)
                        .and_then(|(m, _)| idle_audio_outputs(state, *m).into_iter().next())
                    {
                        d.possible.push(format!(
                            "Nothing measured here, and no cable brings audio in: inspect {} \
                             {port} to see whether it carries signal.",
                            title(state, id)
                        ));
                        d.next = Some((id, port));
                    }
                } else if let Some(next) = at.and_then(|i| path_modules.get(i + 1)).copied() {
                    // Only a measured audio level is called "present"; a CV at 0 is a value,
                    // not an absence.
                    let present = *t == PortType::Audio && s.peak_all() > QUIET;
                    if let Some(port) = first_output(state, next)
                        .filter(|_| state.modules.get(&next).is_some_and(|m| m.kind != "out"))
                    {
                        d.possible.push(if present {
                            format!(
                                "Signal is present here: inspect the next stage, {} {port}.",
                                title(state, next)
                            )
                        } else {
                            format!("Next stage on the path: {} {port}.", title(state, next))
                        });
                        d.next = Some((next, port));
                    }
                }
            }
            None => d.measured.push(format!(
                "No current measurement at {name}: keep the inspector open with audio running."
            )),
        }
    } else {
        d.measured
            .push("No output is inspected: select one (Inspect) to measure it.".into());
    }
    if let Some(level) = cx.output_level {
        d.measured.push(format!(
            "Final output meter (the engine's output, before the audio device): {}.",
            db(level)
        ));
    }
    d.possible.push(
        "An envelope's release tail can sound after keys are up; kabl cannot check speakers, \
         headphones or the device volume."
            .into(),
    );
    d
}

/// Input groups a module needs to pass audio: each group needs at least one cable. Every
/// built-in reads an unplugged input as silence, so a stage whose group is empty outputs
/// nothing. Read from each module's `process`: a VCA, filter, delay, drive or gain multiplies
/// or filters `in`; a mixer sums any of its four channels; reverb and chorus take either side
/// of their stereo pair; a ring modulator multiplies `a` by `b`. Modulation inputs (cv,
/// clock, sync, pitch, reset) are never needed: they only move the stage. Sources and `out`
/// (checked on its own) need nothing here.
pub fn needed_inputs(kind: &str) -> &'static [&'static [&'static str]] {
    match kind {
        "vca" | "filter.svf" | "filter.ladder" | "delay" | "drive" | "gain" => &[&["in"]],
        "mixer" => &[&["in1", "in2", "in3", "in4"]],
        "reverb" | "chorus" => &[&["in_l", "in_r"]],
        "ringmod" => &[&["a"], &["b"]],
        _ => &[],
    }
}

/// A cable is plugged into input `port` of module `id`.
fn plugged_in(state: &PatchState, id: ModuleId, port: &str) -> bool {
    state
        .cables
        .values()
        .any(|c| matches!(&c.to, PortRef::Module { id: t, port: p } if *t == id && p == port))
}

/// Audio outputs of modules other than `except` and the outputs, on modules none of whose
/// outputs has a cable: candidates for a missing audio input. Module order, one per module.
fn idle_audio_outputs(state: &PatchState, except: ModuleId) -> Vec<(ModuleId, String)> {
    let mut out = Vec::new();
    for (&id, m) in &state.modules {
        if id == except || m.kind == "out" {
            continue;
        }
        let Some(info) = registry::info_for(&m.kind) else {
            continue;
        };
        if state.cables.values().any(|c| c.from.module_id() == id) {
            continue;
        }
        if let Some(p) = outputs(info).find(|p| p.port_type == PortType::Audio) {
            out.push((id, p.name.to_string()));
        }
    }
    out
}

/// The first connected audio input of `id` and the output feeding it.
fn upstream(state: &PatchState, id: ModuleId) -> Option<(ModuleId, String)> {
    let info = registry::info_for(&state.modules.get(&id)?.kind)?;
    for p in info
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Input && p.port_type == PortType::Audio)
    {
        for c in state.cables.values() {
            if let (PortRef::Module { id: f, port: fp }, PortRef::Module { id: t, port: tp }) =
                (&c.from, &c.to)
            {
                if *t == id && tp == p.name {
                    return Some((*f, fp.clone()));
                }
            }
        }
    }
    None
}

fn first_output(state: &PatchState, id: ModuleId) -> Option<String> {
    let info = registry::info_for(&state.modules.get(&id)?.kind)?;
    outputs(info).next().map(|p| p.name.to_string())
}

/// Shortest signal-cable path from `from` to `to` (both included), breadth-first over at most
/// `MAX_VISIT` modules. `Err` when the limit stopped it (unknown, not "no path").
pub fn path_between(
    state: &PatchState,
    from: ModuleId,
    to: ModuleId,
) -> Result<Option<Vec<ModuleId>>, explain::PathLimit> {
    let mut prev = BTreeMap::new();
    let mut queue = VecDeque::from([from]);
    prev.insert(from, from);
    while let Some(m) = queue.pop_front() {
        if m == to {
            let mut path = vec![m];
            let mut at = m;
            while at != from {
                at = prev[&at];
                path.push(at);
            }
            path.reverse();
            return Ok(Some(path));
        }
        if prev.len() >= MAX_VISIT {
            return Err(explain::PathLimit);
        }
        for c in state.cables.values() {
            if let (PortRef::Module { id: f, .. }, PortRef::Module { id: t, .. }) = (&c.from, &c.to)
            {
                if *f == m && !prev.contains_key(t) {
                    prev.insert(*t, m);
                    queue.push_back(*t);
                }
            }
        }
    }
    Ok(None)
}

fn weak(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(RichText::new(text.into()).small().weak()).wrap());
}

fn para(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(text.into()).wrap());
}

/// Lane bars for a voiced summary: one small bar per lane.
fn lanes_ui(ui: &mut egui::Ui, s: &Summary, t: PortType) {
    ui.horizontal_wrapped(|ui| {
        for l in 0..s.lanes {
            let v = match t {
                PortType::Audio => {
                    (1.0 + 20.0 * s.peak[l].max(1e-6).log10() / 60.0).clamp(0.0, 1.0)
                }
                PortType::Gate => {
                    if s.held[l] || s.rises[l] > 0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => s.last[l].abs().min(1.0),
            };
            let (r, p) = ui.allocate_painter(egui::vec2(12.0, 22.0), egui::Sense::hover());
            let rect = r.rect;
            p.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
                egui::StrokeKind::Inside,
            );
            let fill = egui::Rect::from_min_max(
                egui::pos2(
                    rect.left() + 2.0,
                    rect.bottom() - 2.0 - (rect.height() - 4.0) * v,
                ),
                egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0),
            );
            p.rect_filled(fill, 1.0, ui.visuals().selection.bg_fill);
            r.on_hover_text(format!(
                "lane {}: peak {:.3}, last {:.3}, {} edges",
                l + 1,
                s.peak[l],
                s.last[l],
                s.rises[l]
            ));
        }
    });
}

/// The drawer section: selection, measurement, "Why no sound?".
pub fn panel(editor: &PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, now: f64) {
    if std::mem::take(&mut ui_state.inspect.reveal) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    }
    let state = editor.state();
    ui.horizontal(|ui| {
        ui.strong("Inspect a signal");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.small_button("Close");
            ui_state.record("inspect-close".into(), r.rect);
            if r.clicked() {
                ui_state.inspect.open = false;
                ui_state.inspect.clear();
                ui_state.inspect.why = false;
            }
        });
    });
    // Outputs of the selected module to choose from.
    let focus = ui_state
        .selected_module
        .or(ui_state.inspect.sel.as_ref().map(|s| s.id));
    if let Some(id) = focus {
        if let Some(info) = state
            .modules
            .get(&id)
            .and_then(|m| registry::info_for(&m.kind))
        {
            let outs: Vec<&'static str> = outputs(info).map(|p| p.name).collect();
            if outs.is_empty() {
                weak(
                    ui,
                    format!("{} has no outputs to measure.", title(state, id)),
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}:", title(state, id)));
                    for p in outs {
                        let on = ui_state
                            .inspect
                            .sel
                            .as_ref()
                            .is_some_and(|s| s.id == id && s.port == p);
                        let r = ui.add(egui::Button::selectable(on, p));
                        ui_state.record(format!("inspect-port:{id}.{p}"), r.rect);
                        if r.clicked() {
                            ui_state.inspect.select(
                                Sel {
                                    id,
                                    port: p.to_string(),
                                },
                                now,
                            );
                        }
                    }
                });
            }
        }
    } else {
        weak(
            ui,
            "Select a module in the rack to choose one of its outputs.",
        );
    }

    let status = ui_state.inspect.status(state, now);
    let sel = ui_state.inspect.sel.clone();
    if let Some(sel) = &sel {
        let t = port_type(state, sel.id, &sel.port);
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!(
                "{} {}{}",
                title(state, sel.id),
                sel.port,
                t.map_or(String::new(), |t| format!(" · {}", type_name(t)))
            ))
            .strong(),
        );
        match &status {
            Status::Missing(id) => para(ui, format!("Module #{id} or this output is no longer in the patch. Undo brings it back.")),
            Status::NoAudio => para(ui, "Unavailable: no audio engine is running, so nothing can be measured."),
            Status::NotCompiled(e) => para(ui, format!("Unavailable: the current patch did not compile ({e}). Audio still plays the last version that did.")),
            Status::Waiting => para(ui, "Waiting for the first measurement…"),
            Status::NotArriving => para(ui, "No measurement is arriving: the audio callback may have stopped (see the status bar)."),
            Status::NotInGraph => para(ui, "Not in the playing graph yet (it is being rebuilt)."),
            Status::Stale(age) => {
                let c = ui.visuals().warn_fg_color;
                ui.add(egui::Label::new(RichText::new(format!("Stale: the last measurement is {age:.1} s old. Values below are not live.")).color(c)).wrap());
            }
            _ => {}
        }
        if let (Some(t), Some(s)) = (t, ui_state.inspect.summary(now)) {
            let live = status == Status::Live;
            weak(
                ui,
                format!(
                    "{} · {}",
                    lane_scope(&s),
                    if s.fading {
                        "measured on the edited version while it crossfades in".to_string()
                    } else if s.generation < ui_state.inspect.generation {
                        "measured on the previous version while the edit compiles".to_string()
                    } else {
                        format!("updated {:.2} s ago", s.newest_age)
                    }
                ),
            );
            let name = format!("{} {}", title(state, sel.id), sel.port);
            for line in measurement_lines(&s, t, &name, ui_state.sample_rate) {
                let text = RichText::new(line);
                ui.add(egui::Label::new(if live { text } else { text.weak() }).wrap());
            }
            if s.voiced {
                lanes_ui(ui, &s, t);
            }
            if ui_state.inspect.dropped > 0 {
                weak(
                    ui,
                    format!(
                        "{} measurements lost on the way (queue full).",
                        ui_state.inspect.dropped
                    ),
                );
            }
        }
    }
    ui.add_space(4.0);
    let open = ui_state.inspect.why;
    let r = ui.add(egui::Button::selectable(open, "Why no sound?"));
    ui_state.record("why".into(), r.rect);
    if r.clicked() {
        ui_state.inspect.why = !open;
        log::debug!(target: "inspect", "why-no-sound open={}", !open);
    }
    if ui_state.inspect.why {
        why_panel(editor, ui_state, ui, now);
    }
}

fn type_name(t: PortType) -> &'static str {
    match t {
        PortType::Audio => "audio",
        PortType::Cv => "CV (±1)",
        PortType::UnipolarCv => "CV (0…1)",
        PortType::Gate => "gate",
        PortType::Pitch => "pitch (semitones from note 60)",
    }
}

fn why_panel(editor: &PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, now: f64) {
    let state = editor.state();
    let sel = ui_state.inspect.sel.clone();
    let tap = sel.as_ref().and_then(|s| {
        let t = port_type(state, s.id, &s.port)?;
        let live = matches!(ui_state.inspect.status(state, now), Status::Live);
        Some((s, t, ui_state.inspect.summary(now).filter(|_| live)))
    });
    let cx = Context {
        clock_running: &ui_state.clock_running,
        output_level: ui_state
            .inspect
            .audio
            .then(|| ui_state.meter.level[0].max(ui_state.meter.level[1])),
        tap,
        midi_input: ui_state.midi_input.as_deref(),
        sample_rate: ui_state.sample_rate,
        output: None,
    };
    let topo = topology(state);
    if ui_state
        .inspect
        .output
        .as_ref()
        .is_none_or(|(t, _)| *t != topo)
    {
        ui_state.inspect.output = Some((topo, engine_output(state)));
    }
    let cx = Context {
        output: ui_state.inspect.output.as_ref().map(|(_, o)| o),
        ..cx
    };
    let d = diagnose(state, ui_state.selected_module, &cx);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        for (head, lines) in [
            ("From the patch", &d.facts),
            ("Measured", &d.measured),
            ("Could be (check next)", &d.possible),
        ] {
            if lines.is_empty() {
                continue;
            }
            ui.label(RichText::new(head).small().strong());
            for l in lines {
                para(ui, format!("• {l}"));
            }
        }
        if let Some((id, port)) = d.next.clone() {
            let r = ui.button(format!("Inspect {} {port}", title(state, id)));
            ui_state.record("why-next".into(), r.rect);
            if r.clicked() {
                ui_state.selected_module = Some(id);
                ui_state.inspect.select(Sel { id, port }, now);
            }
        }
    });
}
