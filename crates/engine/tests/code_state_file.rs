//! D12 PR 0: a code module's saved settings go through a `.kabl` bit-exact. The module's own
//! `state_save` bytes are kept in the file's embedded-code section and come back unchanged; a
//! module loaded from them saves the same bytes again.

use kabl_core::embedded::Embedded;
use kabl_core::format;
use kabl_core::PatchLog;
use kabl_modules::code::state::{save, Restored, SavedState};
use kabl_modules::code::{host, CodeGroup, InstanceConfig, WasmEngine, WasmiEngine};
use std::time::Duration;

#[test]
fn settings_survive_a_kabl_file_bit_exact() {
    let example = &host::EXAMPLES[1]; // demo.bitfilter, built by the SDK, has settings
    let engine = WasmiEngine::new();
    let compiled = engine.compile(example.wasm).unwrap();
    let cfg = InstanceConfig::new(48_000.0, 2048);
    let mut inst = compiled.instantiate(&cfg).unwrap();
    // Put non-default settings in the module through its own load path, then save them.
    let x = inst.state_xfer().unwrap();
    let default = save(&mut *inst).unwrap().unwrap();
    assert_eq!(
        default.bytes.len(),
        32,
        "the bitfilter's settings are 32 bytes"
    );
    let mut custom = default.bytes.clone();
    for (i, b) in custom.iter_mut().enumerate() {
        *b = b.wrapping_add(i as u8 * 7 + 1);
    }
    inst.memory()[x.addr..x.addr + custom.len()].copy_from_slice(&custom);
    let accepted = inst
        .call_state_load(custom.len() as u32, x.state_version)
        .unwrap();
    // The module may refuse arbitrary bytes; then it keeps its defaults and the test below
    // round-trips those, which is the same byte-exactness claim.
    let saved = save(&mut *inst).unwrap().unwrap();
    if accepted == 1 {
        assert_eq!(saved.bytes, custom);
    } else {
        assert_eq!(saved, default);
    }

    // into a file and back
    let mut code = Embedded::new();
    let hash = code
        .add_blob(
            &compiled.meta().kind,
            compiled.meta().version,
            example.wasm.to_vec(),
        )
        .unwrap();
    code.bind(10, hash).unwrap();
    code.keep_state(10, compiled.meta().version, saved.to_kept())
        .unwrap();
    let log = PatchLog::new();
    let mut file = Vec::new();
    format::write_kabl_full(&mut file, &log, None, Some(&code)).unwrap();
    let (_, _, back) = format::read_kabl_full(&file[..]).unwrap();
    let kept = back.kept_state(10, compiled.meta().version).unwrap();
    assert_eq!(
        kept,
        &saved.to_kept()[..],
        "the file returns the bytes unchanged"
    );
    let again = SavedState::from_kept(kept).unwrap();
    assert_eq!(again, saved);

    // a fresh module restores them and saves the same bytes
    let g = CodeGroup::new(compiled, cfg, 1_000_000, Duration::from_secs(1)).unwrap();
    let (_, r) = g.restore(&again).unwrap();
    assert_eq!(r, Restored::Loaded);
    let mut fresh = engine
        .compile(example.wasm)
        .unwrap()
        .instantiate(&cfg)
        .unwrap();
    let fx = fresh.state_xfer().unwrap();
    fresh.memory()[fx.addr..fx.addr + again.bytes.len()].copy_from_slice(&again.bytes);
    assert_eq!(
        fresh.call_state_load(again.bytes.len() as u32, again.state_version),
        Ok(1)
    );
    assert_eq!(save(&mut *fresh).unwrap().unwrap(), saved);
}
