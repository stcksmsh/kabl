# Handover: keys/pluck second half (branch claude/macros-keys-pluck-2)

Done: all 11 presets (Matinee .. Tuning Fork) redesigned in `tools/examples/presets_keys_pluck/group2.rs`, patches regenerated (first-half patch files untouched), library descriptions, doc section in `docs/presets/MACRO-REDESIGN-KEYS-PLUCK.md`, tours for all 11 in `docs/presets/clips/macros-keys-pluck-2/` (git-ignored; old Brass/Tuning Fork versions).
Done in this last commit (unverified): Brass Filament Hold replaced by Space (echo + room, level-compensated); Tuning Fork renamed Free Bar (slug, patch folder, library.rs, trims), Ring and Beat pushed further, Ring stored 1.0 with longer stored decays (deliberate default change, for loudness); Hold/Warp level compensation.
Remaining:
1. `cargo test --release -p kabl-ui --test factory_bank factory_levels_hold_the_target` (last run: only cold-cut "macro sweep peaks above -1"; lower `cold-cut` in `tools/examples/preset_trims.inc` by 2 to 3 dB, then `presets_keys_pluck write patches` and `git checkout` the first-twelve patch dirs). Free Bar passed that run before the last Ring/Beat edit: rerun.
2. Re-audit brass-filament and free-bar (macro_audit + macro_audit.py, level-type: brass-filament/m1, free-bar/m4), re-render their tours (`MACRO_TOUR_BPM`: brass 60, free-bar 120, then `macro_tour_ab.py`), send TOURS with the new name Free Bar.
3. Doc: replace Hold by Space in the Brass Filament section, Tuning Fork by Free Bar (Ring/Beat numbers), flags and trim table; Tuning Fork loudness (tour RMS about -25 dB) still to check.
4. fmt, clippy (`cargo +1.99.0 clippy --workspace --all-targets -- -D warnings`, passed before the last edits), workspace tests, draft PR (gh api for the body), send DONE.
