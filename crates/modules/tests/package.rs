//! `.kablmod` packages: round trip, the wasm-is-authoritative check, and the install rules.

use kabl_modules::code::host::EXAMPLES;
use kabl_modules::code::install::{self, load_all, missing, scan};
use kabl_modules::code::package::{is_copyleft, Manifest, Package};
use kabl_modules::code::WasmiEngine;
use std::collections::BTreeMap;

fn manifest(author: &str) -> Manifest {
    let wasm = EXAMPLES[0].wasm;
    let meta = WasmiEngine::new().read_meta(wasm).unwrap();
    Manifest {
        id: meta.kind,
        version: meta.version,
        abi_major: 0,
        abi_minor: 1,
        name: meta.name,
        author: author.into(),
        licence: "0BSD".into(),
        source: None,
        sha256: String::new(),
        min_kabl: "0.1.0".into(),
        description: "test".into(),
        capabilities: vec![],
        signature: None,
        resources: BTreeMap::new(),
    }
}

fn package(author: &str) -> Package {
    let files = BTreeMap::from([("README.md".to_owned(), b"hello".to_vec())]);
    Package::new(manifest(author), EXAMPLES[0].wasm.to_vec(), files).unwrap()
}

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("kabl-pkg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn round_trip_is_byte_stable() {
    let p = package("a");
    let bytes = p.to_bytes().unwrap();
    assert_eq!(bytes, p.to_bytes().unwrap());
    let back = Package::from_bytes(&bytes).unwrap();
    assert_eq!(back.manifest, p.manifest);
    assert_eq!(back.wasm, p.wasm);
    assert_eq!(back.files["README.md"], b"hello");
}

#[test]
fn a_manifest_that_disagrees_with_the_wasm_is_rejected() {
    let mut p = package("a");
    p.manifest.name = "Something else".into();
    assert!(p.check().unwrap_err().contains("manifest says"));
    let mut p = package("a");
    p.manifest.sha256 = "0".repeat(64);
    assert!(p.check().unwrap_err().contains("sha256"));
    let mut p = package("a");
    p.manifest.version += 1;
    assert!(p.check().is_err());
}

#[test]
fn licence_rules() {
    assert!(is_copyleft("GPL-3.0-or-later") && is_copyleft("MIT OR AGPL-3.0-only"));
    assert!(!is_copyleft("0BSD") && !is_copyleft("MIT OR Apache-2.0"));
    let mut m = manifest("a");
    m.licence = "GPL-3.0-only".into();
    assert!(m.check().unwrap_err().contains("source"));
    m.source = Some("https://example.org/src".into());
    assert!(m.check().is_ok());
    m.licence = " ".into();
    assert!(m.check().unwrap_err().contains("licence"));
}

#[test]
fn damaged_or_foreign_zips_are_refused() {
    let bytes = package("a").to_bytes().unwrap();
    assert!(Package::from_bytes(b"not a zip").is_err());
    let mut flipped = bytes.clone();
    flipped[60] ^= 0xff; // inside the wasm bytes
    assert!(Package::from_bytes(&flipped)
        .unwrap_err()
        .contains("checksum"));
    assert!(Package::from_bytes(&bytes[..bytes.len() - 10]).is_err());
}

#[test]
fn install_rules() {
    let root = temp("rules");
    let dir = install::dir(&root);
    let a = package("alice");
    let first = install::install(&dir, &a.to_bytes().unwrap()).unwrap();
    assert!(first.first && first.path.exists());
    // Same package again: fine, and no longer the first.
    assert!(
        !install::install(&dir, &a.to_bytes().unwrap())
            .unwrap()
            .first
    );
    // Another author, same id: first install wins.
    let err = install::install(&dir, &package("bob").to_bytes().unwrap()).unwrap_err();
    assert!(err.contains("alice"), "{err}");
    let (have, bad) = scan(&dir);
    assert_eq!((have.len(), bad.len()), (1, 0));
    // The registry sees the installed module.
    assert!(load_all(&dir).is_empty());
    // Used versions with no package are reported.
    let used = vec![
        (a.manifest.id.clone(), a.manifest.version),
        ("x.y".into(), 3),
    ];
    assert_eq!(missing(&have, &used), vec![("x.y".to_owned(), 3)]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn same_version_other_code_is_refused_with_both_hashes() {
    let root = temp("hash");
    let dir = install::dir(&root);
    let a = package("alice");
    install::install(&dir, &a.to_bytes().unwrap()).unwrap();
    // Same id and version, different bytes: an extra custom section changes the hash only.
    let mut wasm = EXAMPLES[0].wasm.to_vec();
    wasm.extend([0, 2, 1, b'x']);
    let b = Package::new(manifest("alice"), wasm, BTreeMap::new()).unwrap();
    let err = install::install(&dir, &b.to_bytes().unwrap()).unwrap_err();
    assert!(
        err.contains(&a.manifest.sha256) && err.contains(&b.manifest.sha256),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ids_cannot_escape_the_directory() {
    let root = temp("id");
    let mut p = package("a");
    p.manifest.id = "../evil".into();
    // Not a valid package (the id disagrees with the wasm), so it never reaches the file system.
    assert!(install::install(&install::dir(&root), &p.to_bytes().unwrap()).is_err());
    assert!(!root.exists());
}

#[test]
fn notices_are_readable_from_the_wasm() {
    // The committed examples predate `kabl.notices`; the call just must not fail.
    let _ = WasmiEngine::new().notices(EXAMPLES[0].wasm);
}
