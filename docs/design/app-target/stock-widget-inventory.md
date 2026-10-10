# Stock egui widgets in use today, and what replaces them

Source: `grep` over `crates/ui/src` at origin/master `2195de0`. Counts are call sites, not screens. Rack faces, knobs, jacks and cables are already custom-painted; everything below is the rest of the application.

| Stock widget (call sites) | Where it shows up today | Replacement in all three directions | Notes |
|---|---|---|---|
| `ui.label` (111), `ui.heading` (10) | Every panel; no hierarchy | Type scale (`tokens-*.md`): display / title / h3 / body / label / value / caption, IBM Plex weights | One helper per step; no raw `RichText` size overrides |
| `ui.button` (56), `ui.small_button` (24) | Toolbar, browser (New, Close, Play, Stop), Routing, Perform (Shorter, All notes off, Open folder) | **Primary button**, **chip** (secondary), **icon button** | Primary reserved for one action per panel (Play C4, Save copy, Preview) |
| `egui::Button::selectable`, `ui.selectable_value`, `ui.selectable_label` (15) | Cables All/Focus/Hidden, A-light/A-dark, Browser scopes, Perform banks/direction | **Segmented control** (exclusive, same row) and **chip with selected state** | Selected state is a filled fill plus text colour, never colour alone |
| `ui.checkbox` (7) | Perform pin/learn rows, routing "Bypass", "Illustrated skins", composite and face pickers | **Toggle** (on/off settings) and **pin chip** | Perform's raw checkbox grid goes away entirely |
| `egui::ComboBox` (10) | Browser category and audition note, MIDI input, audio output, add-module kind, sequencer and cue clock/timing | **Dropdown field**: inset field, chevron, popover list with check mark (see components sheet) | Popover uses the `raised` surface and shadow `lg` |
| `ui.text_edit_singleline` / `egui::TextEdit` (7) | Sounds search, patch path, recordings name, labels | **Field** with leading icon, focus ring, placeholder | Patch path stays under "Patch folder (advanced)" |
| `egui::Slider` (2), `egui::DragValue` (2) | Audition velocity/length, panel width, CC values | **Slider** (track + accent fill + handle, numeric readout at right); DragValue becomes a mono readout that edits on click | Sliders need a visible value readout |
| `egui::Window` (3) | Save/unsaved prompts, panel authoring, dialogs | **Dialog**: centred card, one primary action, shadow `lg`, scrim | Drawn once; windows keep egui's area/ordering behaviour |
| `ui.menu_button`, `.context_menu` (11) | Add module, module menu, cable menu | **Menu** with shortcut column, destructive row in `bad` colour | |
| `.on_hover_text` (67) | Almost every control | **Tooltip** card: name and value, what moves it, gesture hint (see components sheet) | Tooltip text becomes data-driven from `ModuleInfo.explain` and the live routes |
| `ui.separator` (32), `ui.collapsing`/`CollapsingHeader` (5) | Panels | Hairline rules (A, B) or 1.5 px ink rules (C); section label + disclosure chevron | |
| `egui::ScrollArea` (8) | Browser, Routing, Perform | Same behaviour, thin overlay scrollbar (6 px, appears on hover/scroll) | Style only |
| `egui::TopBottomPanel` / `SidePanel` / `CentralPanel` | Toolbar, status bar, Sounds and Routing drawers | **Toolbar**, **status bar**, **drawer** (slides in `drawer` ms, one side panel open at a time at 1280 px) | The inspector collapses to a 44 px icon rail so the rack keeps about 100 % zoom |
| Status bar text (`callback worst … µs · 0 late · 0 xruns · RT off`) | Main status bar | Status bar shows meter, audio and MIDI state, voices. Telemetry moves to the **Diagnostics popover** | Defect 6 |
| Perform grid of bordered boxes, raw `CC 20 · 1` text, checkboxes | Perform | **Cards** (Transport, Scenes, Macros, Sequences, small controls), **pads**, **macro knobs** with CC badge, **step lanes** | Defect 3 |

Custom pieces that already exist and are restyled, not replaced: knob, selector, jack, plug, cable, face satin, envelope decor, piano keys, cue buttons, transport buttons.

New custom pieces needed: toolbar, drawer header, card, pad, step lane, display frame, diagnostics popover, toast, dialog, dropdown list, overview card (low-zoom module), cable cartridge.
