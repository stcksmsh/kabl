use kabl_modules::builtins::{MidiIn, Mixer, Out};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

/// `midi.in`'s params at their defaults (POLY, LAST, glide OFF, 120 ms).
const MIDI_PARAMS: [Signal<'static>; 4] = [
    Signal::Scalar(0.0),
    Signal::Scalar(0.0),
    Signal::Scalar(0.0),
    Signal::Scalar(120.0),
];

const BLOCK: usize = 8;

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

#[test]
fn mixer_sums_four_inputs_with_per_channel_levels() {
    let mut module = Mixer::new();
    module.prepare(48000.0, BLOCK, &quality());

    let ins = [[1.0f32; BLOCK], [1.0; BLOCK], [1.0; BLOCK], [1.0; BLOCK]];
    let inputs = [
        Signal::Buffer(&ins[0]),
        Signal::Buffer(&ins[1]),
        Signal::Buffer(&ins[2]),
        Signal::Buffer(&ins[3]),
    ];
    let params = [
        Signal::Scalar(1.0),
        Signal::Scalar(0.5),
        Signal::Scalar(0.25),
        Signal::Scalar(0.0),
    ];
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
    module.process(&mut io);

    // 1*1.0 + 1*0.5 + 1*0.25 + 1*0.0 = 1.75
    for &v in &buf {
        assert!((v - 1.75).abs() < 1e-6, "expected 1.75, got {v}");
    }
}

/// Runs `out` for `blocks` blocks of a constant stereo input at `level_db`; returns the last
/// block's left and right outputs.
fn run_out(level_db: f32, blocks: usize) -> ([f32; BLOCK], [f32; BLOCK]) {
    let mut module = Out::new();
    module.prepare(48000.0, BLOCK, &quality());
    let left = [0.3f32; BLOCK];
    let right = [-0.7f32; BLOCK];
    let inputs = [Signal::Buffer(&left), Signal::Buffer(&right)];
    let params = [Signal::Scalar(level_db)];
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    for _ in 0..blocks {
        let mut outputs: [&mut [f32]; 2] = [&mut l, &mut r];
        let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
        module.process(&mut io);
    }
    (l, r)
}

#[test]
fn out_module_level_defaults_to_zero_db_and_passes_the_input_untouched() {
    let info = kabl_modules::builtins::OUT_INFO.params;
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].name, "level_db");
    assert_eq!(info[0].default, 0.0);
    let (l, r) = run_out(0.0, 1);
    assert_eq!(l, [0.3f32; BLOCK]);
    assert_eq!(r, [-0.7f32; BLOCK]);
}

#[test]
fn out_module_applies_its_level_in_db() {
    // Settled (many blocks past the 20 ms glide): -6.0206 dB is exactly half.
    let (l, r) = run_out(-6.0206, 400);
    for (&a, &b) in l.iter().zip(&r) {
        assert!((a - 0.15).abs() < 1e-3, "left {a}");
        assert!((b + 0.35).abs() < 1e-3, "right {b}");
    }
    // Quieter than the input, and louder with a positive level.
    let (loud, _) = run_out(6.0, 400);
    assert!(loud[0] > 0.3 * 1.99 && loud[0] < 0.3 * 2.01, "{}", loud[0]);
}

#[test]
fn midi_in_note_on_off_drives_gate_pitch_velocity() {
    let mut module = MidiIn::new();
    module.prepare(48000.0, BLOCK, &quality());

    module.note_on(7.0, 0.8); // 7 semitones up, velocity 0.8
    let mut gate = [0f32; BLOCK];
    let mut pitch = [0f32; BLOCK];
    let mut velocity = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 3] = [&mut gate, &mut pitch, &mut velocity];
        let mut io = ProcessIo::new(&[], &mut outputs, &MIDI_PARAMS, BLOCK);
        module.process(&mut io);
    }
    assert!(gate.iter().all(|&v| v == 1.0));
    assert!(pitch.iter().all(|&v| v == 7.0));
    assert!(velocity.iter().all(|&v| (v - 0.8).abs() < 1e-6));

    module.note_off();
    let mut gate2 = [0f32; BLOCK];
    let mut pitch2 = [0f32; BLOCK];
    let mut velocity2 = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 3] = [&mut gate2, &mut pitch2, &mut velocity2];
        let mut io = ProcessIo::new(&[], &mut outputs, &MIDI_PARAMS, BLOCK);
        module.process(&mut io);
    }
    assert!(
        gate2.iter().all(|&v| v == 0.0),
        "note_off should drop gate to 0"
    );
    assert!(
        pitch2.iter().all(|&v| v == 7.0),
        "pitch should hold at the last note's value"
    );
}
