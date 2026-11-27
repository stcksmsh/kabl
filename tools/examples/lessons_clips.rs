//! Lessons 1-3 (lessons/01-03), heard at the start and at the end: the sound each lesson opens
//! on, and the result after its steps (the edits the walkthrough tests in
//! crates/ui/tests/lessons.rs make), as Sounds' preview clip of each.
//!
//!     cargo run --release -p kabl-tools --example lessons_clips -- OUT_DIR
//!
//! Writes `OUT_DIR/<lesson>-start.wav` and `OUT_DIR/<lesson>-end.wav`. Offline: no audio device
//! is opened.
use kabl_core::PortRef;
use kabl_ui::library::{built_factory_dir, Library};
use kabl_ui::{explain, routing, sound_preview, PatchEditor};

const RATE: f32 = 48000.0;

/// Moves `param` of module `id` by `by` from where it is.
fn turn(e: &mut PatchEditor, id: u64, param: &str, by: f32) {
    let info = explain::param_of(e.state(), id, param).expect("the lesson's parameter");
    let v = routing::base_value(e.state(), id, info);
    e.set_param(id, param, v + by);
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("OUT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    // The user area is only read (for favorites), never written.
    let lib = Library::open(Some(built_factory_dir()), dir.join("no-user-sounds"));
    // (lesson, the sound it ends on, the edits its steps make)
    type Lesson = (&'static str, &'static str, fn(&mut PatchEditor));
    let lessons: [Lesson; 3] = [
        ("find-and-play", "factory:presets/glass-elevator", |e| {
            turn(e, 1, "m1", 0.3)
        }),
        ("change-sounds", "factory:recipes/pluck-to-pad", |e| {
            turn(e, 6, "sustain", 0.3);
            turn(e, 6, "release_ms", 300.0);
            turn(e, 6, "attack_ms", 300.0);
        }),
        ("signal-flow", "factory:recipes/filter-movement", |e| {
            let route = *e
                .state()
                .cables
                .iter()
                .find(|(_, c)| {
                    matches!(&c.from, PortRef::Module { id: 7, .. })
                        && matches!(&c.to, PortRef::Param { id: 3, .. })
                })
                .expect("the LFO route")
                .0;
            e.set_route_bypass(route, false);
        }),
    ];
    for (id, sound, finish) in lessons {
        let lesson = kabl_ui::lessons::all()
            .iter()
            .find(|l| l.id == id)
            .expect("the lesson");
        let shelf = lib.get(sound).expect("a factory sound").meta.shelf();
        let mut start = PatchEditor::from_log(lib.read(&lesson.start.sound).unwrap());
        // Lesson 1 opens on a Piece and ends on the Keys sound it asks for.
        let start_shelf = lib.get(&lesson.start.sound).unwrap().meta.shelf();
        let mut end = PatchEditor::from_log(lib.read(sound).unwrap());
        finish(&mut end);
        for (name, e, shelf) in [("start", &mut start, start_shelf), ("end", &mut end, shelf)] {
            let clip = sound_preview::render_state(e.state(), shelf, RATE).expect("renders");
            let wav = dir.join(format!("{id}-{name}.wav"));
            let mut w = hound::WavWriter::create(
                &wav,
                hound::WavSpec {
                    channels: 2,
                    sample_rate: clip.rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for s in &clip.samples {
                w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                    .unwrap();
            }
            w.finalize().unwrap();
            println!("{} ({:+.1} dB levelled)", wav.display(), clip.gain_db);
        }
    }
}
