# Wavetable import and table editor

Branch `claude/wavetable-editor`. The `osc.wt` oscillator can play a table of the user's own: import a `.wav`, shape it in an editor, and the table travels inside the patch (and inside the plugin state), so a sound is the same on another machine. Nothing here has been heard by a person; the clips are for Kosta's ears.

## Use

- **Open.** Every `osc.wt` face has a small **Edit** chip in the corner of its display. It opens the editor window (egui window, works the same in the standalone app and in the CLAP editor).
- **Import.** Type or paste the path of a `.wav` in the path field and press **Import**. The window says what it found ("16 frames of 2048 samples (a multiple of 2048 samples, no clm chunk)") and previews the frames in the stack. **Frame size** (Auto, 256 to 16384, Whole file) corrects a wrong reading; the preview redraws. **Use as table** writes it; **Cancel** drops it.
- **Edit.** The stack on the left shows all frames (front = frame 1); click a frame to select it, shift-click a second one to mark a range. **Draw** is the pen on the selected frame; **Harmonics** are bars for harmonics 1 to 64 (square-root scale, so quiet upper harmonics are reachable). **Insert** (a sine after the selection), **Duplicate**, **Delete**, **Move earlier / later** reorder. **Normalise**, **Remove DC**, **Fade edges** act on the selected range. With two frames marked, **Interpolate** (waveform crossfade) or **Morph** (harmonic by harmonic, amplitude and phase) fills the frames between them; when the two are adjacent, "N new" frames are inserted first.
- **Audition.** The oscillator plays each edit as it is made: a pen stroke or bar drag writes the table at most every 0.12 s, and a release writes the final one. **Audition** holds a note (C3, up to 10 s) through the patch's keyboard; the **Position** slider is the module's own `position`. A note played from MIDI or the keyboard works the same.
- **Undo.** Every operation, and every pen stroke or bar drag as a whole, is one step of the application's undo (Undo and Redo buttons in the window, and the usual shortcuts). The first edit of a factory table writes a copy to the first free slot and points the module's `user` at it in the same step.

## Formats and frame size

`import_wav_with` (crates/modules/src/wavetable.rs) reads RIFF/WAVE: PCM 8, 16, 24, 32 bit, float 32 and 64 bit, WAVE_FORMAT_EXTENSIBLE; any sample rate (the rate is ignored: a table is cycles, not time); any channel count (channels are averaged). Frame length is chosen in this order:

1. a `clm ` chunk (Serum and Vital write `<!>2048 ...`) when it divides the file length;
2. else, a file whose length is a multiple of 2048 samples is that many frames of 2048;
3. else the whole file is one cycle of its own length.

Case 2 is **ambiguous** (a 16384-sample file could be one long cycle): the window flags it and the frame-size choice corrects it. A chosen length cuts the file into whole frames (a "longer audio file sliced into frames"); a shorter tail is dropped and the window says how many samples. Every frame is resampled to 2048 samples by an FFT (or, for lengths that are not a power of two, a direct transform), band-limited to 512 harmonics, DC removed, and the whole table peak-normalised to 0.98: the same canonical form the bundled tables have. The mipmaps the oscillator reads are built from that (`WaveTable::from_frames`), so an imported table cannot alias more than a bundled one.

Not in: resampling in time (pitch-detecting a recording to find its cycle), spectral resynthesis of arbitrary audio, formats other than WAV, drag-and-drop of files.

## Limits (all refused with a message, none a crash)

| What | Limit | Where |
|---|---|---|
| file size | 32 MiB (`MAX_IMPORT_BYTES`; the window reads at most this many bytes of a file, so a huge or sparse file is never loaded) | `parse_wav`, `View::read_file` |
| frames per table | 64 | `MAX_FRAMES` |
| frame length read from a file | 16 to 32768 samples | `MAX_FRAME_SAMPLES` |
| total samples in a non-power-of-two import | 65536 (direct transform; power-of-two frames use the FFT) | `MAX_DFT_SAMPLES` |
| channels | at least 1; averaged | `parse_wav` |
| sample values | NaN or infinity refused; a silent file refused | `import_wav_with` |
| tables per patch | 8 slots (`MAX_TABLES`) | `kabl_core::table` |
| embedded table | 64 x 2048 x 2 bytes plus header | `MAX_TABLE_BYTES` |
| the patch with its tables | 2 MiB saved (`put_table` refuses a write that would pass it) | `PatchEditor::put_table` |

A truncated or lying chunk is `Truncated` (a cut-short `data` chunk is the one exception: streamed files import what is there).

## Decisions

- **Embed the table in the patch; do not reference a file.** A referenced file makes the same patch sound different (or silent) on another machine and in a host project that outlives the folder. Embedding is what composites and panel art already do, it makes the plugin state complete (REAPER recalls it with no files), and it was already in the patch format (schema 5, `Op::SetTable`). Cost: a patch carries up to 256 KiB per table (base64 inside JSON), and the undo log keeps one table per edit gesture. Both are bounded by the limits above.
- **Schema stays 7.** The data shape already existed; this change adds editing, not a new field. `crates/engine/tests/golden_renders.rs` (the 26 committed patches rendered) and the factory bank tests prove old patches and the factory bank load and render unchanged.
- **One write path.** The editor writes the table with `PatchEditor::put_table` and lets the existing compile-and-crossfade swap carry it to the audio thread. No new audio-thread code: the table is decoded and its mipmaps built off the audio thread (`from_canonical_cached`), and the new graph is swapped in with the same linear crossfade as any live edit. `crates/engine/tests/table_swap.rs` swaps twenty different tables under a held note: no allocation, no step larger than the tables' own slope.
- **Pen strokes are one undo step and one log entry.** `PatchLog` merges a continuing `SetTable` into the entry the gesture started (as it does for knob drags), so dragging does not write a table per frame into the saved log.
- **File dialog: a path field.** The app is egui with no native dialog dependency; a modal native dialog opened from a plugin window cannot be parented to the host's window reliably. The window takes a path to a local file, the same way panel art and composite packages are imported. This works the same in the standalone app and in a host. Gap: no browse button and no drag-and-drop yet.

## Frame time

`cargo run --release -p kabl-ui --example bench_chrome 300 live [editor|draw]` (dense Composition, audio running, displays live, rack only). `editor`: the editor window is open on an `osc.wt` (one is added if the patch has none). `draw`: the same, and a pen stroke is down and moving every frame, so the table is written every 0.12 s during the measured frames. Three runs each on a machine another session was also using (so noisy; ranges are over the runs):

| 1440x900 | median | p95 | max |
|---|---|---|---|
| rack only (no editor) | 3.6 to 5.1 ms | 3.9 to 5.6 ms | 4.4 to 6.3 ms |
| editor open | 4.2 to 7.0 ms | 4.5 to 7.4 ms | 5.8 to 8.3 ms |
| editor open, pen stroke down | 6.5 to 8.1 ms | 7.8 to 10.5 ms | 8.9 to 12.0 ms |

1280x800 is 2.4 to 3.6 ms median with the editor open and 3.7 to 4.7 ms while drawing, max 7.7 ms. Median stays under 8 ms except one noisy 1440x900 dark run while drawing (8.1 ms); max stays under 16 ms in every run. A table write costs about 0.1 to 0.2 ms (measured inside `commit`); the rest of the extra time while drawing was not traced (the pointer also moves over the rack in that bench). Earlier builds copied the whole table on every frame to draw the stack (median 6.4 to 7.9 ms with the editor open); the stack now draws from the working copy.

## Clips and captures

`cargo test -p kabl-ui --test wavetable_editor write_clips -- --ignored` writes `target/wt-clips/{1-imported-sweep,2-drawn-table,3-morph-filled}.wav`. The tables are made in the editor window with real pointer input in the test harness, then played by the committed `imported-morph` patch with its envelope slowed so a held note sweeps the whole table. The imported demo table is computed by `make_demo_table.py` (no third-party material). `media/` has the captures.

## Real host (REAPER 7.75, Linux)

The CLAP build opens the same editor in REAPER's FX window (1180x680 plugin editor). Done with the scripts in this folder (`host-make-project.py` builds the project, `host-run.lua` is the command loop REAPER runs, `host-features.py` compares renders; paths are those of the capture machine):

1. A project with the plugin on one track, the committed `imported-morph` patch (factory table selected, its envelope slowed), and a MIDI item.
2. In the plugin editor, with real X input: **Edit** chip, path field typed with `xdotool`, **Import**, **Use as table**, a frame selected in the stack, a pen stroke on it. REAPER marks the project modified (the plugin reports the state change). Captures: `media/plugin-1180x680-editor-draw-light.webp` and `-dark.webp`.
3. **Save** (the host's own: `File > Save` through the "Save project before closing?" query), quit REAPER. The project file carries the table inside the plugin state (about 136 KB for a 16-frame table).
4. The imported `.wav` was moved away, a **fresh REAPER process** opened the project, and the editor shows the same 16 frames with the drawn frame on top (`media/reaper-recalled-editor.webp`): the table is recalled from the state, not from a file.
5. Renders. Two fresh processes rendered the recalled project (a note from 4.0 s to 6.0 s): the rendered sound agrees in level and spectrum in each 0.5 s window (RMS within 3 %, spectral centroid within 6 %: e.g. 4.5 to 5.0 s, 0.0185 / 621 Hz and 0.0191 / 597 Hz). It is not sample-identical: oscillators free-run, so their phase at a note depends on how long the instance ran; the same holds for two renders in one process (peak 0.0654 and 0.0627) and was already so before this change.

**State how the file dialog works in a host:** there is none. The window takes a path typed or pasted into its field, in the standalone app and in the plugin alike. Typing works in REAPER's floating and docked FX windows.

**Seen, not caused by this change, not fixed:** in REAPER's offline render, a project whose patch has an *embedded table* plays nothing for the first three seconds or so of each render, whatever the table: a note at 0.5 s is silent, a note at 4 s sounds (the committed `imported-morph` patch, unmodified, behaves the same; the same patch with a factory table sounds from 0.5 s). Renders in the report therefore put the note at 4 s. Whoever picks up the 100-preset batch should look at this before relying on offline renders of table presets.
