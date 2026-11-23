# Windows standalone + VST3: feasibility (investigation D)

Date 2026-11-23. Base origin/master 89d9c1be (1.0.0). Method: code reading, Linux cross-compile, Wine loader probes.
A cross-compile or a Wine load is NOT Windows support; nothing has run in a real Windows host.

## Recommendation
Route (b): clap-wrapper (free-audio, MIT) around the existing CLAP, ending in a clap-first single-file `.vst3`; also ship
a Windows `kabl.clap` (REAPER loads it natively). Why: kabl's host behaviour lives at raw CLAP level in
`crates/clap/src/state_bridge.rs` (transactional state, recall epochs, MIDI-only dialect, process/flush); nice-plug's own
VST3 export (route a) uses the generic JSON state loader kabl deliberately bypasses (`wrapper/vst3/wrapper.rs:488,507`)
and its RT safety is unaudited. Fall back to (a) only if Live acceptance fails in a way upstream will not fix.
Host params already VST3-safe: fixed list of 18 (`output_gain`, Slot 1..16, `host_clock`; `lib.rs:52-66,876-889`,
`state_bridge.rs:138`), stable 31-bit ids via `hash_param_id` (`vendor/nice-plug/.../wrapper/util.rs:35-46`); patch
edits rebind slots, never add/remove params. MIDI input `MidiConfig::MidiCCs` (`lib.rs:1191`).
Licensing fine: kabl GPL-3.0-or-later; VST3 SDK now MIT; clap-wrapper MIT; `vst3` crate MIT/Apache; nice-plug ISC.
VST logo optional (Steinberg trademark rules if used). C++ deps via CMake are outside cargo-deny (`deny.toml:9-20`):
list them by hand in THIRD-PARTY-NOTICES.md. nice-plug keeps its VST3 wrapper (feature on by default) but kabl disables
it (`crates/clap/Cargo.toml`: default-features=false, editor+strict-clap); strict profile covers only wrapper/clap.
Ableton Live has no CLAP support (mid-2026), so VST3 is required for Live.

## Platform assumptions (crates/ unless noted)
| Area | Where | Windows impact |
|---|---|---|
| User library | ui/src/library.rs:286-294 (KABL_USER_DIR, XDG_DATA_HOME, HOME/.local/share, ./) | no HOME -> current dir (confirmed under Wine). Want %APPDATA%\kabl |
| Factory sounds | ui/src/library.rs:250-283 | prefer <exe dir>\patches |
| Config/themes | ui/src/style/load.rs:600-608 | same fallback; %APPDATA%\kabl |
| Logs | standalone/src/applog.rs:136-153 | %LOCALAPPDATA%\kabl\logs |
| File browser | ui/src/filebrowser.rs:920-922 (HOME only), :781, :996 ("/" root) | no Windows home, no drive letters |
| Desktop hints | ui/src/desktop.rs:92-110 (D-Bus, Linux only) | compiles; Windows theme = registry, low prio |
| RT audio thread | ui/src/app/callback.rs:64,81-99; ui/src/app/audio.rs:314 | ONLY compile error: Windows RT priority handle not Send inside cpal callback. Send newtype or keep handle on audio thread |
| Calibration thread | modules/src/code/host.rs:260-269 (SCHED_IDLE Linux only) | optional on Windows |
| Code modules | wasmi interpreter, CodeScope per instance | portable |
| Atomic save | core/src/format.rs:395-410 | rename fails if file open elsewhere: retry or ReplaceFileW |
| Open folder | ui/src/record.rs:418-431 | already uses explorer |
| Standalone window | eframe glow (ui/Cargo.toml:13-18) | builds; VMs may need wgpu fallback |
| Plugin window | vendor/egui-baseview (OpenGL), baseview f106a51 | builds; keyboard focus in hosts is a risk (keyboard_focus_workaround exists) |
| Audio/MIDI | cpal 0.18 default_host, midir 0.11 | WASAPI/WinMM unchanged; no ASIO; KABL_MIDI_PIPE test hook Linux only (ui/src/app/midi.rs:67-75) |
| Packaging | packaging/linux/package.sh, install.sh | Linux only |
| CI | ci.yml, release.yml, d05-graphical-recovery.yml | all ubuntu-24.04 |
| Dev tool | cargo-kabl/src/build.rs:66-68 ($HOME/.cargo) | needs USERPROFILE |

## Proof (throwaway scratch, deleted)
- Whole workspace compiles and links for x86_64-pc-windows-gnu (target already installed; llvm-mingw in scratch; stub
  libgcc/libgcc_eh; linker wrapper). cargo-xwin (MSVC) not tried: Microsoft CRT/SDK licence is Kosta's call.
- Errors: (1) zstd-sys needs a C cross compiler (toolchain only); (2) E0277 RT handle not Send (real bug, fixed in
  experiment, not committed); (3) lld -lgcc/-lgcc_eh (llvm-mingw quirk). Nothing else failed.
- Artefacts: kabl_clap.dll 30 MB (exports clap_entry only), kabl-ui.exe 16 MB, kabl.exe 4.7 MB; only system + UCRT
  imports. kabl.clap also imports mmdevapi (cpal pulled in via standalone/ui): harmless, worth trimming.
- Wine: kabl.exe self-test rendered 96000 samples, peak 0.427, RMS 0.075, no NaN. kabl.clap: clap_entry 1.2.2, init ok,
  factory lists dev.stcksmsh.kabl 1.0.0. clap-wrapper thin kabl.vst3 (1.2 MB; MinGW needed `#include <exception>` in
  threadchecker_win32.cpp) exports GetPluginFactory/InitDll/ExitDll; with CLAP_PATH set exposes one class "kabl",
  CID bda9c239b42389589c969403a7b9d23e; without it factory NULL (needs kabl.clap in %COMMONPROGRAMFILES%\CLAP or
  %LOCALAPPDATA%\Programs\Common\CLAP): reason for clap-first build. MinGW .vst3 imports libc++.dll/libunwind.dll.
- Not done: no plugin instance, editor, or host audio.

## Must test on real Windows (Live primary, REAPER secondary)
1 discovery/scan; 2 playback (64-2048 buffers, 44.1/48/96 k, offline vs RT; standalone WASAPI shared/exclusive, device
unplug, RT promotion); 3 MIDI via Live's VST3 path (notes, velocity, sustain, bend, mod wheel, CCs); 4 automation of
Slots 1-16/gain/clock surviving patch edit, slot reassignment, save+reopen; 5 state recall (reload, duplicate tracks,
multi-instance, undo, Linux CLAP state into Windows VST3); 6 editor open/close x20, resize, DPI 100/150/200 %, mixed
DPI, keyboard focus vs Live shortcuts, Intel/AMD/NVIDIA OpenGL; 7 eight+ instances; 8 clean-VM install/upgrade/
uninstall, SmartScreen; 9 AppData paths, spaces and non-ASCII user names.

## Slices (Linux CI unchanged; Windows jobs only added)
| # | Slice | Size | Depends | Acceptance |
|---|---|---|---|---|
| W1 | RT handle Send fix + windows-latest MSVC CI job (build ui/clap/standalone, cargo test, Linux-only tests skipped) | S | - | Windows job green, Linux unchanged |
| W2 | Platform paths in one place (dirs 6 or ~20 lines APPDATA/LOCALAPPDATA) for library.rs:250,286, style/load.rs:600, applog.rs:137 | S | W1 | per-OS unit tests; Linux paths unchanged |
| W3 | File browser: profile home, drive roots, breadcrumbs | S | W2 | browse C:\ D:\, save/open .kabl in Documents |
| W4 | Atomic save retry/ReplaceFileW | S | W1 | Windows test: save while file open |
| W5 | Windows CLAP in CI + clap-validator | S | W1 | validator green; REAPER loads it |
| W6 | VST3 via clap-wrapper (packaging/windows/vst3/CMakeLists.txt, pinned clap-wrapper + VST3 SDK, kabl-clap static lib, clap-first kabl.vst3, Steinberg validator in CI) | M | W5 | validator passes; class ID recorded and frozen |
| W7 | VST3 param/MIDI fit (flags, groups, CC mapping) | S-M | W6 | slot automation survives edit+reload in Live and REAPER |
| W8 | Editor: per-monitor DPI manifest, plugin scaling, resize, Live keyboard focus, reopen leak check | M | W6 | test item 6 at 100/150/200 % |
| W9 | Inno Setup installer (Program Files, VST3 + CLAP common folders, Start menu, uninstaller keeps user data) | M | W5,W6 | clean VM: install, Live finds kabl, uninstall leaves user data |
| W10 | Release job: installer + zip + SHA256SUMS, optional signing | S | W9 | tag builds Linux tarball and Windows installer |
| W11 | Optional: Windows theme, idle priority, ASIO | S each | W1 | on request |
Order: W1; W2-W4 alongside W5; W6; W7+W8; W9+W10.
Risks: Live VST3 quirks (CC mapping, param thread, focus); OpenGL on VMs/old iGPUs; CMake+MSVC adds ~5-10 min CI;
RT audit redo for WASAPI/thread priority; SmartScreen while unsigned; cpal inside the plugin binary.

## Kosta decisions
1 Test machine: physical for acceptance + VM snapshot for install tests (remote access, or Kosta runs the list).
2 Ableton Live: trial first, licence before release gate (Lite's track limit cuts multi-instance test). REAPER eval free.
3 Signing: unsigned beta -> Azure Trusted Signing (~10 USD/month) if eligible, else OV cert (~200-400 USD/yr). EV not worth it.
4 Installer: Inno Setup + zip.
5 Freeze VST3 class ID (bda9c239..., from dev.stcksmsh.kabl); VST logo yes/no; ship Windows CLAP (yes); ASIO in v1 (no,
WASAPI only); toolchain: native MSVC on windows-latest (cargo-xwin means accepting Microsoft's CRT/SDK licence).
