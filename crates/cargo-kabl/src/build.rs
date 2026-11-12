//! `cargo kabl build`: a pinned, path-independent release build for wasm32, then strip, add the
//! notices, validate.

use crate::validate::{self, Options};
use crate::wasm;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const TARGET: &str = "wasm32-unknown-unknown";

/// Flags every build uses. Small stack (every voice gets its own copy of linear memory), no
/// entry point, no debug info. Path remaps are added per build.
const FLAGS: [&str; 6] = [
    "-C",
    "link-arg=-zstack-size=8192",
    "-C",
    "link-arg=--no-entry",
    "-C",
    "debuginfo=0",
];

#[derive(Default)]
pub struct Args {
    pub manifest: Option<PathBuf>,
    pub package: Option<String>,
    pub locked: bool,
    pub allow_reserved: bool,
    /// Cargo features for this build (`cargo kabl test` builds the fault variants with them).
    pub features: Option<String>,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = |what: &str| it.next().cloned().ok_or(format!("{what} needs a value"));
        match x.as_str() {
            "--manifest-path" => a.manifest = Some(val("--manifest-path")?.into()),
            "-p" | "--package" => a.package = Some(val("--package")?),
            "--locked" => a.locked = true,
            "--allow-reserved" => a.allow_reserved = true,
            _ => return Err(format!("unknown argument `{x}`")),
        }
    }
    Ok(a)
}

fn output(cmd: &mut Command) -> Result<String, String> {
    let o = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "{cmd:?} failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

/// The `--remap-path-prefix` flags that keep absolute paths out of the blob.
fn remaps(dir: &Path, root: &Path) -> Result<Vec<String>, String> {
    let sysroot = output(
        Command::new("rustc")
            .current_dir(dir)
            .args(["--print", "sysroot"]),
    )?;
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    let mut v = vec![format!("--remap-path-prefix={}=/kabl-src", root.display())];
    if let Some(h) = home {
        v.push(format!("--remap-path-prefix={}=/cargo", h.display()));
    }
    v.push(format!("--remap-path-prefix={sysroot}=/rust-sysroot"));
    Ok(v)
}

/// The absolute manifest path the build uses.
pub fn manifest(args: &Args) -> Result<PathBuf, String> {
    match &args.manifest {
        Some(m) => m
            .canonicalize()
            .map_err(|e| format!("{}: {e}", m.display())),
        None => Ok(std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join("Cargo.toml")),
    }
}

/// Builds and strips the module and adds the notices; returns the blob, the raw cargo output's
/// path and its size. No validation: `cargo kabl test` builds deliberately faulty variants too.
pub fn blob(args: &Args) -> Result<(Vec<u8>, PathBuf, usize), String> {
    let manifest = manifest(args)?;
    let dir = manifest.parent().ok_or("manifest has no directory")?;
    // cargo picks `rust-toolchain.toml` up from here, which is what pins the compiler.
    let root = output(
        Command::new("cargo")
            .current_dir(dir)
            .args(["locate-project", "--workspace", "--message-format", "plain"])
            .arg("--manifest-path")
            .arg(&manifest),
    )?;
    let root = Path::new(&root).parent().unwrap_or(dir).to_owned();
    let mut flags: Vec<String> = FLAGS.iter().map(|s| (*s).to_owned()).collect();
    flags.extend(remaps(dir, &root)?);

    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir)
        .args(["build", "--release", "--target", TARGET])
        .args(["--message-format", "json-render-diagnostics"])
        .arg("--manifest-path")
        .arg(&manifest)
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"))
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(p) = &args.package {
        cmd.args(["--package", p]);
    }
    if args.locked {
        cmd.arg("--locked");
    }
    if let Some(f) = &args.features {
        cmd.args(["--features", f]);
    }
    let out = cmd.output().map_err(|e| format!("cargo: {e}"))?;
    if !out.status.success() {
        return Err("cargo build failed".into());
    }
    let raw = find_artifact(&String::from_utf8_lossy(&out.stdout))?;
    let bytes = std::fs::read(&raw).map_err(|e| format!("{}: {e}", raw.display()))?;
    let blob = wasm::finish(&bytes, wasm::NOTICE_TEXT.as_bytes())?;
    Ok((blob, raw, bytes.len()))
}

/// Runs the build and returns the path of the finished, validated blob.
pub fn run(args: &Args) -> Result<PathBuf, String> {
    let (blob, raw, raw_len) = blob(args)?;
    let dir = raw.parent().unwrap_or(Path::new("."));
    let report = validate::validate(
        &blob,
        Options {
            allow_reserved: args.allow_reserved,
        },
    );
    crate::print_report(&report);
    if !report.ok() {
        return Err("validation failed; no blob written".into());
    }
    let kind = wasm::custom(&blob, wasm::META)?
        .and_then(|m| std::str::from_utf8(m).ok())
        .and_then(|m| m.lines().find_map(|l| l.strip_prefix("kind ")))
        .ok_or("no `kind` in the meta")?
        .trim()
        .to_owned();
    let dest = dir.join("kabl");
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let dest = dest.join(format!("{kind}.wasm"));
    std::fs::write(&dest, &blob).map_err(|e| format!("{}: {e}", dest.display()))?;
    println!(
        "{}: {} bytes (cargo output {} bytes)",
        dest.display(),
        blob.len(),
        raw_len
    );
    Ok(dest)
}

/// The one `.wasm` cdylib artifact in cargo's JSON message stream.
fn find_artifact(stdout: &str) -> Result<PathBuf, String> {
    let mut found: Vec<PathBuf> = vec![];
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"] != "compiler-artifact"
            || !v["target"]["crate_types"].to_string().contains("cdylib")
        {
            continue;
        }
        for f in v["filenames"].as_array().into_iter().flatten() {
            if let Some(f) = f.as_str().filter(|f| f.ends_with(".wasm")) {
                found.push(f.into());
            }
        }
    }
    found.dedup();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err("cargo built no cdylib .wasm: is `crate-type = [\"cdylib\"]` set?".into()),
        _ => Err("several .wasm outputs; pick one with `--package`".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_is_found_among_other_messages() {
        let out = r#"{"reason":"compiler-message","message":{}}
{"reason":"compiler-artifact","target":{"crate_types":["rlib"]},"filenames":["/t/libsdk.rlib"]}
{"reason":"compiler-artifact","target":{"crate_types":["cdylib"]},"filenames":["/t/m.wasm"]}
{"reason":"build-finished","success":true}"#;
        assert_eq!(find_artifact(out).unwrap(), PathBuf::from("/t/m.wasm"));
        assert!(find_artifact("{}").is_err());
    }

    #[test]
    fn args() {
        let a = parse(&["--locked".into(), "-p".into(), "x".into()]).unwrap();
        assert!(a.locked && a.package.as_deref() == Some("x"));
        assert!(parse(&["--nope".into()]).is_err());
        assert!(parse(&["-p".into()]).is_err());
    }
}
