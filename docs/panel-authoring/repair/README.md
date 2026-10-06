# D10 hands-on repair

Kosta requested this repair after using the first D10 bundle. Owner disposition is **changes requested** until Kosta reviews the repaired interactions and finish. Earlier engineering checks, screenshots and bundle remain historical evidence; they do not approve this repair.

Start with [REPORT.md](REPORT.md), [REVIEW.md](REVIEW.md) and [CHECKLIST.md](CHECKLIST.md). [design.md](design.md) explains the shared UI paths and preserved identity contracts. All images and videos here are real application output driven by scripted input on private X11 displays, not design mockups or owner/hardware evidence.

Compare [before interaction](media/before-interaction.mp4) with [repaired interaction](media/after-interaction.mp4). The repaired video shows authored and native moves across crowded neighbours, original knobs/selectors and jack routing, modal controls, nested internals/Back, visibility/order Apply, Escape cancellation, default-card drag, extra-row edge scrolling and themes/Sounds. The before take uses the retained old release binary; the repaired take uses the exact repair debug binary. Binary hashes and input actions are in evidence/before-interaction.json and after-interaction.json.

Viewport images use identical saved patch content at both logical sizes and themes. Open rack-1440x900-light.png, rack-1440x900-dark.png, rack-1280x800-light.png and rack-1280x800-dark.png in media/. More, internals, nesting, Edit face and Sounds have matching captures. Native scale 150% is a 1920×1200 raster for a 1280×800 logical window; its control exercise remains software evidence.

The separate `d10-repair-owner-test` bundle has a normal factory-enabled launcher and an explicit unavailable-library test launcher. It preserves the earlier bundle, all owner projects and user library contents. Normal launch respects KABL_USER_DIR, reuses prior bundle owner resources when present, and otherwise uses normal app library discovery. Factory sounds/resources are copied into the bundle. No source checkout is needed.

Reproduce with flow.py and capture.py after building the app and panel_seed example. make-bundle.py refuses to overwrite an existing bundle. host-fixture.py transfers actual GUI-saved native visibility/order into the private host fixture, preserving all nonpresentation state. check-bundle.py checks manifest hashes, normal factory startup, unavailable startup and relocated REAPER state/native replay. These scripts own only their stated private display/profile directories.

No new dependency, external asset or DSP change is included. Previous font/art/vendor licenses still apply. Hardware listening, physical latency/feel, owner UI approval, ordinary-render nondeterminism and dense-editor/backend limits remain open. Stop at repair submission; no merge or D11 launch is authorized.
