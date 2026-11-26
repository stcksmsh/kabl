//! User-installed code modules: the `.kablmod` packages in the user module directory are
//! registered once per process (so the Add module list shows them), a package dropped on the
//! window is installed, module info shows an installed module's author, licence, source and
//! its `kabl.notices` text, and opening a patch reports versions it uses that are not installed.

use crate::UiState;
use kabl_modules::code::install::{self, Installed};
use kabl_modules::code::package::REDISTRIBUTION;
use kabl_modules::code::WasmiEngine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn dir() -> PathBuf {
    install::dir(&crate::library::default_user_root())
}

/// Registers every installed package, once. Returns the files that could not be loaded.
pub fn ensure_loaded() -> &'static [String] {
    static LOADED: OnceLock<Vec<String>> = OnceLock::new();
    LOADED.get_or_init(|| install::load_all(&dir()))
}

/// The info tooltip of each installed module id (its latest version), built from one scan of the
/// package directory. The Add-module list asks every frame, so the scan and the wasm parse for
/// the notices run once, not per row per frame; installing a package invalidates it.
#[derive(Default)]
struct Details {
    by_id: Option<HashMap<String, String>>,
    /// How many times the directory was scanned, for the test.
    scans: usize,
}

impl Details {
    fn get(&mut self, dir: &Path, kind: &str) -> Option<&str> {
        if self.by_id.is_none() {
            self.scans += 1;
            self.by_id = Some(build(dir));
        }
        self.by_id.as_ref()?.get(kind).map(String::as_str)
    }
}

fn build(dir: &Path) -> HashMap<String, String> {
    let (have, _) = install::scan(dir);
    // `scan` sorts by id then version, so the last entry of an id is its latest version.
    let mut out = HashMap::new();
    for i in &have {
        let m = &i.manifest;
        let mut s = format!(
            "{} v{} by {}\nLicence {}",
            m.name, m.version, m.author, m.licence
        );
        if let Some(src) = &m.source {
            s.push_str(&format!("\nSource {src}"));
        }
        let wasm = std::fs::read(&i.path)
            .ok()
            .and_then(|b| kabl_modules::code::package::Package::from_bytes(&b).ok())
            .map(|p| p.wasm);
        if let Some(n) = wasm.and_then(|w| WasmiEngine::new().notices(&w)) {
            s.push_str(&format!("\n\nNotices\n{n}"));
        }
        out.insert(m.id.clone(), s);
    }
    out
}

fn details() -> &'static Mutex<Details> {
    static D: OnceLock<Mutex<Details>> = OnceLock::new();
    D.get_or_init(Mutex::default)
}

/// Author, licence, source and notices of an installed module, for its info tooltip.
pub fn info(kind: &str) -> Option<String> {
    let mut d = details().lock().unwrap_or_else(|e| e.into_inner());
    d.get(&dir(), kind).map(str::to_owned)
}

/// Installs a dropped `.kablmod` and says what happened in the status line; the first install
/// also shows the redistribution clause.
pub fn dropped(ui: &mut UiState, ctx: &egui::Context) {
    let files = ctx.input(|i| i.raw.dropped_files.clone());
    for f in files {
        let name = f
            .path
            .as_ref()
            .map_or(f.name.clone(), |p| p.display().to_string());
        if !name.ends_with(".kablmod") {
            continue;
        }
        let bytes = match (&f.path, &f.bytes) {
            (Some(p), _) => std::fs::read(p).map_err(|e| e.to_string()),
            (None, Some(b)) => Ok(b.to_vec()),
            _ => Err("the dropped file could not be read".into()),
        };
        let result = bytes.and_then(|b| install::install(&dir(), &b));
        details().lock().unwrap_or_else(|e| e.into_inner()).by_id = None;
        ui.last_message = Some(match result {
            Ok(i) => installed_text(&i),
            Err(e) => format!("Module not installed: {e}"),
        });
    }
}

fn installed_text(i: &Installed) -> String {
    let m = &i.manifest;
    let mut s = format!(
        "Installed {} v{} ({}, by {}).",
        m.id, m.version, m.licence, m.author
    );
    if i.first {
        s.push(' ');
        s.push_str(REDISTRIBUTION);
    }
    s
}

/// One line per module version the patch uses that no installed package provides (it still
/// runs from the copy in the patch). The built-in examples are not packages and are skipped.
pub fn missing_report(state: &kabl_core::PatchState) -> Vec<String> {
    let (have, _) = install::scan(&dir());
    // Only the blobs matter here: no wait for the running graph's settings, and no note.
    let code = kabl_standalone::code_load::embedded_for(
        state,
        &kabl_modules::code::host::active_scope(),
        std::time::Duration::ZERO,
    );
    let used: Vec<(String, u32)> = crate::code_notice::entries(&code)
        .into_iter()
        .filter(|e| {
            !kabl_modules::code::host::EXAMPLES
                .iter()
                .any(|x| x.kind == e.module_id)
        })
        .map(|e| (e.module_id, e.version))
        .collect();
    install::missing(&have, &used)
        .into_iter()
        .map(|(id, v)| {
            format!("{id} v{v} is not installed; this patch runs its own copy of the code")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabl_modules::code::host::EXAMPLES;
    use kabl_modules::code::package::{Manifest, Package};
    use std::collections::BTreeMap;

    #[test]
    fn the_package_directory_is_scanned_once_across_many_frames() {
        let root = std::env::temp_dir().join(format!("kabl-ui-details-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = install::dir(&root);
        let wasm = EXAMPLES[0].wasm;
        let meta = WasmiEngine::new().read_meta(wasm).unwrap();
        let m = Manifest {
            id: meta.kind.clone(),
            version: meta.version,
            abi_major: 0,
            abi_minor: 1,
            name: meta.name,
            author: "alice".into(),
            licence: "0BSD".into(),
            source: None,
            sha256: String::new(),
            min_kabl: "0.1.0".into(),
            description: String::new(),
            capabilities: vec![],
            signature: None,
            resources: BTreeMap::new(),
        };
        let bytes = Package::new(m, wasm.to_vec(), BTreeMap::new())
            .unwrap()
            .to_bytes()
            .unwrap();
        install::install(&dir, &bytes).unwrap();

        let mut d = Details::default();
        for _frame in 0..100 {
            for kind in [meta.kind.as_str(), "not.installed"] {
                let tip = d.get(&dir, kind);
                assert_eq!(tip.is_some(), kind == meta.kind);
            }
        }
        assert_eq!(d.scans, 1);
        assert!(d.get(&dir, &meta.kind).unwrap().contains("by alice"));
        // An install invalidates it: the next ask scans again.
        d.by_id = None;
        d.get(&dir, &meta.kind);
        assert_eq!(d.scans, 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
