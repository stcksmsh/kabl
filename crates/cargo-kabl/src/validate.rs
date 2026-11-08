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

/// Keys the host parser knows. Anything else is ignored by the host, so a typo would pass
/// silently; the validator warns instead.
pub const KNOWN_KEYS: [&str; 12] = [
    "abi",
    "abi_minor",
    "kind",
    "version",
    "name",
    "explain",
    "category",
    "rate",
    "width",
    "port",
    "param",
    "advanced",
];

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

/// Keys in the meta text the host would ignore.
pub fn unknown_keys(meta_text: &str) -> Vec<(usize, String)> {
    meta_text
        .lines()
        .enumerate()
        .filter_map(|(n, l)| {
            let l = l.trim();
            let key = l.split_whitespace().next()?;
            (!l.starts_with('#') && !KNOWN_KEYS.contains(&key)).then(|| (n + 1, key.to_owned()))
        })
        .collect()
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
            for (line, key) in unknown_keys(text) {
                r.warnings.push(format!(
                    "meta line {line}: unknown key `{key}` (the host ignores it; a typo?)"
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
// `f32::MAX` is left out on purpose: any module that scales its input overflows on it.
const STIMULI: [Stimulus; 6] = [
    ("silence", |_| 0.0),
    ("impulse", |i| if i == 0 { 1.0 } else { 0.0 }),
    ("full-scale square", |i| if i % 2 == 0 { 1.0 } else { -1.0 }),
    ("+-1e30", |i| if i % 2 == 0 { 1.0e30 } else { -1.0e30 }),
    ("f32::MIN_POSITIVE", |_| f32::MIN_POSITIVE),
    ("denormals", |i| if i % 2 == 0 { 1.0e-40 } else { -1.0e-40 }),
];

/// Output beyond this is allowed but probably a blow-up.
const BIG: f32 = 1.0e6;

fn smoke(meta: &CodeMeta, inst: &mut dyn WasmInstance, r: &mut Report) {
    let layout = inst.layout();
    let frames = FRAMES.min(layout.max_block as usize);
    let sets: [(&str, Vec<f32>); 3] = [
        ("default", meta.params.iter().map(|p| p.default).collect()),
        ("minimum", meta.params.iter().map(|p| p.min).collect()),
        ("maximum", meta.params.iter().map(|p| p.max).collect()),
    ];
    let mut max_fuel = 0u64;
    for (sname, f) in STIMULI {
        for (pname, params) in &sets {
            if let Err(f) = inst.reset() {
                r.errors.push(format!("reset {}", f.label()));
                return;
            }
            for block in 0..BLOCKS {
                let mem = inst.memory();
                for slot in 0..layout.in_slots as usize {
                    let range = layout.input_range(slot);
                    for (i, c) in mem[range].chunks_exact_mut(4).take(frames).enumerate() {
                        c.copy_from_slice(&f(block * frames + i).to_le_bytes());
                    }
                }
                for (i, v) in params.iter().enumerate() {
                    mem[layout.param_range(i)].copy_from_slice(&v.to_le_bytes());
                }
                inst.set_fuel(FUEL);
                match inst.call_block(frames as u32) {
                    Ok(fuel) => max_fuel = max_fuel.max(fuel),
                    Err(fault) => {
                        r.errors.push(format!(
                            "smoke render: module {} (input: {sname}, params: {pname})",
                            fault.label()
                        ));
                        return;
                    }
                }
                let mem = inst.memory();
                for slot in 0..layout.out_slots as usize {
                    let out = || {
                        mem[layout.output_range(slot)]
                            .chunks_exact(4)
                            .take(frames)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    };
                    let bad = out().any(|v| !v.is_finite());
                    // A huge output from a huge input is no blow-up; only judge the sane inputs.
                    let sane = sname != "+-1e30";
                    if sane
                        && out().any(|v| v.abs() > BIG)
                        && !r.warnings.iter().any(|w| w.contains("exceeds"))
                    {
                        r.warnings.push(format!(
                            "output slot {slot} exceeds +-{BIG} (input: {sname}, params: {pname}); a blow-up?"
                        ));
                    }
                    if bad {
                        r.errors.push(format!(
                            "smoke render: output slot {slot} is not finite (input: {sname}, \
                             params: {pname}); the host disables a module that does this"
                        ));
                        return;
                    }
                }
            }
        }
    }
    r.notes.push(format!(
        "smoke render ok: {} stimuli x {} param sets, {frames}-frame blocks, at most {max_fuel} fuel per block",
        STIMULI.len(),
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
        // The warning comes from our own scan, before the host's parser runs.
        let raw = blob("alice.t", "abi_minro 1\\n", "", 16448, SILENT);
        let r = validate(&raw, Options::default());
        let w = text(&r.warnings);
        assert!(w.contains("unknown key `abi_minro`"), "{w}");
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

    #[test]
    fn unknown_keys_are_found_with_their_line() {
        let t = "kind a.b\n# c\nversion 1\nabi_minro 1\n\nfoo\nparam g 0 1 0 linear 5\n";
        assert_eq!(
            unknown_keys(t),
            vec![(4, "abi_minro".into()), (6, "foo".into())]
        );
    }
}
