// The clap-first entry source for clap-wrapper. kabl's clap_entry is defined in Rust
// (crates/clap/src/state_bridge.rs) and arrives in the kabl-clap static library.
// clap-wrapper's VST3 entry finds it with GetProcAddress on its own module, so the
// symbol must be in the export table of kabl.vst3 (and of the CLAP built alongside).
// The export directive also makes the linker pull it out of the static library.
#pragma comment(linker, "/EXPORT:clap_entry,DATA")
