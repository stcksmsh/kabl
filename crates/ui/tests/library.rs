//! The sound library (`kabl_ui::library`): factory discovery and metadata, user Save As /
//! Save / rename, name and directory collisions, factory protection, read-only and failed
//! saves, interrupted-save repair, malformed and uncompilable patches, favorites and recents
//! (missing entries included), old patches without metadata.
//!
//! `write_factory_library` (ignored) writes the init keyboard patch and every factory
//! `sound.toml`: `cargo test -p kabl-ui --test library write_factory_library -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_ui::library::{self, LibError, Library, Meta, Origin, CATEGORIES};
use kabl_ui::perform::PIN_PREFIX;
use kabl_ui::PatchEditor;

#[allow(dead_code)]
#[path = "support/factory_bank.rs"]
mod factory_bank;

fn factory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .canonicalize()
        .unwrap()
}

/// Every factory sound: (dir, name, category, tags, description).
const FACTORY: &[(&str, &str, &str, &[&str], &str)] = &[
    (
        "grid-cues",
        "Grid Cues",
        "Piece",
        &["sequenced", "cues", "drums", "drum grid", "124bpm"],
        "A drum grid played by cues. Four cues (Intro, Groove, Fill, Break) each pick one of the grid's patterns, and the pattern changes on the next bar, so a piece moves from hats to a groove to a fill to a breakdown without a hand on the grid.",
    ),
    (
        "cue-macros",
        "Cue Macros",
        "Piece",
        &["sequenced", "cues", "macros", "morph", "100bpm"],
        "A self-playing bass line and pad. Four cues (Verse, Chorus, Bridge, Verse 2) each set the three macros to their own values and move there over a set number of beats: Verse dark and dry, Chorus open and wide, Bridge washed in space. Run, then launch the cues on the next bar; turn a macro to take it over.",
    ),
    (
        "macros-eight",
        "Eight Macros",
        "Piece",
        &["pad", "macros", "performance", "drone"],
        "A drifting two-voice pad with eight named macros on the Perform page. Motion does not move a knob: it fades in a slow, gentle breath on the filter cutoff, from still to full. Run, then turn Motion.",
    ),
    (
        "functional-cables/morph-arc",
        "Morph Arc",
        "Piece",
        &[
            "sequenced",
            "functional cables",
            "morph",
            "100bpm",
            "a",
            "minor",
        ],
        "A slow A-minor piece that plays by itself. Four cables each hold two patterns, A and \
         B, and a morph between them: the delay send (one throw per eight-step arp cycle to every step), the \
         pad's gate (a breathing hold to a sixteen-step chop) and the bass filter (four even \
         steps to five uneven ones) and the bass level (quiet to full). Open a cable's Pattern editor and move Morph A to B.",
    ),
    (
        "functional-cables/macro-morph",
        "Macro Morph",
        "Piece",
        &["sequenced", "functional cables", "morph", "macro", "120bpm"],
        "Three gate cables, each with a pattern A and a pattern B, and one macro (m1) routed into all three morphs at +100 %, +50 % and -100 %. Raise the macro: the bass and the middle voice fill in, the high voice thins out. Open a cable's Pattern editor to see its source and change its depth.",
    ),
    (
        "functional-cables/echo-throws",
        "Echo Throws",
        "Piece",
        &["sequenced", "functional cables", "probability", "112bpm"],
        "A self-playing arpeggio. The cable from the voice to the delay opens on three of eight \
         steps and passes 60 % of the time, so echoes are thrown at the notes by chance. The \
         pitch cable moves to a new note 70 % of the time and otherwise holds the last one. \
         Right-click a cable, Pattern, to edit.",
    ),
    (
        "functional-cables/five-against-eight",
        "Five Against Eight",
        "Piece",
        &["sequenced", "functional cables", "polymeter", "104bpm"],
        "An 8-step bass line. A 5-step pattern on the modulation route into the filter cutoff \
         moves the tone against it, so the two realign only every 40 steps, and one cutoff step \
         passes 70 % of the time. A 16-step pattern on the pad's cable chops it into a gate. \
         Open a cable's Pattern editor to change either.",
    ),
    (
        "presets/cinder-block",
        "Cinder Block",
        "Bass",
        &["mono", "distorted", "compressed", "french touch", "signature"],
        "A blunt mono bass: two saws over a square sub, hit with 18 dB of drive, a filter that closes on every note and a compressor squeezing it flat. No wobble, just dirt. Play short, off-beat notes. Bite opens the filter (the mod wheel does too), Grit pushes the drive harder, Room adds a slapback and a tight room, Crush mixes in a bitcrusher.",
    ),
    (
        "presets/parquet-loop",
        "Parquet Loop",
        "Keys",
        &["poly", "filtered", "phased", "disco", "122bpm", "signature"],
        "Hold a chord and it becomes a filtered disco loop: saws and pulse into crunch, a resonant filter that closes on every beat (locked to the clock), a four-stage phaser and a compressor, at 122 BPM with swing. Press Run for the filter loop. Notes slide into each other: change chords and the voices glide. Cutoff opens the filter, Crunch adds grit, Space adds a dotted echo and a tight room, Glide sets how long the slide takes (all the way down, notes just step).",
    ),
    (
        "presets/loading-dock",
        "Loading Dock",
        "Piece",
        &["sequenced", "cues", "drop", "124bpm", "filter house", "signature"],
        "A self-playing filter-house piece at 124 BPM with swing: a bass and a stab line that duck on every beat through a silent envelope on the host clock (no drum is built in), a resonant filter, a noise riser and a hand-played voice on top. Five cues: Intro (filter shut, dry, silent until Run), Build (the filter opens over three bars as the riser and the wash rise), Gap (everything shut but the wash), Drop (drive and pump at full), Break (back down, wet). Press Run, then launch the cues with the pads or CC 40-44; turn a macro to take over. Sweep opens the filter, Drive pushes the drive and the bitcrusher, Wash brings in the riser, the echo and the room, Pump sets how hard the beat ducks the music.",
    ),
    (
        "presets/glass-elevator",
        "Glass Elevator",
        "Keys",
        &["poly", "fm", "electric piano", "bell", "pop"],
        "A six-operator FM electric piano with a glassy tine that fades in a tenth of a second, over a mellow body. Velocity brightens it. Tine sets the FM brightness, Grit adds grit, Space adds ping-pong echo and room, Chorus brings in the chorus.",
    ),
    (
        "presets/undertow",
        "Undertow",
        "Bass",
        &["mono", "reese", "sub", "glide", "drum and bass", "hip-hop"],
        "A reese sub bass: two saws a few cents apart beating slowly against each other over a sine an octave down, through a gentle drive and a low-pass filter, with a short glide between overlapped notes. Play long low notes. Cutoff opens the tone, Weight thickens it with drive, Room adds a short room, Beating widens the beating and sweeps the filter.",
    ),
    (
        "presets/salt-flat",
        "Salt Flat",
        "Pad",
        &["poly", "ambient", "cinematic", "slow", "evolving"],
        "A slow, wide ambient pad: soft triangle stacks with a slow filter bloom, a light chorus, a long echo and a long room. Hold open chords and let them breathe. It starts clean. Bloom opens the filter, Saturation adds a little drive, Space is the echo and the room, Texture brings in a grainy, staticky layer: a high shimmer voice, a wider chorus and a looser unison.",
    ),
    (
        "presets/boxwood",
        "Boxwood",
        "Pluck",
        &["poly", "mallet", "fm", "wooden", "velocity"],
        "An FM mallet, like a wooden bar: a round body with a bright overtone that vanishes at once and a small click. Velocity brings out the overtone. Brightness sets the FM brightness, Warmth adds warmth, Space adds an echo and a medium room, Chorus brings in the chorus.",
    ),
    (
        "presets/neon-highway",
        "Neon Highway",
        "Lead",
        &["mono", "legato", "glide", "bright", "synthwave", "80s"],
        "A wide mono lead for late-night driving: a detuned saw stack and a pulse, a light drive, a ladder filter that opens with velocity, chorus, a dotted echo and a room. Legato notes glide. Cutoff brightens, Bite adds drive, Space adds echo and room, Vibrato adds vibrato.",
    ),
    // BEGIN keys-pluck (generated by presets_keys_pluck library)
    (
        "presets/velvet-tine",
        "Velvet Tine",
        "Keys",
        &["poly", "fm", "phased", "pop", "soul"],
        "A soft Rhodes-like electric piano: a mellow body, a tine that fades in a tenth of a second, velocity-sensitive bark. Play chords. Tine opens the FM brightness and the filter, Drive adds warmth, Space sets the echo and room, Phase brings in a slow phaser.",
    ),
    (
        "presets/checkerboard",
        "Checkerboard",
        "Keys",
        &["poly", "stab", "distorted", "compressed", "french touch", "124bpm", "signature"],
        "A house piano stab: two saws and a square, driven hard and squashed until it barks. Play short syncopated chords. Cutoff opens the filter, Crunch pushes the drive, Room sets a slapback and a tight room, Length stretches the stab.",
    ),
    (
        "presets/linoleum",
        "Linoleum",
        "Keys",
        &["poly", "fm", "phased", "compressed", "french touch", "122bpm", "signature"],
        "A phased disco electric piano, driven and compressed: bright, clipped, with a six-stage phaser always on. Play two-beat chord stabs. Bright opens the FM index and the filter, Crunch pushes the drive, Room sets a tight room and a slapback, Swirl speeds the phaser.",
    ),
    (
        "presets/cellar-door",
        "Cellar Door",
        "Keys",
        &["poly", "fm", "dark", "hip-hop", "lofi"],
        "A dark electric piano that sits low in a beat: a closed filter, a little drive, a slow pitch wobble from the chorus. Play slow minor chords. Dark opens the filter and the FM index, Dirt adds drive, Room sets the echo and room, Wobble brings in the slow chorus.",
    ),
    (
        "presets/pulpit",
        "Pulpit",
        "Keys",
        &["poly", "organ", "additive", "house", "pop"],
        "A drawbar-style organ from six added sines, played through a rotary-speaker chorus. Hold chords, or play a house line. Brightness opens the filter, Overdrive adds the crunch of a pushed tube, Room sets a short room, Rotor brings in the speaker and speeds it up.",
    ),
    (
        "presets/rosewood",
        "Rosewood",
        "Pluck",
        &["poly", "fm", "marimba", "world", "pop"],
        "A deep marimba: a round body, a tuned overtone that rings for a moment, and a soft mallet. Play it with velocity. Mallet sets the hardness of the strike, Warmth adds a little drive, Room sets the echo and a medium room, Ring lengthens the body.",
    ),
    (
        "presets/rain-gutter",
        "Rain Gutter",
        "Pluck",
        &["poly", "fm", "kalimba", "ambient", "pop"],
        "A kalimba: a short metallic click, a long clear ring, and a wet tail that blurs the notes into each other. Play slow, sparse notes. Tine sets the brightness of the click, Warmth adds a little drive, Space sets the echo and the long room, Shimmer brings in the slow chorus.",
    ),
    (
        "presets/spark-plug",
        "Spark Plug",
        "Pluck",
        &["poly", "pluck", "distorted", "compressed", "french touch", "124bpm", "signature"],
        "A blunt, driven synth pluck: two saws and a square, 18 dB of drive, a fast compressor. Play short low notes. Cutoff opens the filter, Crunch pushes the drive, Room sets a slapback and a tight room, Decay lengthens the note.",
    ),
    (
        "presets/tin-roof",
        "Tin Roof",
        "Keys",
        &["poly", "fm", "reedy", "funk", "disco"],
        "A Wurlitzer-like electric piano: a reedy attack that barks harder when you play harder, and a quick fall to a soft sustain. Play funky stabs and short vamps. Tone opens the FM brightness and the filter, Bark pushes the drive, Room sets the echo and a short room, Chorus brings in a light chorus.",
    ),
    (
        "presets/turnstile",
        "Turnstile",
        "Keys",
        &["poly", "sequenced", "pumping", "phased", "french touch", "123bpm", "signature"],
        "Hold a chord and press Run: a driven, phased saw chord that ducks on every beat (a silent envelope on the host clock at 123 BPM with swing; no drum is built in). Cutoff opens the filter, Crunch pushes the drive, Room sets a synced echo and a tight room, Pump sets how hard the beat ducks the chord.",
    ),
    (
        "presets/hand-bell",
        "Hand Bell",
        "Keys",
        &["poly", "fm", "bell", "ambient", "experimental"],
        "An inharmonic FM bell: three pairs of operators that ring for seconds and darken as they fade. Play slow, sparse notes. Strike sets the brightness of the attack, Grit adds a little drive, Space sets the echo and a long room, Shimmer brings in a slow chorus.",
    ),
    (
        "presets/corduroy",
        "Corduroy",
        "Keys",
        &["poly", "clav", "pulse", "phased", "funk"],
        "A funk clavinet: a narrow pulse with a hard filter snap and a short sustain. Play 16th-note chops. Cutoff opens the filter, Drive adds the bite, Room sets a short room and slapback, Phase brings in a phaser.",
    ),
    (
        "presets/matinee",
        "Matinee",
        "Keys",
        &["poly", "supersaw", "chorus", "synthwave"],
        "A wide keyboard from seven detuned saws, a soft filter and a big room. Play slow chords. Cutoff opens the filter, Warmth adds a little drive, Space sets the echo and the long room, Width pulls the saws apart.",
    ),
    (
        "presets/snowmelt",
        "Snowmelt",
        "Keys",
        &["poly", "fm", "soft", "ambient", "cinematic"],
        "A slow, soft electric piano in a big room: a gentle attack, a long fade and a drifting tuning. Play slow open chords. Tine opens the FM brightness and the filter, Warmth adds a little drive, Space sets the echo and a six-second room, Drift detunes the voice and slows the chorus.",
    ),
    (
        "presets/scaffold",
        "Scaffold",
        "Keys",
        &["poly", "stab", "phased", "compressed", "drum and bass", "174bpm"],
        "A dark, wide stab for drum and bass: eight detuned saws, a closing filter, a phaser and a compressor. Play short rolling chords. Cutoff opens the filter, Grit pushes the drive, Room sets a slapback and a tight room, Length stretches the stab.",
    ),
    (
        "presets/brass-filament",
        "Brass Filament",
        "Keys",
        &["poly", "fm", "metallic", "experimental"],
        "A metallic FM tone that rings and wavers: inharmonic partials, a little feedback and a slow fade. Play sparse two-note intervals. Strike sets the brightness, Rust adds drive, Space sets the echo and room, Warp feeds the sound back into itself.",
    ),
    (
        "presets/spinet",
        "Spinet",
        "Keys",
        &["poly", "pulse", "harpsichord", "world"],
        "A bright harpsichord-like key: a thin pulse and a saw, an instant attack and a plucked fall, with a faint buzz. Play counterpoint lines. Tone opens the filter, Buzz adds drive, Room sets the echo and a room, Chorus doubles the strings.",
    ),
    (
        "presets/willow",
        "Willow",
        "Pluck",
        &["poly", "fm", "harp", "ambient", "cinematic"],
        "A soft harp: a quick pluck and a long, clean ring. Play rolled chords. Pluck sets the brightness of the attack, Warmth adds a little drive, Space sets the echo and a long room, Stretch lengthens the ring.",
    ),
    (
        "presets/dry-creek",
        "Dry Creek",
        "Pluck",
        &["poly", "fm", "guitar", "pop", "world"],
        "A nylon-string guitar pluck: a soft pick, a woody body and a quick decay. Play arpeggios. Pick sets the attack brightness, Warmth adds a little drive, Room sets the echo and a room, Chorus doubles the string.",
    ),
    (
        "presets/gumdrop",
        "Gumdrop",
        "Pluck",
        &["poly", "pluck", "chorus", "pop"],
        "A short, bouncy pop pluck: two pulses and a saw, a resonant filter that bounces on each note. Play catchy riffs. Cutoff opens the filter, Drive adds the bite, Space sets the echo and room, Decay stretches the note.",
    ),
    (
        "presets/cold-cut",
        "Cold Cut",
        "Pluck",
        &["poly", "fm", "bell", "dark", "hip-hop", "trap"],
        "A dark, short bell-pluck for trap: a metallic click, a hollow body and a quick fall. Play minor figures with room to ring. Tone opens the FM brightness and the filter, Dirt pushes the drive, Room sets the echo and a room, Decay lengthens the note.",
    ),
    (
        "presets/rivet-gun",
        "Rivet Gun",
        "Pluck",
        &["poly", "pluck", "phased", "compressed", "french touch", "125bpm", "signature"],
        "A bright, staccato, phased pluck, driven and squashed: a narrow pulse with a hard filter snap. Play short syncopated riffs. Cutoff opens the filter, Crunch pushes the drive, Room sets a slapback and a tight room, Phase brings in the phaser.",
    ),
    (
        "presets/tuning-fork",
        "Tuning Fork",
        "Pluck",
        &["poly", "fm", "metallic", "ambient", "experimental"],
        "Tuned metal bars: six added partials of a free bar, a soft strike and a long ring. Play slow melodies. Strike opens the brightness, Warmth adds a little drive, Space sets the echo and a long room, Wobble brings in a slow chorus.",
    ),
    // END keys-pluck
    (
        "presets/steel-toe",
        "Steel Toe",
        "Bass",
        &["mono", "distorted", "compressed", "sidechain", "124bpm", "french touch", "signature"],
        "A blunt mono bass for French touch: two saws over a square sub, 18 dB of drive and a compressor squeezing it flat, then ducked on every beat by a silent envelope on the host clock (no drum is built in). Press Run to start the clock, then play short off-beat notes; the mod wheel opens the filter. Bite opens the filter, Grit pushes the drive, Pump sets how hard the beat ducks the bass, Room adds a tempo-synced slapback and a tight room.",
    ),
    (
        "presets/tar-pit",
        "Tar Pit",
        "Bass",
        &["mono", "808", "sub", "glide", "trap", "hip-hop"],
        "A trap 808: a pure sine with a small pitch snap and a long, round body, sliding from note to note. Play long low notes and overlap them to slide. Tone opens the low-pass, Dirt drives it so it speaks on small speakers, Slide sets how long the slide takes, Room adds a short slapback and room.",
    ),
    (
        "presets/acid-rain",
        "Acid Rain",
        "Bass",
        &["mono", "legato", "acid", "glide", "house", "techno"],
        "A legato acid bass: a saw over a square sub into a resonant ladder filter that snaps on every note, with a short slide between overlapped notes. Play a sixteenth-note line; velocity accents the filter. Cutoff opens the filter, Squelch raises the resonance, Space adds a dotted echo and a room, Drive pushes the drive stage.",
    ),
    (
        "presets/oatmeal",
        "Oatmeal",
        "Bass",
        &["mono", "clean", "round", "pop"],
        "A clean, round pop bass: a triangle over a sine an octave down, a soft pluck in the low-pass and a light compressor. It sits under anything. Tone opens the filter, Pluck deepens the filter snap on each note, Grit adds a little drive, Room adds a short room and slapback.",
    ),
    (
        "presets/spun-sugar",
        "Spun Sugar",
        "Lead",
        &["mono", "legato", "smooth", "chorus", "pop"],
        "A smooth, vocal pop lead: a detuned saw stack over a triangle, a ladder filter that opens with velocity, light chorus and a dotted echo. Play legato; notes glide. Tone brightens, Edge adds drive, Space adds the echo and a room, Vibrato fades in a gentle vibrato.",
    ),
    (
        "presets/pig-iron",
        "Pig Iron",
        "Lead",
        &["mono", "distorted", "bitcrushed", "compressed", "french touch", "signature"],
        "A hard, crushed lead for French touch: a narrow pulse and a saw into 16 dB of drive, a bitcrusher and a compressor, retriggered on every note. Play short syncopated lines. Cutoff opens the filter, Grit pushes the drive, Crush mixes in the bitcrusher, Room adds a slapback and a tight room.",
    ),
    (
        "presets/hot-wire",
        "Hot Wire",
        "Lead",
        &["mono", "legato", "acid", "glide", "house", "techno"],
        "A legato acid lead: a saw and a narrow pulse into a resonant ladder filter with velocity accents and a dotted echo. Play a sixteenth-note line and overlap notes to slide. Cutoff opens the filter, Squelch raises the resonance, Space adds the echo and a room, Pulse sets a slow pulse-width sweep on the second oscillator.",
    ),
    (
        "presets/lantern",
        "Lantern",
        "Lead",
        &["mono", "legato", "soft", "slow", "ambient", "cinematic"],
        "A soft ambient lead that swells in: a triangle stack with a sine an octave up, chorus, a long echo and a big room. Play long, slow notes. Tone opens the filter, Swell lengthens the fade-in of each note, Space is the echo and room, Vibrato adds a slow vibrato.",
    ),
    (
        "presets/rubber-band",
        "Rubber Band",
        "Bass",
        &["mono", "plucked", "slap", "funk", "disco"],
        "A funky plucked bass: a narrow pulse over a saw an octave down, a fast filter slap on every note and a light compressor. Play 16th-note grooves; harder notes open the filter more. Tone opens the filter, Slap deepens the filter snap on each note, Grit adds drive, Room adds a short slapback and room.",
    ),
    (
        "presets/slate-roof",
        "Slate Roof",
        "Bass",
        &["mono", "neuro", "unison", "distorted", "drum and bass"],
        "A neuro bass for drum and bass: a four-voice saw stack against a detuned saw over a square sub, 14 dB of drive and a compressor. Play long half-time notes; the mod wheel opens the filter. Cutoff opens the filter, Growl fades in a rolling filter movement, Crush mixes in a bitcrusher, Room adds a tight room and slapback.",
    ),
    (
        "presets/gutter",
        "Gutter",
        "Bass",
        &["mono", "distorted", "bitcrushed", "compressed", "french touch", "signature"],
        "A dirty mid-range bass for French touch: a saw and a square sub into 24 dB of drive, a light bitcrusher and an 8:1 compressor, with a pulse an octave up to bring in. Play short, chugging notes. Bite opens the filter, Grit pushes the drive, Octave brings in the pulse an octave up, Room adds a slapback and a tight room.",
    ),
    (
        "presets/low-tide",
        "Low Tide",
        "Bass",
        &["mono", "legato", "sub", "slow", "ambient", "cinematic"],
        "An ambient sub bass that swells in: a sine over a triangle an octave down, a slow filter tide, chorus, a long echo and a big room. Play long, slow notes; they glide into each other. Tone opens the filter, Tide fades in a slow filter movement, Swell lengthens the fade-in of each note, Space is the echo and the room.",
    ),
    (
        "presets/copper-wire",
        "Copper Wire",
        "Bass",
        &["mono", "ring mod", "metallic", "experimental", "modular"],
        "An experimental metal bass: a sine ring-modulated by a triangle a fifth up, which adds inharmonic bell tones above a clean sub. Play slow, low notes. Tone opens the filter, Metal brings in the ring modulator, Grit adds drive, Room adds an echo and a room.",
    ),
    (
        "presets/tail-light",
        "Tail Light",
        "Bass",
        &["mono", "chorus", "pulsing", "synthwave", "80s"],
        "A synthwave bass: two saws and a square sub with a medium pluck and a wide chorus. Play steady eighth notes. Cutoff opens the filter, Drive adds drive, Chorus brings in the chorus, Space adds a dotted echo and a room.",
    ),
    (
        "presets/rail-yard",
        "Rail Yard",
        "Bass",
        &["mono", "sidechain", "124bpm", "house"],
        "A house bass that ducks on every beat: a saw and a square sub through a compressor, pulled down by a silent envelope on the host clock (no drum is built in). Press Run for the pump, then play off-beat notes; the mod wheel opens the filter. Cutoff opens the filter, Drive adds drive, Pump sets how hard the beat ducks the bass, Room adds a synced echo and a room.",
    ),
    (
        "presets/oak-stave",
        "Oak Stave",
        "Bass",
        &["mono", "plucked", "upright", "acoustic", "world"],
        "An upright-style plucked bass: a triangle with a sine body, a soft pitch snap and a filter that closes after each pluck. Play walking lines. Tone opens the filter, Pluck deepens the filter snap, Body adds the sine body under the note, Room adds a short room.",
    ),
    (
        "presets/glass-needle",
        "Glass Needle",
        "Lead",
        &["mono", "legato", "pure", "sine", "modular", "experimental"],
        "A pure modular lead: a sine with an octave of overtone and a ping-pong echo. Play slow, high lines; notes glide. Tone opens the filter, Overtone brings in the octave above, Space adds the echo and the room, Vibrato fades in a gentle vibrato.",
    ),
    (
        "presets/loud-mouth",
        "Loud Mouth",
        "Lead",
        &["mono", "formant", "talking", "distorted", "french touch", "signature"],
        "A talking lead for French touch: a saw and a pulse driven hard into a formant filter that sounds like a mouth, with a gentle vowel drift. Play short riffs; the mod wheel moves the vowel. Vowel sets the vowel from a to u, Grit pushes the drive, Shift makes the mouth smaller or larger, Room adds a slapback and a tight room.",
    ),
    (
        "presets/dust-devil",
        "Dust Devil",
        "Lead",
        &["mono", "glide", "whistle", "trap", "hip-hop"],
        "A trap whistle lead: a sine with a touch of triangle that slides from note to note, with a dotted echo and a vibrato. Play simple melodies. Tone opens the filter, Slide sets how long the slide takes, Space adds the echo and the room, Vibrato fades in a vibrato.",
    ),
    (
        "presets/afterglow",
        "Afterglow",
        "Lead",
        &["mono", "legato", "unison", "chorus", "synthwave", "80s"],
        "A big synthwave lead: a seven-voice saw stack with an octave stack and a pulse, chorus, a long echo and a big room. Play slow, soaring lines; notes glide. Cutoff brightens, Spread widens the detune of the main stack, Space adds the echo and the room, Vibrato adds vibrato.",
    ),
    (
        "presets/horn-section",
        "Horn Section",
        "Lead",
        &["mono", "brass", "stabs", "funk", "disco"],
        "A synth brass lead for funk and disco: a three-voice saw stack with a second saw, a filter that swells open on every note and a light compressor. Play short stabs; harder notes are brighter. Tone opens the filter, Blat deepens the filter swell on each note, Space adds a short echo and a room, Vibrato adds a light vibrato.",
    ),
    (
        "presets/fire-escape",
        "Fire Escape",
        "Lead",
        &["mono", "unison", "stabs", "dive", "house"],
        "A house stab lead: two unison saw stacks with a pitch dive on every note, chorus and a dotted echo. Play short, bright stabs. Cutoff opens the filter, Dive sets how far each note falls into its pitch, Space adds the echo and the room, Edge adds drive.",
    ),
    (
        "presets/short-circuit",
        "Short Circuit",
        "Lead",
        &["mono", "ring mod", "bitcrushed", "distorted", "french touch", "signature"],
        "A metallic lead for French touch: a saw ring-modulated by a square a fifth up, 14 dB of drive, a bitcrusher and a heavy compressor. Play short, jagged lines. Cutoff opens the filter, Metal brings in the ring modulator, Crush mixes in the bitcrusher, Room adds a slapback and a tight room.",
    ),
    (
        "presets/fog-bank",
        "Fog Bank",
        "Pad",
        &["poly", "slow", "wide", "ambient", "cinematic"],
        "A very slow ambient cloud: a saw stack and a triangle with a long attack and a nine second room. Play long chords and let them overlap. Bloom opens the filter, Drift adds slow drift to the filter and the pitch, Depth brings in an octave of sub, Space adds the echo and the room.",
    ),
    (
        "presets/deep-field",
        "Deep Field",
        "Pad",
        &["poly", "swell", "sub", "cinematic", "ambient"],
        "A cinematic swell: saws over a deep sub with a filter that opens over several seconds. Play low, wide chords and hold them. Swell sets how far the filter opens, Sub adds an octave below, Shimmer adds an octave above, Space adds the echo and the room.",
    ),
    (
        "presets/cotton-mill",
        "Cotton Mill",
        "Pad",
        &["poly", "analog", "pulse width", "chorus", "pop"],
        "An analog-style pad for pop: a pulse with moving width and two detuned saws, soft chorus and a mid-sized room. Play open chords. Cutoff opens the filter, Motion sets how far the pulse width moves, Chorus brings in the chorus, Space adds the echo and the room.",
    ),
    (
        "presets/picture-window",
        "Picture Window",
        "Pad",
        &["poly", "supersaw", "bright", "wide", "pop"],
        "A clean, bright pad for pop: a seven-voice saw stack with a light chorus and a medium room. Play simple triads. Cutoff opens the filter, Spread widens the detune of the stack, Sparkle adds an octave of airy saw above, Space adds the echo and the room.",
    ),
    (
        "presets/drive-in",
        "Drive-In",
        "Pad",
        &["poly", "supersaw", "chorus", "vibrato", "synthwave", "80s"],
        "A lush synthwave pad: detuned saws over a square, a slow vibrato, wide chorus and a long echo. Play slow minor chords. Cutoff opens the filter, Wobble sets the depth of the vibrato, Chorus brings in the chorus, Space adds the echo and the room.",
    ),
    (
        "presets/dry-cleaner",
        "Dry Cleaner",
        "Pad",
        &["poly", "phased", "compressed", "stabs", "french touch", "signature"],
        "Phased chord stabs for French touch: two detuned saws and a pulse through a phaser, a compressor and a little drive, in a close, dry room. Play short, syncopated chords. Sweep opens the filter like a DJ, Phase brings in the phaser, Grit adds drive, Room adds the echo and the room.",
    ),
    (
        "presets/concrete-cloud",
        "Concrete Cloud",
        "Pad",
        &["poly", "distorted", "compressed", "bitcrushed", "french touch", "signature"],
        "A thick, saturated pad for French touch: saws and an octave down driven hard into a compressor, with a bitcrusher on request. Play fat, held chords. Weight sets the amount of drive, Cutoff opens the filter, Crush brings in the bitcrusher, Room adds the echo and the room.",
    ),
    (
        "presets/static-garden",
        "Static Garden",
        "Pad",
        &["poly", "evolving", "noise", "ring mod", "ambient"],
        "An evolving, textured pad: a triangle bed with a noise layer, a ring modulated layer and a filter that wanders on its own. Play slow chords. Texture brings in the noise, Motion sets how far the filter wanders, Metal adds the ring modulated layer, Space adds the echo and the room.",
    ),
    (
        "presets/copper-mile",
        "Copper Mile",
        "Groove",
        &["phased", "pumping", "french touch", "123bpm", "signature"],
        "A filtered disco loop for French touch: a soft octave bass, syncopated saw stabs and a hard kick and clap, phased and compressed, with the kick ducking the music. Press Run, then play stabs on top. Filter opens the loop like a DJ knob, Phase brings in the phaser, Pump sets how hard the kick ducks the music, Room adds a tempo-synced echo and a tight room.",
    ),
    (
        "presets/brick-dust",
        "Brick Dust",
        "Groove",
        &["mono", "distorted", "compressed", "french touch", "126bpm", "signature"],
        "A blunt, overdriven stomp: a hard-driven saw and square bass line under a short kick, snare and hats, squashed by a heavy compressor. Press Run, then play low notes over it. Filter opens the bass, Dirt adds drive, Pump sets how hard the kick ducks the music, Room adds a slapback and a tight room.",
    ),
    (
        "presets/ore-cart",
        "Ore Cart",
        "Groove",
        &["rumble", "techno", "132bpm"],
        "A rolling techno groove: a four-on-the-floor kick, off-beat hats, a rumbling bass between the kicks and a sparse stab. Press Run, then play stabs over it. Filter opens the music, Rumble raises the bass, Pump sets how hard the kick ducks it, Space adds the echo and the room.",
    ),
    (
        "presets/stoop-sale",
        "Stoop Sale",
        "Groove",
        &["swung", "dusty", "hip hop", "86bpm"],
        "A boom-bap loop: a loose kick and snare, swung hats, a sine sub bass and a soft seventh chord. Press Run, then play chords over it. Swing pushes the off-beats late, Dust adds bitcrushed grit, Filter opens the music, Space adds the echo and the room.",
    ),
    (
        "presets/fault-line",
        "Fault Line",
        "Groove",
        &["reese", "breaks", "drum and bass", "174bpm"],
        "A two-step drum and bass loop: a sharp kick and snare, ghost snares, fast hats and a detuned reese bass. Press Run, then play low notes over it. Reese adds drive to the bass, Filter opens it, Hats raises the hats, Space adds the echo and the room.",
    ),
    (
        "presets/corner-store",
        "Corner Store",
        "Groove",
        &["swung", "funk", "108bpm"],
        "A funk groove: a soft plucked bass with octave jumps, a tight kick and snare, swung hats and clav-like stabs. Press Run, then play lines over it. Swing sets the shuffle of the 16ths, Filter opens the stabs and the bass, Drive adds bite, Room adds a short echo and a small room.",
    ),
    (
        "presets/glass-coast",
        "Glass Coast",
        "Groove",
        &["arp", "chorus", "synthwave", "100bpm"],
        "A synthwave groove: a pulsing octave bass, a rolling arpeggio, a gated snare and clap and a long echo. Press Run, then play leads over it. Filter opens the music, Hats raises the hats, Chorus widens the whole mix, Space adds the long echo and the room.",
    ),
    (
        "presets/still-water",
        "Still Water",
        "Groove",
        &["slow", "sparse", "ambient", "68bpm"],
        "A sparse ambient groove: a soft kick, a few random hats and toms, a slow bass and bell-like chords with long tails. Press Run, then play slow chords over it. Bloom opens the music, Pulse raises the hats and drops, Weight raises the bass, Space adds the long echo and room.",
    ),
    (
        "presets/glacier",
        "Glacier",
        "Pad",
        &["poly", "slow", "wide", "ambient", "cinematic"],
        "A glassy, slow pad: a wide sine and triangle stack with a high glass partial, a slow swell and a long echo. Play long, open chords. Frost opens the filter and brings in the glass partial, Swell opens the filter over each held chord, Drift adds a slow pitch sway, Space adds the echo and the room.",
    ),
    (
        "presets/pump-house",
        "Pump House",
        "Pad",
        &["poly", "sidechain", "124bpm", "french touch", "signature"],
        "A chord pad that pumps with the beat for French touch: saws and an octave down, ducked on every beat of the host tempo by a silent envelope, no drum. Press Run to start the clock, then hold chords. Cutoff opens the filter, Drive pushes the saturation, Pump sets how deep the duck goes, Room adds a tempo-synced echo and a tight room.",
    ),
    (
        "presets/tin-voice",
        "Tin Voice",
        "Pad",
        &["poly", "formant", "distorted", "robot", "french touch", "signature"],
        "Robot vowel chords for French touch: saws and a pulse driven into a formant filter, close and dry. Play held chords and move the mod wheel. Vowel sweeps the mouth shape, Grit pushes the drive, Shift moves the formants up or down, Room adds a short echo and a small room.",
    ),
    (
        "presets/rooftop",
        "Rooftop",
        "Pad",
        &["poly", "analog", "wide", "brass", "pop"],
        "A brassy analog pad: detuned saws with a slow filter envelope that swells every chord. Play open triads and hold them. Cutoff sets the filter, Swell sets how far each chord opens, Chorus widens the sound, Space adds the echo and the room.",
    ),
    (
        "presets/tidepool",
        "Tidepool",
        "Pad",
        &["poly", "evolving", "wide", "ambient", "glass"],
        "An evolving glassy pad that never repeats: slow random steps in pitch and filter over a wide triangle stack. Play slow chords and let it move. Drift sets the pitch sway, Motion sets how far the filter wanders, Tone sets the brightness, Space adds the echo and the room.",
    ),
    (
        "palette/keyboard-bass",
        "Round Keyboard Bass",
        "Bass",
        &["mono", "keyboard"],
        "A playable bass with shaped attack.",
    ),
    (
        "palette/keys",
        "Glass Keys",
        "Keys",
        &["poly", "pluck"],
        "Velocity-sensitive plucked keys.",
    ),
    (
        "palette/progression",
        "Slow Horizons",
        "Pad",
        &["sequence", "minor"],
        "Editable slow harmony with four banks.",
    ),
    (
        "init-keyboard",
        "Init Keyboard",
        "Basic",
        &["starting point", "simple", "saw"],
        "A plain keyboard voice to start from: one saw oscillator, a low-pass filter and an \
         envelope. Its main controls are on the Perform panel.",
    ),
    (
        "palette/strings",
        "Ensemble Strings",
        "Strings",
        &["warm", "chorus", "poly", "evolving"],
        "Animated ensemble strings: two detuned saws, slow bow envelope, vibrato, stereo chorus.",
    ),
    (
        "palette/pad",
        "Evolving Pad",
        "Pad",
        &["warm", "slow", "poly", "evolving", "echo"],
        "Warm evolving pad: slow pulse-width motion, a breathing driven ladder filter, echo and \
         a big room.",
    ),
    (
        "palette/lead",
        "Singing Lead",
        "Lead",
        &["mono", "legato", "glide", "bright"],
        "Singing mono lead: legato glide between overlapping notes, a synced vowel on each \
         attack, drive and ping-pong echo.",
    ),
    (
        "palette/bass",
        "Sequence Bass",
        "Bass",
        &["sequenced", "resonant", "dry"],
        "Resonant sequence bass: saw plus square sub through a resonant ladder, accents from \
         the sequencer. Runs its own sequence; press Start.",
    ),
    (
        "palette/breath",
        "Breath",
        "Wind",
        &["noise", "airy", "poly", "texture"],
        "Airy wind texture: noise through a narrow band that follows the key, gusts from a slow \
         LFO, wide chorus and a long room.",
    ),
    (
        "palette/perc",
        "Noise Percussion",
        "Percussion",
        &["drums", "sequenced", "noise"],
        "Kick, snare and hat from noise and sines on three sequencers from one clock. Runs its \
         own sequence; press Start.",
    ),
    (
        "sound-palette",
        "Sound Palette Piece",
        "Piece",
        &["sequenced", "layers", "cues", "keyboard"],
        "The palette voices together: sequenced bass and percussion with section cues, and \
         four keyboard layers on faders.",
    ),
    (
        "composition",
        "Composition",
        "Piece",
        &["sequenced", "banks", "cues", "evolving", "keyboard"],
        "Interlocking sequences that move through sections: pattern banks, cues, a synced LFO \
         and macros.",
    ),
    (
        "performance",
        "Performance",
        "Piece",
        &["sequenced", "reverb", "echo", "keyboard"],
        "A sequenced piece with a plate reverb, echo, velocity and gate length, set up for the \
         Perform panel.",
    ),
    (
        "expressive/lead",
        "Expressive Lead",
        "Lead",
        &["mono", "legato", "bend", "wheel", "vibrato"],
        "The palette's mono lead set up for a controller: bend ±2 semitones, and the \
         modulation wheel adds vibrato.",
    ),
    (
        "expressive/split",
        "Keys and Bass by Channel",
        "Study",
        &["channels", "split", "bass", "wheel", "bend"],
        "Two keyboards played apart: MIDI channel 1 plays polyphonic keys (the wheel opens \
         their filter), channel 2 a legato bass with glide and a ±12 semitone bend.",
    ),
    (
        "interlocking",
        "Interlocking Sequences",
        "Piece",
        &["sequenced", "polyrhythm"],
        "A 7-step bass on 16ths against a 5-step lead on 8ths from one clock.",
    ),
    (
        "echo",
        "Echo Sequence",
        "Piece",
        &["sequenced", "echo", "delay"],
        "A sequence through the clock-synced echo.",
    ),
    (
        "sequence",
        "First Sequence",
        "Piece",
        &["sequenced", "simple"],
        "The smallest sequence: one clock and one 8-step sequencer playing a voice.",
    ),
    (
        "reference",
        "Reference Voice",
        "Study",
        &["keyboard", "modulation", "lfo"],
        "The rack's reference patch: a keyboard voice with LFO and envelope routes into the \
         filter and envelope.",
    ),
    (
        "recipes/pluck-to-pad",
        "Recipe: Pluck to Pad",
        "Study",
        &["recipe", "envelope", "keyboard"],
        "Starting patch of the Pluck to pad recipe (Learn): a plucked saw with its envelope on \
         the Perform panel.",
    ),
    (
        "recipes/filter-movement",
        "Recipe: Filter Movement",
        "Study",
        &["recipe", "filter", "lfo", "keyboard"],
        "Starting patch of the Filter movement recipe (Learn): a held saw through a resonant \
         low-pass, with an LFO route to Cutoff that starts bypassed.",
    ),
    (
        "crowded",
        "Crowded Rack",
        "Study",
        &["keyboard", "layout"],
        "Many modules and cables, for trying the rack's cable views and zoom.",
    ),
];

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

/// The initialized keyboard patch: midi.in → saw → low-pass → VCA (ADSR) → out, with its
/// five main controls pinned to the Perform panel.
fn init_keyboard() -> PatchEditor {
    let mut e = PatchEditor::new();
    let at = |row: usize, x: f32| Vec2 {
        x,
        y: kabl_ui::rack::row_y(row),
    };
    let midi = e.add_module("midi.in", at(0, 24.0));
    let osc = e.add_module("osc.va", at(0, 240.0));
    let filter = e.add_module("filter.svf", at(0, 420.0));
    let vca = e.add_module("vca", at(0, 600.0));
    let out = e.add_module("out", at(0, 760.0));
    let env = e.add_module("env.adsr", at(1, 600.0));
    e.set_param(osc, "waveform", 2.0); // saw
    e.set_param(filter, "cutoff_hz", 2200.0);
    e.set_param(filter, "resonance", 0.2);
    e.set_param(env, "attack_ms", 8.0);
    e.set_param(env, "decay_ms", 300.0);
    e.set_param(env, "sustain", 0.7);
    e.set_param(env, "release_ms", 350.0);
    e.set_param(vca, "gain", 0.0);
    e.connect(port(midi, "pitch"), port(osc, "pitch"));
    e.connect(port(osc, "out"), port(filter, "in"));
    e.connect(port(filter, "lp"), port(vca, "in"));
    e.connect(port(midi, "gate"), port(env, "gate"));
    e.connect(port(env, "out"), port(vca, "cv"));
    e.connect(port(vca, "out"), port(out, "left"));
    e.connect(port(vca, "out"), port(out, "right"));
    let pins: [(ModuleId, &str, &str); 5] = [
        (filter, "cutoff_hz", "Brightness"),
        (filter, "resonance", "Resonance"),
        (env, "attack_ms", "Attack"),
        (env, "release_ms", "Release"),
        (osc, "waveform", "Wave"),
    ];
    let changes: Vec<_> = pins
        .iter()
        .enumerate()
        .map(|(i, (id, key, _))| (*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)))
        .collect();
    e.set_presentation(&changes);
    for (id, key, label) in pins {
        e.set_label(id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
    }
    e.migrated()
}

#[test]
#[ignore = "writes patches/init-keyboard and every factory sound.toml"]
fn write_factory_library() {
    let root = factory();
    kabl_core::save(&root.join("init-keyboard"), init_keyboard().log()).unwrap();
    kabl_core::save(
        &root.join("recipes/pluck-to-pad"),
        kabl_ui::recipes::pluck_to_pad().log(),
    )
    .unwrap();
    kabl_core::save(
        &root.join("recipes/filter-movement"),
        kabl_ui::recipes::filter_movement().log(),
    )
    .unwrap();
    for (dir, name, category, tags, description) in FACTORY {
        let log = kabl_core::load(&root.join(dir)).unwrap();
        let (keys, sequence) = Meta::play_of(log.state());
        let meta = Meta {
            name: name.to_string(),
            category: category.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            description: description.split_whitespace().collect::<Vec<_>>().join(" "),
            keys,
            sequence,
        };
        fs::write(
            root.join(dir).join(library::META_FILE),
            toml::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();
    }
    factory_bank::write_metadata(&root);
}

fn open(user: &Path) -> Library {
    Library::open(Some(factory()), user.to_path_buf())
}

#[test]
fn the_recipe_patches_are_what_the_code_builds() {
    for (dir, e) in [
        ("recipes/pluck-to-pad", kabl_ui::recipes::pluck_to_pad()),
        (
            "recipes/filter-movement",
            kabl_ui::recipes::filter_movement(),
        ),
    ] {
        let saved = kabl_core::load(&factory().join(dir)).unwrap();
        assert_eq!(saved.state(), e.state(), "{dir}");
    }
}

#[test]
fn the_init_keyboard_file_is_what_the_code_builds() {
    let saved = kabl_core::load(&factory().join("init-keyboard")).unwrap();
    assert_eq!(saved.state(), init_keyboard().state());
}

#[test]
fn every_factory_sound_is_discoverable_categorized_and_loads() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = open(tmp.path());
    assert!(lib.notes.is_empty(), "{:?}", lib.notes);
    let factory: Vec<_> = lib
        .entries
        .iter()
        .filter(|e| e.origin == Origin::Factory)
        .collect();
    assert_eq!(
        factory.len(),
        FACTORY.len(),
        "no stray or missing factory sound"
    );
    for (dir, name, category, _, _) in FACTORY {
        let e = lib
            .get(&format!("factory:{dir}"))
            .unwrap_or_else(|| panic!("{dir} discovered"));
        assert_eq!(e.meta.name, *name);
        assert_eq!(e.meta.category, *category);
        assert!(CATEGORIES.contains(category));
        let log = lib.read(&e.id).unwrap_or_else(|err| panic!("{dir}: {err}"));
        assert_eq!(
            (e.meta.keys, e.meta.sequence),
            Meta::play_of(log.state()),
            "{dir}: the saved play flags describe the patch"
        );
    }
    // Voices vs pieces, as the browser shows them.
    let keys_only = |id: &str| {
        let m = &lib.get(id).unwrap().meta;
        m.keys && !m.sequence
    };
    for v in ["strings", "pad", "lead", "breath"] {
        assert!(
            keys_only(&format!("factory:palette/{v}")),
            "{v} is a keyboard voice"
        );
    }
    assert!(keys_only("factory:init-keyboard"));
    for s in [
        "palette/bass",
        "palette/perc",
        "sequence",
        "interlocking",
        "echo",
    ] {
        let m = &lib.get(&format!("factory:{s}")).unwrap().meta;
        assert!(m.sequence && !m.keys, "{s} runs a sequence");
    }
    let m = &lib.get("factory:composition").unwrap().meta;
    assert!(m.sequence && m.keys, "a piece you can also play");
}

#[test]
fn search_matches_every_word_across_name_category_and_tags() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = open(tmp.path());
    let hits = |q: &str| -> Vec<String> {
        lib.entries
            .iter()
            .filter(|e| e.matches(q))
            .map(|e| e.id.clone())
            .collect()
    };
    assert_eq!(hits("singing"), ["factory:palette/lead"]);
    assert_eq!(hits("PAD warm"), ["factory:palette/pad"]);
    assert!(hits("sequenced").contains(&"factory:palette/bass".to_string()));
    assert!(hits("no such sound").is_empty());
    assert_eq!(hits("").len(), FACTORY.len());
}

fn edited_pad(lib: &Library) -> PatchEditor {
    let mut e = PatchEditor::from_log(lib.read("factory:palette/pad").unwrap());
    let (&id, _) = e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "filter.ladder")
        .unwrap();
    e.set_param(id, "cutoff_hz", 3333.0);
    e
}

fn dir_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn save_as_reopen_rename_and_the_factory_file_is_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let before = dir_bytes(&factory().join("palette/pad"));
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "My Pad".into(),
        category: "Pad".into(),
        tags: vec!["mine".into()],
        ..Meta::default()
    };
    assert!(matches!(
        lib.save(e.log(), meta.clone(), Some("factory:palette/pad")),
        Err(LibError::Factory)
    ));
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    assert_eq!(id, "user:my-pad");
    assert_eq!(dir_bytes(&factory().join("palette/pad")), before);

    // Found again after a restart, with its edit and full history.
    let mut lib = open(tmp.path());
    let saved = lib.get(&id).unwrap().clone();
    assert_eq!(saved.origin, Origin::User);
    assert_eq!(saved.meta.name, "My Pad");
    assert!(saved.meta.keys && !saved.meta.sequence);
    assert_eq!(
        lib.prefs.recents.first().map(String::as_str),
        Some(id.as_str())
    );
    let back = lib.read(&id).unwrap();
    assert_eq!(back.state(), e.state());
    assert_eq!(
        back.entries().len(),
        e.log().entries().len(),
        "undo history kept"
    );
    assert!(lib.entries.iter().any(|x| x.matches("my pad mine")));

    // Rename keeps the identity (and so favorites/recents); the directory stays.
    lib.toggle_favorite(&id);
    lib.set_meta(
        &id,
        Meta {
            name: "Brighter Pad".into(),
            ..saved.meta.clone()
        },
    )
    .unwrap();
    let lib = open(tmp.path());
    assert_eq!(lib.get(&id).unwrap().meta.name, "Brighter Pad");
    assert!(lib.prefs.favorites.contains(&id));
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
}

#[test]
fn names_and_directories_never_collide_silently() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let named = |n: &str| Meta {
        name: n.into(),
        ..Meta::default()
    };
    let a = lib.save(e.log(), named("Pad One"), None).unwrap();
    // Same name, other case: refused, naming the sound that has it.
    match lib.save(e.log(), named(" pad one "), None) {
        Err(LibError::NameTaken(id)) => assert_eq!(id, a),
        other => panic!("{other:?}"),
    }
    // Replacing it is explicit.
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    assert_eq!(lib.save(e2.log(), named("Pad One"), Some(&a)).unwrap(), a);
    assert_eq!(lib.read(&a).unwrap().state(), e2.state());
    // A different name with the same directory slug gets its own directory.
    let b = lib.save(e.log(), named("Pad-One!"), None).unwrap();
    assert_eq!(b, "user:pad-one-2");
    assert_eq!(lib.read(&a).unwrap().state(), e2.state(), "a untouched");
    // Rename onto a taken name is refused and changes nothing.
    assert!(matches!(
        lib.set_meta(&b, named("PAD ONE")),
        Err(LibError::NameTaken(_))
    ));
    assert_eq!(lib.get(&b).unwrap().meta.name, "Pad-One!");
    // Factory names don't block user names: identity is not the name.
    lib.save(e.log(), named("Evolving Pad"), None).unwrap();
    assert!(matches!(
        lib.save(e.log(), named("   "), None),
        Err(LibError::BadName(_))
    ));
    assert!(matches!(
        lib.set_meta("factory:palette/pad", named("x")),
        Err(LibError::Factory)
    ));
}

#[test]
fn a_failed_save_leaves_the_old_sound_and_explains() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "Keep".into(),
        ..Meta::default()
    };
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    let dir = lib.get(&id).unwrap().dir.clone();
    let before = dir_bytes(&dir);
    // The staging directory's place is taken by a file: the save fails before any swap.
    fs::write(tmp.path().join("sounds/.keep.new"), b"in the way").unwrap();
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    let err = lib.save(e2.log(), meta, Some(&id)).unwrap_err();
    assert!(matches!(err, LibError::Io(_)), "{err:?}");
    assert!(!err.to_string().is_empty());
    assert_eq!(dir_bytes(&dir), before, "the saved sound is intact");
}

#[cfg(unix)]
#[test]
fn a_read_only_user_directory_is_an_error_not_a_panic() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ro");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores permissions; a user root that is a file fails the same way for everyone.
    let root = if fs::write(root.join("probe"), b"").is_ok() {
        let file = tmp.path().join("a-file");
        fs::write(&file, b"").unwrap();
        file
    } else {
        root
    };
    let mut lib = open(&root);
    let e = edited_pad(&lib);
    let err = lib
        .save(
            e.log(),
            Meta {
                name: "Nope".into(),
                ..Meta::default()
            },
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("can't create"), "{err}");
    assert!(
        lib.toggle_favorite("factory:palette/pad").is_some(),
        "reported"
    );
    assert!(
        lib.prefs.favorites.contains("factory:palette/pad"),
        "kept for the session"
    );
}

#[test]
fn an_interrupted_save_is_repaired_on_the_next_scan() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let id = lib
        .save(
            e.log(),
            Meta {
                name: "Swap".into(),
                ..Meta::default()
            },
            None,
        )
        .unwrap();
    let sounds = tmp.path().join("sounds");
    // Stopped after "target → .old", before ".new → target": the old one comes back.
    fs::rename(sounds.join("swap"), sounds.join(".swap.old")).unwrap();
    fs::create_dir_all(sounds.join(".swap.new")).unwrap();
    let lib = open(tmp.path());
    assert!(
        lib.notes.iter().any(|n| n.contains("restored")),
        "{:?}",
        lib.notes
    );
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
    assert!(!sounds.join(".swap.new").exists() && !sounds.join(".swap.old").exists());
    // Stopped after ".new → target": the leftover .old is removed.
    fs::create_dir_all(sounds.join(".swap.old")).unwrap();
    let lib = open(tmp.path());
    assert!(!sounds.join(".swap.old").exists());
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
}

#[test]
fn malformed_and_uncompilable_patches_are_reported_not_loaded() {
    let tmp = tempfile::tempdir().unwrap();
    let sounds = tmp.path().join("sounds");
    // A copied-in patch whose log is cut short.
    let bad = sounds.join("broken");
    fs::create_dir_all(&bad).unwrap();
    fs::copy(factory().join("reference/meta.toml"), bad.join("meta.toml")).unwrap();
    fs::write(
        bad.join("log.jsonl"),
        "{\"seq\":0,\"t_ms\":0,\"op\":{\"AddMod",
    )
    .unwrap();
    // One that parses but names a module kind kabl doesn't have.
    let mut e = PatchEditor::new();
    e.add_module("no.such.module", Vec2::default());
    kabl_core::save(&sounds.join("alien"), e.log()).unwrap();
    // Unreadable metadata falls back to the directory name.
    let odd = sounds.join("odd");
    kabl_core::save(&odd, init_keyboard().log()).unwrap();
    fs::write(odd.join(library::META_FILE), "name = [").unwrap();

    let lib = open(tmp.path());
    assert!(matches!(
        lib.read("user:broken"),
        Err(LibError::Unreadable(_))
    ));
    let err = lib.read("user:alien").unwrap_err();
    assert!(matches!(err, LibError::Uncompilable(_)), "{err:?}");
    assert!(err.to_string().contains("no.such.module"), "{err}");
    assert_eq!(lib.get("user:odd").unwrap().meta.name, "odd");
    assert!(lib.notes.iter().any(|n| n.contains("odd")));
    assert!(lib.read("user:odd").is_ok());
    assert!(matches!(
        lib.read("user:gone"),
        Err(LibError::Unreadable(_))
    ));
}

#[test]
fn old_patch_directories_without_metadata_are_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("sounds/old-song");
    fs::create_dir_all(&old).unwrap();
    for f in ["log.jsonl", "meta.toml", "checkpoint.json"] {
        fs::copy(factory().join("interlocking").join(f), old.join(f)).unwrap();
    }
    let lib = open(tmp.path());
    let e = lib.get("user:old-song").unwrap();
    assert_eq!(e.meta.name, "old-song");
    assert!(e.meta.sequence && !e.meta.keys);
    assert!(lib.read(&e.id).is_ok());
}

#[test]
fn favorites_and_recents_persist_and_missing_ones_are_kept_until_forgotten() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    assert!(lib.toggle_favorite("factory:palette/lead").is_none());
    lib.touch("factory:palette/pad");
    lib.touch("user:deleted-later");
    lib.touch("factory:palette/pad");
    let mut lib = open(tmp.path());
    assert!(lib.prefs.favorites.contains("factory:palette/lead"));
    assert_eq!(
        lib.prefs.recents,
        ["factory:palette/pad", "user:deleted-later"],
        "newest first, no duplicates"
    );
    assert!(lib.get("user:deleted-later").is_none());
    lib.forget("user:deleted-later");
    lib.toggle_favorite("factory:palette/lead");
    let lib = open(tmp.path());
    assert_eq!(lib.prefs.recents, ["factory:palette/pad"]);
    assert!(lib.prefs.favorites.is_empty());
    for i in 0..30 {
        let mut l = open(tmp.path());
        l.touch(&format!("user:s{i}"));
    }
    assert_eq!(open(tmp.path()).prefs.recents.len(), library::MAX_RECENTS);
}

#[test]
fn a_missing_factory_directory_is_a_note_not_a_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::open(None, tmp.path().to_path_buf());
    assert!(lib.entries.is_empty());
    assert!(lib.notes[0].contains("KABL_FACTORY_DIR"));
    let lib = Library::open(Some(tmp.path().join("nowhere")), tmp.path().to_path_buf());
    assert!(lib.entries.is_empty());
}

#[test]
fn factory_paths_are_recognized_for_the_advanced_folder_save() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = open(tmp.path());
    assert!(lib.is_factory_path(&factory().join("palette/pad")));
    assert!(lib.is_factory_path(&factory().join("new/sub/dir")));
    assert!(lib.is_factory_path(&factory().join("palette/../reference")));
    assert!(!lib.is_factory_path(&tmp.path().join("x")));
}

#[test]
fn slugs_are_plain() {
    assert_eq!(library::slug("  My  Pad!! 2 "), "my-pad-2");
    assert_eq!(library::slug("???"), "sound");
    assert_eq!(library::slug("Čelo"), "čelo");
}

// ---- D01-R1 repairs (docs/find-play-save/repair-1) ----

fn files(d: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(d)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn a_replacement_must_name_the_sound_it_replaces() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let named = |n: &str| Meta {
        name: n.into(),
        ..Meta::default()
    };
    let a = lib.save(e.log(), named("Alpha"), None).unwrap();
    let before = dir_bytes(&lib.get(&a).unwrap().dir);
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    // A stale confirmation: target Alpha, name Beta.
    let err = lib.save(e2.log(), named("Beta"), Some(&a)).unwrap_err();
    assert!(matches!(err, LibError::ReplaceMismatch { .. }), "{err:?}");
    assert_eq!(dir_bytes(&lib.get(&a).unwrap().dir), before);
    assert!(lib.get("user:beta").is_none());
    // Legitimate: Save in place and a replacement of the sound with that name.
    assert_eq!(lib.save(e2.log(), named("ALPHA"), Some(&a)).unwrap(), a);
    assert_eq!(lib.read(&a).unwrap().state(), e2.state());
}

#[test]
fn a_user_save_failing_at_any_stage_keeps_the_saved_sound() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "Keep".into(),
        ..Meta::default()
    };
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    let dir = lib.get(&id).unwrap().dir.clone();
    let before = dir_bytes(&dir);
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    for stage in ["staged", "replace:dir"] {
        library::fail_saves_at(Some(stage));
        let err = lib.save(e2.log(), meta.clone(), Some(&id)).unwrap_err();
        library::fail_saves_at(None);
        assert!(err.to_string().contains(stage), "{err}");
        assert_eq!(dir_bytes(&dir), before, "{stage}");
        let names: Vec<_> = fs::read_dir(tmp.path().join("sounds"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["keep"], "{stage}: nothing left over");
    }
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
}

#[test]
fn a_folder_save_writes_the_patch_and_keeps_other_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    // A new folder.
    library::save_folder(&dir, e.log()).unwrap();
    assert_eq!(kabl_core::load(&dir).unwrap().state(), e.state());
    // An existing one with the user's own files and metadata.
    fs::write(dir.join("notes.txt"), "mine").unwrap();
    fs::write(dir.join(library::META_FILE), "name = \"Song\"\n").unwrap();
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.set_param(3, "cutoff_hz", 900.0);
    library::save_folder(&dir, e2.log()).unwrap();
    assert_eq!(library::read_patch(&dir).unwrap().state(), e2.state());
    assert_eq!(fs::read(dir.join("notes.txt")).unwrap(), b"mine");
    assert_eq!(
        fs::read(dir.join(library::META_FILE)).unwrap(),
        b"name = \"Song\"\n"
    );
    assert!(!dir.join(".kabl-save").exists());
}

#[test]
fn a_folder_save_failing_at_any_stage_leaves_the_folder_as_it_was() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    kabl_core::save(&dir, e.log()).unwrap();
    fs::write(dir.join("notes.txt"), "mine").unwrap();
    let before = files(&dir);
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.set_param(3, "cutoff_hz", 900.0);
    for stage in [
        "staged",
        "replace:meta.toml",
        "replace:checkpoint.json",
        "replace:log.jsonl",
    ] {
        library::fail_saves_at(Some(stage));
        let err = library::save_folder(&dir, e2.log()).unwrap_err();
        library::fail_saves_at(None);
        assert!(matches!(err, LibError::Io(_)), "{stage}: {err:?}");
        assert_eq!(files(&dir), before, "{stage}");
        assert!(!dir.join(".kabl-save").exists(), "{stage}");
        assert_eq!(kabl_core::load(&dir).unwrap().state(), e.state());
    }
    // A new folder whose first save fails is left without patch files, not half of them.
    let fresh = tmp.path().join("fresh");
    library::fail_saves_at(Some("replace:log.jsonl"));
    assert!(library::save_folder(&fresh, e2.log()).is_err());
    library::fail_saves_at(None);
    assert!(files(&fresh).is_empty());
}

/// Builds the state a folder save killed part way leaves: staging with kabl's marker in
/// `phase`, the new copy staged, and `moved` files already swapped (old aside, new in).
fn crashed_save(dir: &Path, new: &PatchEditor, phase: &str, moved: &[&str]) {
    let staging = dir.join(".kabl-save");
    kabl_core::save(&staging, new.log()).unwrap();
    fs::write(staging.join("kabl-staging"), phase).unwrap();
    for f in moved {
        if dir.join(f).exists() {
            fs::rename(dir.join(f), staging.join(format!("{f}.old"))).unwrap();
        }
        fs::rename(staging.join(f), dir.join(f)).unwrap();
    }
}

#[test]
fn an_interrupted_folder_save_is_repaired_on_the_next_open() {
    let tmp = tempfile::tempdir().unwrap();
    let old = init_keyboard();
    let mut new = PatchEditor::from_log(old.log().clone());
    new.set_param(3, "cutoff_hz", 900.0);
    let complete = |name: &str| {
        let dir = tmp.path().join(name);
        kabl_core::save(&dir, old.log()).unwrap();
        (dir.clone(), files(&dir))
    };

    // Killed with meta and checkpoint swapped and the old log moved aside: rolled back.
    let (dir, before) = complete("a");
    crashed_save(&dir, &new, "replacing", &["meta.toml", "checkpoint.json"]);
    fs::rename(dir.join("log.jsonl"), dir.join(".kabl-save/log.jsonl.old")).unwrap();
    let note = library::repair_folder(&dir).expect("says it restored");
    assert!(note.contains("undone"), "{note}");
    assert_eq!(files(&dir), before);
    assert!(!dir.join(".kabl-save").exists());

    // Killed after the new log moved in, before cleanup: the save stands.
    let (dir, _) = complete("b");
    crashed_save(
        &dir,
        &new,
        "replacing",
        &["meta.toml", "checkpoint.json", "log.jsonl"],
    );
    assert_eq!(library::read_patch(&dir).unwrap().state(), new.state());
    assert!(!dir.join(".kabl-save").exists());

    // Killed while the new copy was still being written: the folder never changed.
    let (dir, before) = complete("c");
    crashed_save(&dir, &new, "writing", &[]);
    assert_eq!(library::read_patch(&dir).unwrap().state(), old.state());
    assert_eq!(files(&dir), before);
    assert!(!dir.join(".kabl-save").exists());

    // A folder without checkpoint.json: the checkpoint the save added goes on rollback.
    let (dir, _) = complete("d");
    fs::remove_file(dir.join("checkpoint.json")).unwrap();
    let before = files(&dir);
    crashed_save(&dir, &new, "replacing", &["meta.toml", "checkpoint.json"]);
    library::repair_folder(&dir);
    assert_eq!(files(&dir), before, "no stray checkpoint");

    // A brand-new folder: rollback leaves no half patch.
    let dir = tmp.path().join("e");
    fs::create_dir_all(&dir).unwrap();
    crashed_save(&dir, &new, "replacing", &["meta.toml", "checkpoint.json"]);
    library::repair_folder(&dir);
    assert!(files(&dir).is_empty());
    assert!(!dir.join(".kabl-save").exists());
}

#[test]
fn a_kabl_save_directory_kabl_didnt_make_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    kabl_core::save(&dir, e.log()).unwrap();
    fs::create_dir_all(dir.join(".kabl-save")).unwrap();
    fs::write(dir.join(".kabl-save/precious.txt"), "mine").unwrap();
    fs::write(dir.join(".kabl-save/log.jsonl.old"), "not kabl's").unwrap();
    let before = files(&dir);
    assert!(library::repair_folder(&dir).is_none());
    assert!(library::read_patch(&dir).is_ok());
    let err = library::save_folder(&dir, e.log()).unwrap_err();
    assert!(err.to_string().contains("in the way"), "{err}");
    assert_eq!(files(&dir), before);
    assert_eq!(
        fs::read(dir.join(".kabl-save/precious.txt")).unwrap(),
        b"mine"
    );
    // Even kabl's own staging keeps anything else put in it.
    let other = tmp.path().join("other");
    kabl_core::save(&other, e.log()).unwrap();
    crashed_save(&other, &e, "writing", &[]);
    fs::write(other.join(".kabl-save/extra.txt"), "x").unwrap();
    library::repair_folder(&other);
    assert_eq!(fs::read(other.join(".kabl-save/extra.txt")).unwrap(), b"x");
}

#[test]
fn a_retry_after_an_interrupted_user_save_keeps_the_only_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "Keep".into(),
        ..Meta::default()
    };
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    let sounds = tmp.path().join("sounds");
    // What an Interrupted save leaves: the only copy aside as .keep.old.
    fs::rename(sounds.join("keep"), sounds.join(".keep.old")).unwrap();
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    library::fail_saves_at(Some("replace:dir"));
    assert!(lib.save(e2.log(), meta, Some(&id)).is_err());
    library::fail_saves_at(None);
    assert_eq!(
        lib.read(&id).unwrap().state(),
        e.state(),
        "the old copy survives"
    );
}

#[test]
fn a_save_killed_during_cleanup_stands() {
    let tmp = tempfile::tempdir().unwrap();
    let old = init_keyboard();
    let mut new = PatchEditor::from_log(old.log().clone());
    new.set_param(3, "cutoff_hz", 900.0);
    for (name, existing) in [("existing", true), ("brand-new", false)] {
        for phase in ["replacing", "done"] {
            let dir = tmp.path().join(format!("{name}-{phase}"));
            if existing {
                kabl_core::save(&dir, old.log()).unwrap();
            }
            library::save_folder(&dir, new.log()).unwrap();
            let saved = files(&dir);
            // Cleanup got as far as removing every staged file but the marker.
            fs::create_dir_all(dir.join(".kabl-save")).unwrap();
            fs::write(dir.join(".kabl-save/kabl-staging"), phase).unwrap();
            assert_eq!(
                library::read_patch(&dir).unwrap().state(),
                new.state(),
                "{name} {phase}"
            );
            assert_eq!(files(&dir), saved, "{name} {phase}");
            assert!(!dir.join(".kabl-save").exists());
        }
    }
}

#[test]
fn leftover_staging_is_cleared_or_explained() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    kabl_core::save(&dir, e.log()).unwrap();
    // Empty (kabl stopped right after making it): the save goes ahead.
    fs::create_dir_all(dir.join(".kabl-save")).unwrap();
    library::save_folder(&dir, e.log()).unwrap();
    // kabl's own, which could not be undone: the save says where the old files are.
    let staging = dir.join(".kabl-save");
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("kabl-staging"), "replacing").unwrap();
    fs::write(staging.join("log.jsonl"), "staged").unwrap();
    fs::create_dir_all(staging.join("log.jsonl.old")).unwrap(); // can't be renamed back over a file
    fs::write(staging.join("log.jsonl.old/x"), "").unwrap();
    let note = library::repair_folder(&dir).unwrap();
    assert!(note.contains("could not be undone"), "{note}");
    let err = library::save_folder(&dir, e.log()).unwrap_err();
    assert!(matches!(err, LibError::Interrupted(_)), "{err:?}");
    assert!(err.to_string().contains("kept in"), "{err}");
}
