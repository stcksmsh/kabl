//! User-installed code modules: the `.kablmod` packages in the user module directory are
//! registered once per process (so the Add module list shows them), a package dropped on the
//! window is installed, module info shows an installed module's author, licence, source and
//! its `kabl.notices` text, and opening a patch reports versions it uses that are not installed.

use crate::UiState;
use kabl_modules::code::install::{self, Installed};
use kabl_modules::code::package::REDISTRIBUTION;
use kabl_modules::code::WasmiEngine;
use std::path::PathBuf;
use std::sync::OnceLock;

fn dir() -> PathBuf {
    install::dir(&crate::library::default_user_root())
}

/// Registers every installed package, once. Returns the files that could not be loaded.
pub fn ensure_loaded() -> &'static [String] {
    static LOADED: OnceLock<Vec<String>> = OnceLock::new();
    LOADED.get_or_init(|| install::load_all(&dir()))
}

/// Author, licence, source and notices of an installed module, for its info tooltip.
pub fn info(kind: &str) -> Option<String> {
    let (have, _) = install::scan(&dir());
    let i = have.iter().rfind(|i| i.manifest.id == kind)?;
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
    Some(s)
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
        ui.last_message = Some(match bytes.and_then(|b| install::install(&dir(), &b)) {
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
    let used: Vec<(String, u32)> =
        crate::code_notice::entries(&crate::code_notice::embedded_for(state))
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
