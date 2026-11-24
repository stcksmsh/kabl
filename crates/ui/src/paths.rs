//! Every directory kabl reads or writes, resolved in one place. See `docs/windows/PATHS.md`.
//!
//! This file is compiled into two crates (`kabl-ui` as `library::paths`, `kabl-standalone` as
//! `paths`) because the standalone crate is below the UI crate and needs the log directory.
//! It therefore uses only `std`.
//!
//! The `*_in` functions are pure: they take the platform and an environment lookup, so both
//! platforms are tested on any host. The plain functions use the host and the real environment.
//! An environment variable that is set but empty counts as unset.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Unix,
    Windows,
}

pub const HOST: Os = if cfg!(windows) { Os::Windows } else { Os::Unix };

/// Environment lookup.
pub type Vars<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// No usable base directory could be found.
#[derive(Debug, PartialEq, Eq)]
pub struct PathsError(pub &'static str);

impl fmt::Display for PathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot locate the {} folder", self.0)
    }
}

impl std::error::Error for PathsError {}

fn var(vars: Vars, key: &str) -> Option<PathBuf> {
    vars(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn home(vars: Vars) -> PathBuf {
    var(vars, "HOME").unwrap_or_else(|| PathBuf::from("."))
}

/// The user's home folder: `HOME`, on Windows `USERPROFILE`. `None` when it is not set, so the
/// caller can pick its own fallback.
pub fn home_dir_in(os: Os, vars: Vars) -> Option<PathBuf> {
    var(
        vars,
        if os == Os::Windows {
            "USERPROFILE"
        } else {
            "HOME"
        },
    )
}

pub fn home_dir() -> Option<PathBuf> {
    home_dir_in(HOST, &real)
}

/// `%APPDATA%`, else `%USERPROFILE%\AppData\Roaming`.
fn roaming(vars: Vars) -> Result<PathBuf, PathsError> {
    var(vars, "APPDATA")
        .or_else(|| var(vars, "USERPROFILE").map(|p| p.join("AppData").join("Roaming")))
        .ok_or(PathsError("%APPDATA%"))
}

/// `%LOCALAPPDATA%`, else `%USERPROFILE%\AppData\Local`.
fn local(vars: Vars) -> Result<PathBuf, PathsError> {
    var(vars, "LOCALAPPDATA")
        .or_else(|| var(vars, "USERPROFILE").map(|p| p.join("AppData").join("Local")))
        .ok_or(PathsError("%LOCALAPPDATA%"))
}

/// User library root: `KABL_USER_DIR`, else `$XDG_DATA_HOME/kabl`, else `~/.local/share/kabl`
/// (Windows: `%APPDATA%\kabl`).
pub fn user_dir_in(os: Os, vars: Vars) -> Result<PathBuf, PathsError> {
    if let Some(d) = var(vars, "KABL_USER_DIR") {
        return Ok(d);
    }
    match os {
        Os::Unix => Ok(var(vars, "XDG_DATA_HOME")
            .map(|d| d.join("kabl"))
            .unwrap_or_else(|| home(vars).join(".local/share/kabl"))),
        Os::Windows => Ok(roaming(vars)?.join("kabl")),
    }
}

/// Config and themes: `KABL_CONFIG_DIR`, else `$XDG_CONFIG_HOME/kabl`, else `~/.config/kabl`
/// (Windows: `%APPDATA%\kabl`).
pub fn config_dir_in(os: Os, vars: Vars) -> Result<PathBuf, PathsError> {
    if let Some(d) = var(vars, "KABL_CONFIG_DIR") {
        return Ok(d);
    }
    match os {
        Os::Unix => Ok(var(vars, "XDG_CONFIG_HOME")
            .map(|d| d.join("kabl"))
            .unwrap_or_else(|| home(vars).join(".config/kabl"))),
        Os::Windows => Ok(roaming(vars)?.join("kabl")),
    }
}

/// Logs: `KABL_LOG_DIR`, else `KABL_USER_DIR/logs`, else `$XDG_STATE_HOME/kabl/logs`, else
/// `~/.local/state/kabl/logs` (Windows: `%LOCALAPPDATA%\kabl\logs`).
pub fn log_dir_in(os: Os, vars: Vars) -> Result<PathBuf, PathsError> {
    if let Some(d) = var(vars, "KABL_LOG_DIR") {
        return Ok(d);
    }
    if let Some(d) = var(vars, "KABL_USER_DIR") {
        return Ok(d.join("logs"));
    }
    match os {
        Os::Unix => Ok(var(vars, "XDG_STATE_HOME")
            .map(|d| d.join("kabl").join("logs"))
            .unwrap_or_else(|| home(vars).join(".local/state/kabl/logs"))),
        Os::Windows => Ok(local(vars)?.join("kabl").join("logs")),
    }
}

/// Where an installed build keeps factory content `sub`, best first. `exe_dir` is the
/// executable's directory. Unix: `<exe>/../share/kabl/<sub>`, then `$XDG_DATA_DIRS/kabl/<sub>`.
/// Windows: `<exe>\share\kabl\<sub>`.
pub fn factory_candidates_in(
    os: Os,
    vars: Vars,
    exe_dir: Option<&Path>,
    sub: &str,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    match os {
        Os::Unix => {
            if let Some(bin) = exe_dir {
                out.push(bin.join("../share/kabl").join(sub));
            }
            let data_dirs = vars("XDG_DATA_DIRS")
                .and_then(|s| s.into_string().ok())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
            out.extend(
                data_dirs
                    .split(':')
                    .map(|d| Path::new(d).join("kabl").join(sub)),
            );
        }
        Os::Windows => {
            if let Some(bin) = exe_dir {
                out.push(bin.join("share").join("kabl").join(sub));
            }
        }
    }
    out
}

/// The directory when no base folder exists (Windows without `APPDATA` and `USERPROFILE`):
/// `<tmp>/kabl`. The current directory could be `System32` or the host's folder. The failure is
/// reported on stderr and to the `log` crate.
pub fn or_temp(r: Result<PathBuf, PathsError>, tmp: &Path) -> PathBuf {
    r.unwrap_or_else(|e| {
        let dir = tmp.join("kabl");
        eprintln!("kabl: {e}; using {}", dir.display());
        log::error!("{e}; using {}", dir.display());
        dir
    })
}

fn real(key: &str) -> Option<OsString> {
    std::env::var_os(key)
}

pub fn user_dir() -> PathBuf {
    or_temp(user_dir_in(HOST, &real), &std::env::temp_dir())
}

pub fn config_dir() -> PathBuf {
    or_temp(config_dir_in(HOST, &real), &std::env::temp_dir())
}

pub fn log_dir() -> PathBuf {
    or_temp(log_dir_in(HOST, &real), &std::env::temp_dir())
}

/// A factory directory: `env_var`, else the installed layouts (`factory_candidates_in`), else
/// `dev`, the source checkout's copy (a development build). The first that is a directory wins.
/// `canonical` resolves symlinks in the result.
pub fn find_factory(env_var: &str, sub: &str, dev: PathBuf, canonical: bool) -> Option<PathBuf> {
    if let Some(d) = var(&real, env_var) {
        return Some(d);
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.canonicalize().ok())
        .and_then(|e| e.parent().map(Path::to_path_buf));
    let mut candidates = factory_candidates_in(HOST, &real, exe_dir.as_deref(), sub);
    candidates.push(dev);
    candidates.into_iter().find(|d| d.is_dir()).map(|d| {
        if canonical {
            d.canonicalize().unwrap_or(d)
        } else {
            d
        }
    })
}
