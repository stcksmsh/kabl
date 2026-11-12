//! `cargo kabl validate`: everything the host would refuse, plus the things it silently
//! tolerates that a creator wants to hear about. Loading goes through the real `WasmiEngine`, so
//! the answer is the host's answer.

use crate::wasm;
use kabl_modules::code::{
    CodeMeta, InstanceConfig, WasmEngine, WasmInstance, WasmiEngine, DEFAULT_MEMORY_CAP,
    MAX_BLOB_BYTES, META_SECTION,
};
use kabl_modules::PortDirection;

/// Id prefixes only kabl itself may use (decision B).
pub const RESERVED_PREFIXES: [&str; 3] = ["kabl.", "demo.", "test."];

/// Block size the host runs modules at (`host::MAX_BLOCK`); the layout must hold it.
const HOST_BLOCK: u32 = 2048;
/// Frames per smoke-render block, and blocks per stimulus.
const FRAMES: usize = 512;
const BLOCKS: usize = 4;
/// Fuel for one smoke block (the host budgets far less; this only guards against a hang).
const FUEL: u64 = 500_000_000;

#[derive(Default)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    /// Accept ids under the reserved prefixes (kabl's own examples and test fixtures).
    pub allow_reserved: bool,
}

/// `githubhandle.module`: one dot, both parts non-empty lowercase letters, digits, `_` or `-`.
pub fn check_id(id: &str, allow_reserved: bool) -> Result<(), String> {
    let part = |p: &str| {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    };
    match id.split_once('.') {
        Some((a, b)) if part(a) && part(b) => {}
        _ => {
            return Err(format!(
                "id `{id}` must be `githubhandle.module` (lowercase letters, digits, `_`, `-`)"
            ))
        }
    }
    if !allow_reserved {
        if let Some(p) = RESERVED_PREFIXES.iter().find(|p| id.starts_with(**p)) {
            return Err(format!("id `{id}` uses the reserved prefix `{p}`"));
        }
    }
    Ok(())
}

pub fn validate(wasm_bytes: &[u8], opts: Options) -> Report {
    let mut r = Report::default();
    r.notes.push(format!("{} bytes", wasm_bytes.len()));
    if wasm_bytes.len() > MAX_BLOB_BYTES {
        r.errors.push(format!(
            "blob is {} bytes; the cap is {MAX_BLOB_BYTES}",
            wasm_bytes.len()
        ));
        return r;
    }
    // Section-level checks first: they hold even when the module does not load.
    match wasm::custom(wasm_bytes, wasm::NOTICES) {
        Ok(Some(n)) if n == wasm::NOTICE_TEXT.as_bytes() => {}
        Ok(Some(_)) => r.warnings.push(
            "`kabl.notices` is not the text this cargo-kabl writes; rebuild with `cargo kabl build`"
                .into(),
        ),
        Ok(None) => r.warnings.push(
            "no `kabl.notices` section: build with `cargo kabl build`, which adds the licence \
             notices for the Rust toolchain code in your module"
                .into(),
        ),
        Err(e) => {
            r.errors.push(e);
            return r;
        }
    }
    if let Ok(Some(text)) = wasm::custom(wasm_bytes, META_SECTION) {
        if let Ok(text) = std::str::from_utf8(text) {
            for key in kabl_modules::code::wasmi_engine::unknown_keys(text) {
                r.warnings.push(format!(
                    "meta: unknown key `{key}` (the host ignores it; a typo?)"
                ));
            }
        }
    }

    // The host's own loader: imports, required exports, meta parse and sanity, ABI.
    let engine = WasmiEngine::new();
    let compiled = match engine.compile(wasm_bytes) {
        Ok(c) => c,
        Err(e) => {
            r.errors.push(e.to_string());
            return r;
        }
    };
    let meta = compiled.meta().clone();
    if let Err(e) = check_id(&meta.kind, opts.allow_reserved) {
        r.errors.push(e);
    }
    r.notes.push(format!(
        "module `{}` v{}: {} in, {} out slots, {} params",
        meta.kind,
        meta.version,
        meta.slots(PortDirection::Input),
        meta.slots(PortDirection::Output),
        meta.params.len()
    ));

    let cfg = InstanceConfig::new(48_000.0, HOST_BLOCK);
    let mut inst = match compiled.instantiate(&cfg) {
        Ok(i) => i,
        Err(e) => {
            r.errors.push(e.to_string());
            return r;
        }
    };
    let layout = inst.layout();
    let mem_len = inst.memory().len();
    r.notes
        .push(format!("memory {mem_len} bytes (cap {DEFAULT_MEMORY_CAP})"));
    if let Err(e) = layout.validate(
        mem_len,
        meta.slots(PortDirection::Input),
        meta.slots(PortDirection::Output),
        meta.params.len(),
        HOST_BLOCK as usize,
    ) {
        r.errors.push(format!("layout: {e}"));
        return r;
    }
    smoke(&meta, inst.as_mut(), &mut r);
    r
}

/// Input patterns, one value per frame.
type Stimulus = (&'static str, fn(usize) -> f32);
/// Tier 1: what a module can really receive (audio, CV, Hz-valued inputs are at most ~1e4).
/// Non-finite output here is an error: the host disables the module.
const REALISTIC: [Stimulus; 6] = [
    ("silence", |_| 0.0),
    ("impulse", |i| if i == 0 { 1.0 } else { 0.0 }),
    ("full-scale square", |i| {
        if i.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        }
    }),
    (
        "+-1e4",
        |i| if i.is_multiple_of(2) { 1.0e4 } else { -1.0e4 },
    ),
    ("f32::MIN_POSITIVE", |_| f32::MIN_POSITIVE),
    ("denormals", |i| {
        if i.is_multiple_of(2) {
            1.0e-40
        } else {
            -1.0e-40
        }
    }),
];
/// Tier 2: absurd inputs, smallest first. Non-finite output is only a warning, naming the
/// smallest magnitude that did it (`x * x` is infinite from about 2e19).
const EXTREME: [f32; 6] = [1.0e6, 1.0e9, 1.0e12, 1.0e15, 1.0e20, 1.0e30];

/// Output beyond this on an input of +-1 or less is allowed but probably a blow-up.
const BIG: f32 = 1.0e6;

struct Pass {
    fuel: u64,
    peak: f32,
    non_finite: bool,
}

/// Renders `BLOCKS` blocks of `f` with `params` after a reset; `Err` is a fault or a bad reset.
fn render(
    inst: &mut dyn WasmInstance,
    f: &dyn Fn(usize) -> f32,
    params: &[f32],
    frames: usize,
) -> Result<Pass, String> {
    let layout = inst.layout();
    // Reset runs under the caller's fuel; give it a full budget, not the previous block's leftover.
    inst.set_fuel(FUEL);
    inst.reset().map_err(|f| format!("reset {}", f.label()))?;
    let mut pass = Pass {
        fuel: 0,
        peak: 0.0,
        non_finite: false,
    };
    for block in 0..BLOCKS {
        let mem = inst.memory();
        for slot in 0..layout.in_slots as usize {
            for (i, c) in mem[layout.input_range(slot)]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .take(frames)
                .enumerate()
            {
                *c = f(block * frames + i).to_le_bytes();
            }
        }
        for (i, v) in params.iter().enumerate() {
            mem[layout.param_range(i)].copy_from_slice(&v.to_le_bytes());
        }
        inst.set_fuel(FUEL);
        pass.fuel = pass.fuel.max(
            inst.call_block(frames as u32)
                .map_err(|f| f.label().to_owned())?,
        );
        let mem = inst.memory();
        for slot in 0..layout.out_slots as usize {
            for c in mem[layout.output_range(slot)]
                .as_chunks::<4>()
                .0
                .iter()
                .take(frames)
            {
                let v = f32::from_le_bytes(*c);
                pass.non_finite |= !v.is_finite();
                if v.is_finite() {
                    pass.peak = pass.peak.max(v.abs());
                }
            }
        }
    }
    Ok(pass)
}

fn smoke(meta: &CodeMeta, inst: &mut dyn WasmInstance, r: &mut Report) {
    let frames = FRAMES.min(inst.layout().max_block as usize);
    let sets: [(&str, Vec<f32>); 3] = [
        ("default", meta.params.iter().map(|p| p.default).collect()),
        ("minimum", meta.params.iter().map(|p| p.min).collect()),
        ("maximum", meta.params.iter().map(|p| p.max).collect()),
    ];
    let mut max_fuel = 0u64;
    let mut blew_up = false;
    for (sname, f) in REALISTIC {
        for (pname, params) in &sets {
            match render(inst, &f, params, frames) {
                Err(e) => {
                    r.errors.push(format!(
                        "smoke render: module {e} (input: {sname}, params: {pname})"
                    ));
                    return;
                }
                Ok(p) if p.non_finite => {
                    r.errors.push(format!(
                        "smoke render: output is not finite (input: {sname}, params: {pname}); \
                         the host disables a module that does this"
                    ));
                    return;
                }
                Ok(p) => {
                    max_fuel = max_fuel.max(p.fuel);
                    if !blew_up && p.peak > BIG && sname != "+-1e4" {
                        blew_up = true;
                        r.warnings.push(format!(
                            "output reaches {} on an input of +-1 or less (input: {sname}, \
                             params: {pname}); the output exceeds +-{BIG}, a blow-up?",
                            p.peak
                        ));
                    }
                }
            }
        }
    }
    // Tier 2: the smallest absurd input that makes the output non-finite.
    let mut failed = None;
    'ladder: for mag in EXTREME {
        let f = move |i: usize| if i.is_multiple_of(2) { mag } else { -mag };
        for (_, params) in &sets {
            match render(inst, &f, params, frames) {
                Ok(p) if !p.non_finite => max_fuel = max_fuel.max(p.fuel),
                Ok(_) | Err(_) => {
                    failed = Some(mag);
                    break 'ladder;
                }
            }
        }
    }
    if let Some(mag) = failed {
        r.warnings.push(format!(
            "kabl stops this module if an input reaches about +-{mag:e} (its output goes \
             non-finite or it faults there); guard huge inputs if that matters"
        ));
    }
    r.notes.push(format!(
        "smoke render ok: {} realistic stimuli x {} param sets, {frames}-frame blocks, at most {max_fuel} fuel per block",
        REALISTIC.len(),
        sets.len()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str =
        "kind {id}\\nversion 1\\nname T\\ncategory utility\\nrate voice\\nwidth 4\\n\
port in audio in 1\\nport out audio out 1\\nparam g 0 4 1 linear 5 x\\n{extra}";

    /// One in, one out, one param; `process` is the body given. Layout as `Layout::validate`
    /// wants it (2048-frame slots), `params` at the given address.
    fn blob(id: &str, extra: &str, imports: &str, params: u32, process: &str) -> Vec<u8> {
        let meta = META.replace("{id}", id).replace("{extra}", extra);
        wat::parse_str(format!(
            r#"(module {imports}
 (@custom "kabl.meta" "{meta}")
 (memory (export "memory") 1)
 (func (export "init") (param f32 i32) (result i32)
  (i32.store (i32.const 0) (i32.const 64)) (i32.store (i32.const 4) (i32.const 8256))
  (i32.store (i32.const 8) (i32.const {params})) (i32.store (i32.const 12) (i32.const 16452))
  (i32.store (i32.const 16) (i32.const 0)) (i32.store (i32.const 20) (i32.const 2048))
  (i32.store (i32.const 24) (i32.const 1)) (i32.store (i32.const 28) (i32.const 1))
  (i32.store (i32.const 32) (i32.const 1))
  (i32.const 0))
 (func (export "reset"))
 (func (export "process") (param $n i32) {process}))"#
        ))
        .unwrap()
    }

    const SILENT: &str = "(memory.fill (i32.const 8256) (i32.const 0) (i32.const 8192))";
    const PASS: &str = "(memory.copy (i32.const 8256) (i32.const 64) (i32.const 8192))";

    fn good() -> Vec<u8> {
        wasm::finish(
            &blob("alice.t", "", "", 16448, SILENT),
            wasm::NOTICE_TEXT.as_bytes(),
        )
        .unwrap()
    }

    fn text(v: &[String]) -> String {
        v.join(" | ")
    }

    #[test]
    fn a_good_module_passes_cleanly() {
        let r = validate(&good(), Options::default());
        assert!(
            r.ok() && r.warnings.is_empty(),
            "{} {}",
            text(&r.errors),
            text(&r.warnings)
        );
        assert!(r.notes.iter().any(|n| n.contains("smoke render ok")));
    }

    #[test]
    fn a_missing_notice_warns() {
        let r = validate(&blob("alice.t", "", "", 16448, SILENT), Options::default());
        assert!(r.ok(), "{}", text(&r.errors));
        assert!(text(&r.warnings).contains("no `kabl.notices`"));
    }

    #[test]
    fn an_unknown_key_warns() {
        // The key list is the host's own (`wasmi_engine::unknown_keys`); the host still loads the blob.
        let raw = blob("alice.t", "abi_minro 1\\n", "", 16448, SILENT);
        let r = validate(&raw, Options::default());
        let w = text(&r.warnings);
        assert!(w.contains("unknown key `abi_minro`"), "{w}");
        assert!(r.ok(), "{}", text(&r.errors));
    }

    #[test]
    fn reserved_ids_are_errors_unless_allowed() {
        let b = wasm::finish(&blob("demo.t", "", "", 16448, SILENT), b"x").unwrap();
        assert!(text(&validate(&b, Options::default()).errors).contains("reserved"));
        assert!(validate(
            &b,
            Options {
                allow_reserved: true
            }
        )
        .ok());
    }

    #[test]
    fn imports_are_errors() {
        let b = blob("alice.t", "", r#"(import "env" "f" (func))"#, 16448, SILENT);
        assert!(text(&validate(&b, Options::default()).errors).contains("imports"));
    }

    #[test]
    fn a_bad_layout_is_an_error() {
        let b = blob("alice.t", "", "", 16450, SILENT);
        assert!(text(&validate(&b, Options::default()).errors).contains("layout"));
    }

    #[test]
    fn non_finite_output_fails_the_smoke_render() {
        let inf = "(f32.store (i32.const 8256) (f32.const inf))";
        let e = text(&validate(&blob("alice.t", "", "", 16448, inf), Options::default()).errors);
        assert!(e.contains("not finite"), "{e}");
    }

    #[test]
    fn a_huge_output_warns() {
        let r = validate(&blob("alice.t", "", "", 16448, PASS), Options::default());
        assert!(r.ok(), "{}", text(&r.errors));
        // PASS copies its input, so only the huge inputs give huge output: no warning.
        assert!(!text(&r.warnings).contains("exceeds"));
        let boom = "(f32.store (i32.const 8256) (f32.const 1e9))";
        let r = validate(&blob("alice.t", "", "", 16448, boom), Options::default());
        assert!(text(&r.warnings).contains("exceeds"));
    }

    #[test]
    fn a_square_overflows_only_on_absurd_inputs_so_it_warns() {
        let sq = "(f32.store (i32.const 8256) (f32.mul (f32.load (i32.const 64)) (f32.load (i32.const 64))))";
        let r = validate(&blob("alice.t", "", "", 16448, sq), Options::default());
        assert!(r.ok(), "{}", text(&r.errors));
        let w = text(&r.warnings);
        assert!(w.contains("input reaches about +-1e20"), "{w}");
    }

    #[test]
    fn a_trap_is_an_error() {
        let b = blob("alice.t", "", "", 16448, "(unreachable)");
        assert!(!validate(&b, Options::default()).ok());
    }

    #[test]
    fn garbage_and_oversize_are_errors() {
        assert!(!validate(b"nope", Options::default()).ok());
        assert!(!validate(&vec![0; MAX_BLOB_BYTES + 1], Options::default()).ok());
    }

    #[test]
    fn ids() {
        assert!(check_id("alice.reverb", false).is_ok());
        assert!(check_id("a-b.c_d1", false).is_ok());
        for bad in ["noprefix", "a.b.c", ".x", "x.", "Alice.x", "a b.c"] {
            assert!(check_id(bad, false).is_err(), "{bad}");
        }
        for r in ["kabl.sine", "demo.logic", "test.thing"] {
            assert!(check_id(r, false).unwrap_err().contains("reserved"), "{r}");
            assert!(check_id(r, true).is_ok(), "{r}");
        }
        // `demo` as a handle is fine: only the `demo.` prefix is reserved.
        assert!(check_id("demonic.x", false).is_ok());
    }
}
