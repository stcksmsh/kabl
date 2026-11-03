# Colour audit, line_solid (Signal solid dark, Drafting solid light)

Mockup source: `docs/design/redesign/mockups/directions.html` (`APPS`, `faceStyle`, `ACC`, `FAM`).
App source: `crates/ui/src/style/solid.rs` (`roles`, `sections`), `theme.rs`, `style/mod.rs`.
Status: ok = equal to the mockup; fixed = changed in this audit; note = deliberate difference.

| Role | Token | App dark / light | Mockup dark / light | Status |
|---|---|---|---|---|
| chrome bg | roles.bg | #111214 / #ECEAE5 | same | ok |
| chrome bar (toolbar, drawer, status) | roles.bar | #17181B / #F7F6F2 | same | ok |
| panel / surface | roles.surface | #1A1B1E / #FBFAF7 | same | ok |
| raised | roles.raised | #232529 / #FFFFFF | same | ok |
| inset, well | roles.inset | #0F1012 / #E4E1DA | same | ok |
| outline | roles.line / line2 | #2A2C31, #393C42 / #DCD8D0, #C8C4BB | same | ok |
| text, muted, faint | text / text2 / text3 | #ECEAE6 #A29F99 #6F6D68 / #1A1917 #5F5C56 #8E8A83 | same | ok |
| accent (fills, arcs, outlines) | roles.accent | #F2A33A both | #F2A33A both | ok |
| accent text (small) | roles.accent_text | #F2A33A / #A94F00 | #F2A33A / #A94F00 (G4 spec) | ok |
| selection outline, focus ring | roles.focus | was #A94F00 on paper, now #F2A33A both | accent.c | fixed |
| display trace | roles.disp_trace | was #A94F00 on paper, now #F2A33A both | acc.c | fixed |
| hyperlink text | egui hyperlink_color | focus, now accent_text | text role | fixed |
| signal audio / cv / gate / pitch | roles.audio... | #F59E3B #38C6B2 #A68BFA #5BA8F7 / #D9781C #12958A #7651D6 #2F7FD6 | same | ok |
| good | roles.good | was satin #6cc788 / #2f8a4a, now #5CCB8A / #2E9E5E | meter and status dot | fixed |
| bad (clip) | roles.bad | was satin #b8322a, now #E5533C both | #E5533C | fixed |
| warn | roles.warn | was satin #b0661a, now accent (dark) / accent_text (paper) | none; the meter warn segment is the accent | fixed |
| rack bg, rail | rack / rail | #0C0D0E #1B1C1F / #DCD9D2 #C9C5BC | same | ok |
| rack mounting hole | roles.hole | was #050506 / #24232A, now #060607 / #A39E94 | `APPS.hole` | fixed |
| jack hole | Style::jack_hole | #050506 / #24232A | `faceStyle.hole` | ok (split from rack hole) |
| face fill | sections.*.base | rack mixed 2.2 % / 40 % white | rgba(255,255,255,.018 / .35) | note: opaque mix of the same |
| face title inks | sections.*.ink | family hue, mixed to 4.5:1 | FAM hues | note: contrast-corrected on paper |
| port tray plate | kit::parts::tray | face mixed 25 % toward raised | none (line faces draw hairlines) | fixed (was raised, white on paper) |
| cap, skirt, nut | roles.knob*, theme.skirt / nut | #1D1F23 #3C3F45, #E2DED6 #FFFFFF; skirt #141518 / #D6D2C9; nut #2B2D32 / #FFFFFF | `cap-hyb-d/l`, `faceStyle(hybrid)` | ok |
| section family hues | sections | #E07A5F #7FB59A #9D8DF1 #5FA8D3 #E3BE5C #B9B2A6 | FAM | ok |
| cable core edge | parts.rs cable edge | rgb(11,12,13) | dark casing in `cable()` | ok |
| route (modulation) cable colours | lib.rs `CABLE_COLORS` | 8 fixed hues | none in the mockup | note: per-source identity palette, not a role |
| warning text in explain / routing | `Color32::from_rgb(230,160,60)`, `(255,200,90)` | fixed | none | note: two warning strings, not themed yet |
| routing label chip | routing.rs (20,20,22,235), (90,155,255) | fixed | none | note: overlay chip, not themed yet |

Fixed colours: 9 (focus, disp_trace, hyperlink, good, bad, warn, rack hole, jack hole split, tray plate).
Contrast gates: `mark_pairs` now allow the mockup orange as a stroke on paper (1.5:1 on the inset well, 1.8:1 on the surface); dark keeps 3:1.

## Capture check (rack 1440x900, #87, #88 and #90 merged locally, not pushed)

Sampled with `PIL` at fixed points of the mockup and app captures. Bars and drawer match exactly; the rack background and face points did not land on the same element, so they are not compared.

| Spot | Light mockup / app | Dark mockup / app |
|---|---|---|
| toolbar bar | f7f6f2 / f7f6f2 | 17181b / 17181b |
| status bar | f7f6f2 / f7f6f2 | 17181b / 17181b |
| drawer bg | f7f6f2 / f7f6f2 | 17181b / 17181b |
| Save fill (edge antialiasing) | eb9a2f / e09736 | eb9a2f / cf8b31 |

Captures: `tokens-{rack,perform,sounds}-{light,dark}.png`, `module-2x-{light,dark}.png` (this directory).
The old selection outline (rust) is now the accent orange; cables show over the trays.
