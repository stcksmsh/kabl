//! `cargo kabl new`: copies `sdk/template` (embedded at build time) into a new crate.

use crate::validate::check_id;
use std::path::{Path, PathBuf};

const CARGO_TOML: &str = include_str!("../../../sdk/template/Cargo.toml");
const TOOLCHAIN: &str = include_str!("../../../sdk/template/rust-toolchain.toml");
const GITIGNORE: &str = include_str!("../../../sdk/template/.gitignore");
const AUDIO: &str = include_str!("../../../sdk/template/src/lib.rs");
const CONTROL: &str = include_str!("../../../sdk/template/control.rs");
const README: &str = include_str!("../../../sdk/template/README.md");
const LICENSE: &str = include_str!("../../../sdk/template/LICENSE");

/// What the template contains in place of your values.
const T_ID: &str = "yourhandle.gain";
const T_CRATE: &str = "yourhandle-gain";
const T_SDK: &str = r#"path = "../kabl-module-sdk""#;
const SDK_GIT: &str = r#"git = "https://github.com/stcksmsh/kabl""#;

pub struct Args {
    pub id: String,
    pub control: bool,
    pub sdk_path: Option<PathBuf>,
    pub into: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    let (mut id, mut control, mut sdk_path, mut into) = (None, false, None, None);
    let mut it = args.iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--audio" => control = false,
            "--control" => control = true,
            "--sdk-path" => {
                sdk_path = Some(it.next().ok_or("--sdk-path needs a directory")?.into())
            }
            "--into" => into = Some(it.next().ok_or("--into needs a directory")?.into()),
            s if s.starts_with('-') => return Err(format!("unknown argument `{s}`")),
            s if id.is_none() => id = Some(s.to_owned()),
            s => return Err(format!("unexpected argument `{s}`")),
        }
    }
    Ok(Args {
        id: id.ok_or("usage: cargo kabl new <githubhandle.module> [--audio|--control]")?,
        control,
        sdk_path,
        into,
    })
}

fn year() -> u64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    1970 + secs / 31_556_952
}

/// The files of the new project, relative path and contents.
pub fn render(a: &Args, year: u64) -> Result<Vec<(&'static str, String)>, String> {
    check_id(&a.id, false)?;
    let handle = a.id.split('.').next().unwrap_or("");
    let krate = a.id.replace('.', "-");
    let sdk = match &a.sdk_path {
        Some(p) => {
            let p = p
                .canonicalize()
                .map_err(|e| format!("--sdk-path {}: {e}", p.display()))?;
            format!(r#"path = "{}""#, p.display())
        }
        None => SDK_GIT.to_owned(),
    };
    let sub = |s: &str| {
        s.replace(T_ID, &a.id)
            .replace(T_CRATE, &krate)
            .replace(T_SDK, &sdk)
            .replace("<year>", &year.to_string())
            .replace("<your name>", handle)
    };
    Ok(vec![
        ("Cargo.toml", sub(CARGO_TOML)),
        ("rust-toolchain.toml", TOOLCHAIN.to_owned()),
        (".gitignore", GITIGNORE.to_owned()),
        ("src/lib.rs", sub(if a.control { CONTROL } else { AUDIO })),
        ("README.md", sub(README)),
        ("LICENSE", sub(LICENSE)),
    ])
}

/// Creates the project directory `<into>/<id with . as ->` and returns it.
pub fn run(a: &Args) -> Result<PathBuf, String> {
    let files = render(a, year())?;
    let base = a.into.clone().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join(a.id.replace('.', "-"));
    if dir.exists() {
        return Err(format!("{} already exists", dir.display()));
    }
    for (rel, text) in files {
        let p: &Path = &dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap_or(&dir)).map_err(|e| e.to_string())?;
        std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(id: &str, control: bool) -> Args {
        Args {
            id: id.into(),
            control,
            sdk_path: None,
            into: None,
        }
    }

    #[test]
    fn values_are_substituted_everywhere() {
        for control in [false, true] {
            for (_, text) in render(&args("alice.wobble", control), 2026).unwrap() {
                assert!(!text.contains("yourhandle"), "{text}");
                assert!(!text.contains("<year>") && !text.contains("<your name>"));
            }
        }
        let files = render(&args("alice.wobble", false), 2026).unwrap();
        let get = |n: &str| &files.iter().find(|f| f.0 == n).unwrap().1;
        assert!(get("Cargo.toml").contains(r#"name = "alice-wobble""#));
        assert!(get("Cargo.toml").contains("git = "));
        assert!(get("src/lib.rs").contains(r#"Meta::new("alice.wobble", 1)"#));
        assert!(get("LICENSE").contains("Copyright (c) 2026 alice"));
        assert!(get("rust-toolchain.toml").contains("channel = "));
    }

    #[test]
    fn control_replaces_the_module() {
        let files = render(&args("alice.wobble", true), 2026).unwrap();
        assert!(files
            .iter()
            .any(|f| f.0 == "src/lib.rs" && f.1.contains("Scale")));
    }

    #[test]
    fn bad_and_reserved_ids_are_refused() {
        for id in ["kabl.x", "demo.x", "test.x", "nodot", "A.b"] {
            assert!(render(&args(id, false), 2026).is_err(), "{id}");
        }
    }

    #[test]
    fn parse_flags() {
        let a = parse(&["a.b".into(), "--control".into()]).unwrap();
        assert!(a.control && a.id == "a.b");
        assert!(parse(&[]).is_err());
        assert!(parse(&["a.b".into(), "c.d".into()]).is_err());
    }
}
