# D02 owner checklist — Musical controls that explain themselves

For Kosta, hands-on, on the laptop. Leave the boxes unchecked until you have tried them. Launch
from the integrated branch (after D01-R1 is merged and D02 has been re-verified on top of it):

    cargo run --release -p kabl-ui -- --patch patches/palette/pad --perform --rate 48000 --frames 256
    # repeat once with --size 1280x800

| # | Action | Expected | Pass/fail, notes |
|---|---|---|---|
| [ ] 1 | Play a chord. Press **?** on the "Warmth" card. | The drawer shows **Warmth** with "What it changes (2)": `Ladder Filter #7 · Cutoff` and `Ladder Filter #7 · Drive`, each with depth, range and value at the stored setting. The card is outlined. The sound doesn't change. | |
| [ ] 2 | Turn Warmth (card slider or your mapped CC). | Brighter/warmer, as the explanation says. The "at its stored …" values follow. | |
| [ ] 3 | Press **Show** on Drive. | The ladder expands, Drive flashes and is marked "Shown", and its route row is visible in the drawer. Nothing is saved (● stays off). | |
| [ ] 4 | Press **← Back to "Warmth"**. | The rack view returns to where it was, and the Warmth card flashes. | |
| [ ] 5 | Press **?** on "Attack", then **Show**. | ADSR #8 Attack is inspected. Base, range, "Modulation: none" and the signal path are shown. | |
| [ ] 6 | On Warmth → Cutoff, tick **Bypass** in the drawer, then press Ctrl+Z. | The row is struck through at once ("bypassed, adds nothing now"), then comes back. | |
| [ ] 7 | Type in the Sounds search field and press Escape. Then press Escape with nothing focused. | The first Escape only leaves the field. The second closes the explanation. No preview, no transport change, no edit. | |
| [ ] 8 | Open another sound while an explanation is open. | The explanation closes. Nothing refers to the old sound. | |
| [ ] 9 | Open Sound Palette and explain **Motion**. | It lists the three ring modulators' `b` inputs and, under each, the cutoffs or fine tunings they then move. | |
| [ ] 10 | Beginner check (if you can arrange it): someone new to modular makes the pad brighter and slower, then explains which control did it, without help. | Your observation. Scripted runs can't answer this. | |
| [ ] 11 | Look at both themes and both sizes. | The text is readable, nothing is clipped that matters, and the rack keeps its character. | |

Owner review status: **pending**.
