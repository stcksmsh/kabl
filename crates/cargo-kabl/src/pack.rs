//! The last step of `cargo kabl build`: wrap the validated blob in `target/kabl/<id>-<version>.kablmod`.
//! Author, licence, source and description come from the crate's Cargo metadata; the manifest
//! rules (licence required, source for copyleft) are the host's own, from `kabl_modules`.

use kabl_modules::code::package::{Manifest, Package, OPTIONAL};
use kabl_modules::code::{WasmiEngine, ABI_VERSION};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Writes the package for `wasm` and returns its path.
pub fn write(wasm: &[u8], manifest_path: &Path, package: Option<&str>) -> Result<PathBuf, String> {
    let out = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest_path)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let pkgs = meta["packages"].as_array().map_or(&[][..], Vec::as_slice);
    let pkg = pkgs
        .iter()
        .find(|p| match package {
            Some(n) => p["name"] == n,
            None => Path::new(p["manifest_path"].as_str().unwrap_or("")) == manifest_path,
        })
        .ok_or("could not find the package in `cargo metadata`; pass `--package`")?;
    let text = |k: &str| pkg[k].as_str().unwrap_or("").to_owned();
    let authors: Vec<&str> = pkg["authors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.as_str())
        .collect();

    let code = WasmiEngine::new().read_meta(wasm)?;
    let manifest = Manifest {
        id: code.kind.clone(),
        version: code.version,
        abi_major: ABI_VERSION,
        abi_minor: 0,
        name: code.name.clone(),
        author: authors.join(", "),
        licence: text("license"),
        source: Some(text("repository")).filter(|s| !s.is_empty()),
        sha256: String::new(),
        min_kabl: env!("CARGO_PKG_VERSION").to_owned(),
        description: text("description"),
        capabilities: vec![],
        signature: None,
        resources: BTreeMap::new(),
    };
    // The Cargo fields feed the manifest, so say which one to set when a rule fails.
    let hint = |e: String| format!("{e} (set it in Cargo.toml: authors, license, repository)");

    let dir = Path::new(&text("manifest_path"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or("package has no directory")?;
    let files = OPTIONAL
        .iter()
        .filter_map(|n| Some((n.to_string(), std::fs::read(dir.join(n)).ok()?)))
        .collect();
    let pack = Package::new(manifest, wasm.to_vec(), files).map_err(hint)?;

    let target = meta["target_directory"]
        .as_str()
        .ok_or("no target directory")?;
    let dest = Path::new(target).join("kabl");
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let dest = dest.join(format!("{}-{}.kablmod", code.kind, code.version));
    std::fs::write(&dest, pack.to_bytes()?).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok(dest)
}
