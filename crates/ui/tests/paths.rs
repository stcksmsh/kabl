//! Directory resolution on both platforms. The pure `*_in` functions take an environment map, so
//! the Windows rules are checked on Linux too.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use kabl_ui::library::paths::{
    config_dir_in, factory_candidates_in, log_dir_in, or_temp, user_dir_in, Os, PathsError,
};

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let m: HashMap<String, OsString> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect();
    move |k| m.get(k).cloned()
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

struct Case {
    env: Vec<(&'static str, &'static str)>,
    user: &'static str,
    config: &'static str,
    log: &'static str,
}

fn case(
    env: &[(&'static str, &'static str)],
    user: &'static str,
    config: &'static str,
    log: &'static str,
) -> Case {
    Case {
        env: env.to_vec(),
        user,
        config,
        log,
    }
}

/// The values the hand-written XDG code produced before the paths module existed.
#[test]
fn unix_directories_match_the_old_resolution() {
    let table = [
        case(
            &[("HOME", "/h")],
            "/h/.local/share/kabl",
            "/h/.config/kabl",
            "/h/.local/state/kabl/logs",
        ),
        case(
            &[
                ("HOME", "/h"),
                ("XDG_DATA_HOME", "/d"),
                ("XDG_CONFIG_HOME", "/c"),
                ("XDG_STATE_HOME", "/s"),
            ],
            "/d/kabl",
            "/c/kabl",
            "/s/kabl/logs",
        ),
        case(
            &[
                ("HOME", "/h"),
                ("KABL_USER_DIR", "/u"),
                ("KABL_CONFIG_DIR", "/k"),
            ],
            "/u",
            "/k",
            "/u/logs",
        ),
        case(
            &[
                ("HOME", "/h"),
                ("KABL_USER_DIR", "/u"),
                ("KABL_LOG_DIR", "/l"),
            ],
            "/u",
            "/h/.config/kabl",
            "/l",
        ),
        // Empty counts as unset.
        case(
            &[
                ("HOME", "/h"),
                ("XDG_DATA_HOME", ""),
                ("XDG_CONFIG_HOME", ""),
                ("XDG_STATE_HOME", ""),
                ("KABL_CONFIG_DIR", ""),
            ],
            "/h/.local/share/kabl",
            "/h/.config/kabl",
            "/h/.local/state/kabl/logs",
        ),
        // Empty override counts as unset (it was an empty path before).
        case(
            &[("HOME", "/h"), ("KABL_USER_DIR", "")],
            "/h/.local/share/kabl",
            "/h/.config/kabl",
            "/h/.local/state/kabl/logs",
        ),
        // No HOME: the current directory, as before.
        case(
            &[],
            "./.local/share/kabl",
            "./.config/kabl",
            "./.local/state/kabl/logs",
        ),
    ];
    for c in &table {
        let e = env(&c.env);
        assert_eq!(user_dir_in(Os::Unix, &e), Ok(p(c.user)), "{:?}", c.env);
        assert_eq!(config_dir_in(Os::Unix, &e), Ok(p(c.config)), "{:?}", c.env);
        assert_eq!(log_dir_in(Os::Unix, &e), Ok(p(c.log)), "{:?}", c.env);
    }
}

#[test]
fn unix_factory_candidates_match_the_old_search_order() {
    let exe = Path::new("/opt/kabl/bin");
    let e = env(&[]);
    assert_eq!(
        factory_candidates_in(Os::Unix, &e, Some(exe), "patches"),
        [
            p("/opt/kabl/bin/../share/kabl/patches"),
            p("/usr/local/share/kabl/patches"),
            p("/usr/share/kabl/patches"),
        ]
    );
    let e = env(&[("XDG_DATA_DIRS", "/a:/b")]);
    assert_eq!(
        factory_candidates_in(Os::Unix, &e, None, "factory-composites"),
        [
            p("/a/kabl/factory-composites"),
            p("/b/kabl/factory-composites")
        ]
    );
    let e = env(&[("XDG_DATA_DIRS", "")]);
    assert_eq!(
        factory_candidates_in(Os::Unix, &e, None, "patches"),
        [
            p("/usr/local/share/kabl/patches"),
            p("/usr/share/kabl/patches")
        ]
    );
}

#[test]
fn windows_directories_use_appdata_and_localappdata() {
    let e = env(&[
        ("APPDATA", r"C:\Users\Ana\AppData\Roaming"),
        ("LOCALAPPDATA", r"C:\Users\Ana\AppData\Local"),
        ("USERPROFILE", r"C:\Users\Ana"),
        // Linux variables mean nothing there.
        ("XDG_DATA_HOME", "/d"),
        ("HOME", "/h"),
    ]);
    let roaming = Path::new(r"C:\Users\Ana\AppData\Roaming").join("kabl");
    assert_eq!(user_dir_in(Os::Windows, &e), Ok(roaming.clone()));
    assert_eq!(config_dir_in(Os::Windows, &e), Ok(roaming));
    assert_eq!(
        log_dir_in(Os::Windows, &e),
        Ok(Path::new(r"C:\Users\Ana\AppData\Local")
            .join("kabl")
            .join("logs"))
    );
    assert_eq!(
        factory_candidates_in(Os::Windows, &e, Some(Path::new(r"C:\Kabl")), "patches"),
        [Path::new(r"C:\Kabl")
            .join("share")
            .join("kabl")
            .join("patches")]
    );
}

#[test]
fn windows_falls_back_to_userprofile_then_errors() {
    let e = env(&[("USERPROFILE", r"C:\Users\Ana"), ("APPDATA", "")]);
    let base = Path::new(r"C:\Users\Ana").join("AppData");
    assert_eq!(
        user_dir_in(Os::Windows, &e),
        Ok(base.join("Roaming").join("kabl"))
    );
    assert_eq!(
        log_dir_in(Os::Windows, &e),
        Ok(base.join("Local").join("kabl").join("logs"))
    );
    let none = env(&[]);
    assert_eq!(
        user_dir_in(Os::Windows, &none),
        Err(PathsError("%APPDATA%"))
    );
    assert_eq!(
        config_dir_in(Os::Windows, &none),
        Err(PathsError("%APPDATA%"))
    );
    assert_eq!(
        log_dir_in(Os::Windows, &none),
        Err(PathsError("%LOCALAPPDATA%"))
    );
    assert!(PathsError("%APPDATA%").to_string().contains("%APPDATA%"));
    // The plain functions use <tmp>/kabl, never the current directory.
    let tmp = Path::new("/t");
    assert_eq!(
        or_temp(user_dir_in(Os::Windows, &none), tmp),
        tmp.join("kabl")
    );
    assert_eq!(
        or_temp(user_dir_in(Os::Windows, &e), tmp),
        base.join("Roaming").join("kabl")
    );
}

#[test]
fn overrides_win_on_windows_too() {
    let e = env(&[
        ("KABL_USER_DIR", r"D:\u"),
        ("KABL_CONFIG_DIR", r"D:\c"),
        ("KABL_LOG_DIR", r"D:\l"),
    ]);
    assert_eq!(user_dir_in(Os::Windows, &e), Ok(p(r"D:\u")));
    assert_eq!(config_dir_in(Os::Windows, &e), Ok(p(r"D:\c")));
    assert_eq!(log_dir_in(Os::Windows, &e), Ok(p(r"D:\l")));
    // No APPDATA needed once an override is set.
    let e = env(&[("KABL_USER_DIR", r"D:\u")]);
    assert_eq!(
        log_dir_in(Os::Windows, &e),
        Ok(Path::new(r"D:\u").join("logs"))
    );
}

/// The real-environment wrappers and the factory override, in one test because the
/// environment is process-wide.
#[cfg(unix)]
#[test]
fn real_environment_wrappers() {
    use kabl_ui::library::paths;
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    std::env::set_var("KABL_USER_DIR", r.join("u"));
    std::env::set_var("KABL_CONFIG_DIR", r.join("c"));
    std::env::remove_var("KABL_LOG_DIR");
    std::env::set_var("KABL_FACTORY_COMPOSITES_DIR", r.join("fc"));
    assert_eq!(paths::user_dir(), r.join("u"));
    assert_eq!(paths::config_dir(), r.join("c"));
    assert_eq!(paths::log_dir(), r.join("u").join("logs"));
    assert_eq!(kabl_ui::composites::find_factory_bank(), Some(r.join("fc")));
    // Empty factory overrides count as unset: the search continues instead of returning "".
    std::env::set_var("KABL_FACTORY_DIR", "");
    std::env::set_var("KABL_FACTORY_COMPOSITES_DIR", "");
    for var in ["KABL_FACTORY_DIR", "KABL_FACTORY_COMPOSITES_DIR"] {
        assert_eq!(
            paths::find_factory(var, "no-such-sub", r.to_path_buf(), false),
            Some(r.to_path_buf())
        );
    }
}
