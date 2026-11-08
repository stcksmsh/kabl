//! Builds the two SDK examples for wasm32 and checks the blobs: exports, sections, no imports,
//! no buffers shipped as data, size. Needs `rustup target add wasm32-unknown-unknown`.
//!
//! This does NOT load them into kabl's `WasmiEngine`: the current host rejects `abi_minor`
//! (`meta line 3: unknown key`), which is D12 PR 0's job. Until then, loading is checked by the
//! gate spike only (see docs/product-research/d12/GATE.md).

use std::path::{Path, PathBuf};
use std::process::Command;

fn leb(b: &[u8], i: &mut usize) -> usize {
    let (mut v, mut shift) = (0usize, 0);
    loop {
        let c = b[*i];
        *i += 1;
        v |= ((c & 0x7f) as usize) << shift;
        if c & 0x80 == 0 {
            return v;
        }
        shift += 7;
    }
}

struct Blob {
    exports: Vec<String>,
    has_imports: bool,
    data_bytes: usize,
    customs: Vec<(String, Vec<u8>)>,
    len: usize,
}

fn parse(b: &[u8]) -> Blob {
    assert_eq!(&b[..8], b"\0asm\x01\0\0\0");
    let mut out = Blob {
        exports: vec![],
        has_imports: false,
        data_bytes: 0,
        customs: vec![],
        len: b.len(),
    };
    let mut i = 8;
    while i < b.len() {
        let id = b[i];
        i += 1;
        let size = leb(b, &mut i);
        let body = &b[i..i + size];
        match id {
            0 => {
                let mut j = 0;
                let n = leb(body, &mut j);
                let name = String::from_utf8(body[j..j + n].to_vec()).unwrap();
                out.customs.push((name, body[j + n..].to_vec()));
            }
            2 => out.has_imports = true,
            7 => {
                let mut j = 0;
                for _ in 0..leb(body, &mut j) {
                    let n = leb(body, &mut j);
                    out.exports
                        .push(String::from_utf8(body[j..j + n].to_vec()).unwrap());
                    j += n + 1;
                    leb(body, &mut j);
                }
            }
            11 => out.data_bytes = size,
            _ => {}
        }
        i += size;
    }
    out
}

fn build() -> PathBuf {
    let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let status = Command::new(env!("CARGO"))
        .current_dir(&ws)
        .args([
            "build",
            "--release",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .args(["-p", "sdk-demo-logic", "-p", "sdk-demo-bitfilter"])
        .status()
        .expect("run cargo");
    assert!(
        status.success(),
        "wasm build failed (rustup target add wasm32-unknown-unknown)"
    );
    ws.join("target/wasm32-unknown-unknown/release")
}

fn check(dir: &Path, file: &str, kind: &str, max_len: usize) {
    let blob = parse(&std::fs::read(dir.join(file)).unwrap());
    // wasm-ld adds the two address globals; the old hand-written examples have them too.
    let mut exports = blob.exports.clone();
    exports.retain(|e| e != "__data_end" && e != "__heap_base");
    exports.sort();
    assert_eq!(
        exports,
        [
            "init",
            "layout_ext",
            "memory",
            "process",
            "reset",
            "state_load",
            "state_save"
        ],
        "{file}"
    );
    assert!(!blob.has_imports, "{file} imports nothing");
    let custom = |n: &str| {
        let hits: Vec<_> = blob.customs.iter().filter(|c| c.0 == n).collect();
        assert_eq!(hits.len(), 1, "{file}: one `{n}` section");
        String::from_utf8(hits[0].1.clone()).unwrap()
    };
    let meta = custom("kabl.meta");
    assert!(
        meta.starts_with(&format!("kind {kind}\nversion 1\nabi_minor 1\n")),
        "{file}"
    );
    for line in meta.lines() {
        let key = line.split(' ').next().unwrap();
        assert!(
            [
                "kind",
                "version",
                "abi_minor",
                "name",
                "explain",
                "category",
                "rate",
                "width",
                "port",
                "param",
                "advanced"
            ]
            .contains(&key),
            "{file}: unexpected meta key `{key}`"
        );
    }
    let notices = custom("kabl.notices");
    assert!(
        notices.contains("MIT") && notices.contains("compiler_builtins"),
        "{file}"
    );
    // Zeroed statics are .bss: the data segment holds only the two `link_section` statics
    // (emitted a second time there) plus a little slack, not the ~41 KB of buffers of v0.
    assert!(
        blob.data_bytes <= meta.len() + notices.len() + 64,
        "{file}: data section {} B is more than meta + notices",
        blob.data_bytes
    );
    assert!(blob.len <= max_len, "{file}: {} B", blob.len);
    println!("{file}: {} B total, {} B data", blob.len, blob.data_bytes);
}

#[test]
fn both_examples_build_to_small_blobs_with_the_expected_exports_and_sections() {
    let dir = build();
    check(&dir, "sdk_demo_logic.wasm", "demo.logic", 16 * 1024);
    check(&dir, "sdk_demo_bitfilter.wasm", "demo.bitfilter", 20 * 1024);
}
