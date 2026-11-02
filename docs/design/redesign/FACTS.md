# Facts checked before G0 (2026-10-31, against origin/master 6e92737)

The brief and plan mark four items as unverified. Findings:

1. **`bench_chrome` exists.** `crates/ui/examples/bench_chrome.rs` has been in the tree since #35. Usage per `docs/themes/README.md`: `KABL_BENCH_THEME=<id> cargo run --release -p kabl-ui --example bench_chrome 300 live` (dense Composition, 1440x900, rack only). Other benches: `bench_composition`, `bench_palette`, `bench_palette_parts`, `bench_performance`, `bench_reference`, `bench_echo`. PLAN §9.8's "A creates it if missing" is not needed; A may need to add a 1180x680 mode.
2. **Theme file format** (`crates/ui/src/style/load.rs`): TOML, at most 64 KiB, at most 64 user files, `ui_api = 1`. A file is an overlay on a built-in named by `extends` (`kabl.satin` or `kabl.lacquer`, default satin). Unknown, wrong-type or out-of-range values are dropped with a note. A mode whose text contrast is below 3.0 is dropped whole; below 4.5 is only reported. Tables: `[light|dark.roles]`, `[light|dark.sections.<family>]` (base, ink, ink2), `[light|dark.slots.<part>]` (recipe choices, for example knob `body`, `scale`). Example: `docs/themes/example/midnight-rose.toml`. The 3-layer model (app theme × face style × accent) needs a new vocabulary version or new tables; `ui_api` 1 files must keep loading.
3. **PR #55** (visual polish stage 2a, `claude/visual-polish-2`, e71ce2c) is open as a draft, last updated 2026-10-28, parked by the owner. The redesign supersedes it; do not build on it. Close or leave it as the owner decides.
4. **seq-32** (`claude/seq-32`, dd33766, pushed, no PR yet): 32 steps per pattern, length 1 to 32, face shows pages of 8; seq param cap 655; per-step params are never automation-lane targets. Agent 2 is finishing it. G3 timing/drums (agent D) must build on it after it merges.

Also present on master: an older design set in `docs/design/` (BRIEF.md, app-target/, directions/, revision-2/, REVIEW.md, QA.md). `docs/design/redesign/` is the current one.

## Colour tokens: mockup vs app (2026-11-02)

Mockup source: `APPS`, `ACC` in `mockups/directions.html`. App: `crates/ui/src/style/solid.rs` `roles()`, default "Signal solid" (dark) and "Drafting solid" (light). Before = origin/master, After = this PR.

| Token | Graphite mockup | App before | App after | Paper mockup | App before | App after |
|---|---|---|---|---|---|---|
| bg | #111214 | same | same | #ECEAE5 | same | same |
| bar (toolbar, status, side panels) | #17181B | no token (used surface #1A1B1E) | `bar` #17181B | #F7F6F2 | no token (surface #FBFAF7) | `bar` #F7F6F2 |
| surf (cards) | #1A1B1E | same, but cards drew `raised` | same, cards draw surf | #FBFAF7 | same, cards drew white | same, cards draw surf |
| raised | #232529 | same | same | #FFFFFF | same | same |
| inset | #0F1012 | same | same | #E4E1DA | same | same |
| line / line2 | #2A2C31 / #393C42 | same | same | #DCD8D0 / #C8C4BB | same | same |
| tx / tx2 / tx3 | #ECEAE6 / #A29F99 / #6F6D68 | same | same | #1A1917 / #5F5C56 / #8E8A83 | same | same |
| rack | #0C0D0E | same | same | #DCD9D2 | same | same |
| rail | #1B1C1F | same | same | #C9C5BC | same | same |
| jack hole (face) | #050506 | same | same | #24232A | same | same (the mockup's #A39E94 is the rack punch colour) |
| accent (amber) | #F2A33A | same | same | #F2A33A | #A97229 (darkened for 3:1) | #F2A33A |
| accent ink | #1C1206 | same | same | #1C1206 | white or #1C1206 by contrast | #1C1206 |
| sig audio / cv / gate / pitch | #F59E3B #38C6B2 #A68BFA #5BA8F7 | same | same | #D9781C #12958A #7651D6 #2F7FD6 | #B66518 #11897F #7651D6 #2E7CD2 (darkened for 3:1 on the rack) | mockup values (supersede the 2026-11-01 darkened hues) |

Not a mockup token, added so paper keeps legible small text: `accent_text`, equal to the accent on graphite, the amber mixed toward black until 4.5:1 on surf on paper (#A94F00 as in the G4 spec: 5.3:1 on surf, 4.6:1 on bg). Uses: `Tone::Accent` text, the "Queued" word on a scene pad, the focus ring, the display trace on paper. Everything else keeps the orange: buttons, dots, value arcs, active chips, playing pads, selected-row marks.

Contrast checker changes: new text pair "accent_text on surface" (4.0, the satin theme's own accent text is 4.35). The marks "accent on surface" (3:1) is gone, because the mockup's orange is about 2:1 on paper by design and carries its own ink; the paper signal marks on the rack are checked at 2:1 (they reach 2.2 to 3.9), dark keeps 3:1.
