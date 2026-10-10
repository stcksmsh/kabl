# kabl logo

The artwork is Kosta's. Both files are his single-path SVGs, unchanged (fill #020202, no
background):

- `kabl-logo.svg`: the wide lockup (cable loop, two plugs, "kabl" wordmark), viewBox 1897x632.
  Used in the toolbar.
- `kabl-mark.svg`: the small mark (cable loop and a single "k"), viewBox 1095x1095. For
  icon-sized uses inside the app; not used yet.

`kabl-logo-mask.png` (948 px wide) is `kabl-logo.svg` rendered with the fill set to white on a
transparent background, so the UI can tint it from the theme (`kit::logo`). It is a straight
render of his path; nothing is redrawn. Render the mark the same way when it is needed:
`inkscape mark-white.svg --export-type=png --export-width=384 --export-background-opacity=0`.
