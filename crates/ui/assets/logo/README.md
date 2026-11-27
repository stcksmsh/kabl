# kabl logo

The artwork is Kosta's. The letters and the "k" are his original paths, copied verbatim; the
cable frame and plugs are the geometric rebuild he approved on 2026-11-02 (see
`docs/design/redesign/g4/logo/` and `docs/design/verdicts.json`). All files draw in
`currentColor` with `color="#020202"` on the root, no background:

- `kabl-logo.svg`: the wide lockup (cable loop, two plugs, "kabl" wordmark), viewBox 272x100.
  Used in the toolbar and About, in the theme's text colour.
- `kabl-mark.svg`: the mark (cable loop and a single "k"), viewBox 100x100, for 32 px and up.
- `kabl-mark-small.svg`: the mark with simplified plugs, for 16-24 px.

`kabl-logo-mask.png` (948 px wide) is `kabl-logo.svg` rendered with the colour set to white on
a transparent background, so the UI can tint it from the theme (`kit::logo`):
`sed 's/color="#020202"/color="#FFFFFF"/' kabl-logo.svg > /tmp/w.svg` then
`inkscape /tmp/w.svg --export-type=png --export-width=948 --export-background-opacity=0 -o kabl-logo-mask.png`.

The orange app icon (tile `#F2A33A`, mark `#1C1206`) lives in `packaging/linux/logo/`; orange
is used only there and for the splash. The window icon is its 256 px PNG.

`crates/ui/assets/windows/kabl.ico` (frames 16, 24, 32, 48, 64, 128, 256; the 256 frame is PNG-compressed)
is built once from the orange app-icon PNGs above and committed; `kabl.rc` embeds it into
`kabl-ui.exe` at build time (`build.rs`, `embed-resource`). Regenerate from the repository root with:

    cd packaging/linux/logo/hicolor && convert 16x16/apps/kabl.png 24x24/apps/kabl.png 32x32/apps/kabl.png \
      48x48/apps/kabl.png 64x64/apps/kabl.png 128x128/apps/kabl.png 256x256/apps/kabl.png \
      ../../../../crates/ui/assets/windows/kabl.ico
