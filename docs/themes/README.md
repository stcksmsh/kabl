# Themes

kabl ships two built-in themes, **Satin Studio** (`kabl.satin`, the default) and **Lacquer and Light** (`kabl.lacquer`), each with a light and a dark mode. Switch in the settings menu (the gear at the bottom of the rail): Light or Dark, then the theme list. The change applies at once and is remembered for the standalone and for every plugin instance.

## Making your own theme

1. Copy `docs/themes/example/midnight-rose.toml` into the themes folder:
   `$KABL_CONFIG_DIR/themes/`, else `$XDG_CONFIG_HOME/kabl/themes/`, else `~/.config/kabl/themes/`.
2. Change `id` (unique, lowercase), `name`, and the values you want. Everything you leave out comes from the theme named in `extends` (`kabl.satin` or `kabl.lacquer`).
3. Restart kabl. Your theme appears in the settings menu. Anything kabl could not use is listed there (first four) and in the log, with the key name.

The file format, the limits and what a theme cannot change are in `docs/design/app-target/THEMING.md`, section 13. `[light]` and `[dark]` take the same keys as the full built-in theme: colour roles, section palettes, type scale, radii, shadows, motion and the recipe slots (knob, jack, cable, display, face and chrome recipes).

A theme file is treated as untrusted: it is capped at 64 KiB, every value is checked, and a bad value is replaced by the base theme's value with a message. A mode whose text would be hard to read (under 3:1 contrast) is ignored as a whole.

## Captures

`media/` has Satin Studio (left) and Lacquer and Light (right) side by side at 1440x900, light and dark: `simple-*` (reference patch), `dense-*` (Composition), `closeup-*` (FM voice with displays at 100%), `perform-*`, `pattern-*` (cable pattern editor). `effect-faces-<theme>-<mode>.webp` are contact sheets of the nine effect faces. `plugin-1180-*` are the CLAP editor in REAPER 7.75 (theme restored from `settings.toml`, then switched through the menu). `example-midnight-rose-*` is `example/midnight-rose.toml` loaded from the themes folder. The effect faces under `docs/effects-filters/faces/` are re-taken in Satin Studio, light and dark.

## Frame time

`KABL_BENCH_THEME=<id> cargo run --release -p kabl-ui --example bench_chrome 300 live` (dense Composition, 1440x900, rack only, displays animating, audio on; GPU not included). Switching a theme rebuilds one `Style` once; nothing is added per frame beyond a tab flag lookup and one contrast comparison per module palette.

| theme, mode | median | p95 | max |
|---|---|---|---|
| Satin Studio light | 4.6 to 5.3 ms | 4.8 to 5.7 | 5.1 to 7.0 |
| Satin Studio dark | 5.4 to 5.6 ms | 5.7 to 5.9 | 6.3 to 6.5 |
| Lacquer and Light light | 4.2 to 4.3 ms | 4.5 | 4.7 to 5.1 |
| Lacquer and Light dark | 4.5 ms | 4.8 to 5.1 | 5.2 to 8.9 |

Three runs each, machine under moderate load (load average 3 to 5); heavily loaded runs earlier showed 10 to 14 ms medians for both themes alike.

## Capture tooling

All scripts need `target/release/kabl-ui`, Xvfb and xdotool, and run from the repository root. `KABL_THEME=ID[:dark|light]` starts kabl in a theme without changing the remembered choice, so no menu clicking is involved.

- `scripts/capture.sh SCRIPT SIZE PATCH OUTDIR [DISPLAY]`: run a `docs/rack-migration/drive.py` script under every built-in theme and mode (`THEMES="id id"` picks others; `THEME_FILES="a.toml"` installs user themes first).
- `scripts/matrix.sh OUTDIR [SIZE] [DISPLAY]`: the four side-by-side scenes (simple rack, dense rack, face close-up, Perform).
- `scripts/capture_faces.sh OUTDIR [DISPLAY]` and `scripts/sheet.py`: the nine effect-module faces in every theme, and a contact sheet of them.
- `scripts/scenes.txt`, `scene-pattern.txt`: drawers, menus, dialogs and the cable pattern editor.
