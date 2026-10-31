# Macro defects: peaks and clicks (Groove)

Only the objective defects of `MACRO-AUDIT-GROOVE.md` (#66). The subjective flags (dead top thirds, switch-like Pumps, weak macros) are untouched: Kosta has not listened yet. The macro audit tools (`crates/ui/examples/macro_audit.rs`, `docs/presets/scripts/macro_audit.py`) were copied from the `claude/macro-audit-groove` branch of #66 (not merged: only those two files), so the audit can be rerun here.

## Peaks (fixed)

Every macro position must stay at or under -1 dBFS. Before, from the audit's 8 steps per macro:

| Macro | Before (worst step) | After |
|---|---|---|
| Loading Dock Sweep | +1.4 dBFS at 0.57 (over 0 dBFS from 0.57 to 0.86, -0.1 at 1) | -2.0 at 0.57 (worst), -6.3 at 1 |
| Loading Dock Pump | -0.6 dBFS at 0 | -1.5 at 0 |
| Rope Walk Pump | +0.1 dBFS at 0 | -1.8 at 0 |

Cause (Loading Dock): the first beat of a run, before the kick has ducked anything: with the filter open or with no duck the downbeat is a 9-sample peak (later beats are 12 dB lower). Rope Walk at Pump 0 has no duck at all, so the music alone peaks.

How, with the stored (default) macros unchanged:
- Loading Dock: the trim (`gain`) after the filter follows the macros: -9 dB per unit of Sweep and +2.2 dB per unit of Pump, from the stored Intro values (Sweep 0.12, Pump 0.5), so the Intro sounds as before. In the cues the Sweep is higher (Build 0.9, Drop 0.78), so Build and Drop are quieter than before by about 6 and 5 dB (the Pump trim gives some back); this is the price of headroom without a limiter. If that is too much, the other way is to shorten the first-beat transient (a duck that engages sooner), which changes the sound.
- Rope Walk: a new `T::PumpTrim(2.2)`: the music enters the final mix at a level that is 2.2 dB lower at Pump 0 and rises linearly to 1.0 at the stored value 0.6 (above it the level stays at 1.0).

## Travel check

`presets_groove macros` now also renders each macro at 8 positions (the others at their stored values) and asserts that the peak of every position is at most -1 dBFS (it prints the worst: macro, position, dBFS). All 13 presets of group 1 and 2 pass; the tightest travel peaks are Ore Cart Pump -1.2 (at 0), Scrap Metal Pump -1.2 (at 0), Fault Line Reese -1.5 (at 1), Corner Store Drive -1.6 (at 1) and Rope Walk -1.8. The Loading Dock lives in `presets_pilot`; its steps come from the macro audit (`macro_audit` + `macro_audit.py`).

## Default render before and after (the clip, float, same phrase)

| Preset | Loudness | Peak | Difference render (rms under the render) |
|---|---|---|---|
| Rope Walk | +0.01 dB | -6.2 to -6.3 dBFS | -8 dB (the bass pitch slew, below) moves the waveform, not the levels |
| Loading Dock (30 s clip with the cues) | -0.17 dB | -11.8 to -11.8 dBFS | -29 dB |

## Clicks

| Preset, macro | What the audit saw | Result |
|---|---|---|
| Rope Walk Pump | CLICK at 8.9 s in the macro-0 render | **Fixed.** The bass (`seq` -> `osc`) jumps pitch (octaves) while the previous note's tail rings and the music is not ducked. A 1.5 ms slew on the bass pitch (`Part::smooth`) removes the click in the 8 steps and in the sweep. |
| Fault Line Filter | CLICK at 5.85 s in the sweep only | **Not fixed.** |
| Corner Store Filter | CLICK at 3.75 s and 7.64 s at macro 0.57 and up (also in the static renders), 5.69 s | **Not fixed.** |

Fault Line and Corner Store are not mid-move clicks: Corner Store's is in the static renders from cutoff 0.57 up, Fault Line's sits on a bass note start. In the samples the spike is the reset edge of the bass saw (0.25 to -0.25 within 3 samples) at the start of a note, and it is only a "click" for the scanner when the filter is open enough to pass it and the 50 ms scan window holds one such edge. Muting the bass (`mixer.level1`) removes it; muting the drums, the chords or the played voice does not; a 1.5 ms pitch slew, a longer attack or a different unison do not (tried). What does hide it is a release of 120 ms (a different sound) or a darker filter. I left both presets alone: the fix is a sound decision (close the filter range, or a softer bass), the same as the dead top thirds in the audit.
