# kabl — website branch

The source of kabl's website, <https://stcksmsh.github.io/kabl/>. This is an orphan branch:
it shares no history with `master` and holds only the site.

- Plain Vite, no framework, one dev dependency. `npm install && npm run dev` to work on it,
  `npm run build` for `dist/`.
- `.github/workflows/deploy-website.yml` builds and deploys it to this repository's GitHub
  Pages on every push to `website` (Settings → Pages → Source: "GitHub Actions").
- Base path: `/kabl/` by default (`vite.config.js`). For a custom domain, set the repository
  variable `BASE_PATH` to `/`, add `public/CNAME`, and update `url` in `publication.json`.
- `publication.json` is the only file stcksmsh.github.io reads from this branch (its project
  index links here). `visibility: "unlisted"` hides kabl from that index without taking the
  site down.
- Content comes from `master`'s README; images in `public/media/` are copies from `master`'s
  `docs/design/` and `packaging/linux/logo/` (licences in `master`'s `ASSET-LICENSES.md`).
  Update them here when they change there.
