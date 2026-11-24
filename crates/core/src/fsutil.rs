//! Replace-on-save: the one place that moves a fully written temporary file over its target.
//!
//! On Unix this is `rename(2)`, atomic and unchanged. On Windows the replace fails with a
//! sharing violation or access-denied error while another program (virus scanner, search
//! indexer, cloud sync, a second kabl window) has the target open, so there it uses
//! `ReplaceFileW` and retries for a short, bounded time. Callers run on UI or worker
//! threads, never the audio thread: the retry sleeps.

use std::io;
use std::path::Path;
use std::time::Duration;

/// Windows `ERROR_ACCESS_DENIED` and `ERROR_SHARING_VIOLATION`: a target another program holds.
const WINDOWS_BUSY: [i32; 2] = [5, 32];

/// Waits between attempts: about 0.95 s in all, then the save fails.
const BACKOFF_MS: [u64; 7] = [10, 20, 40, 80, 160, 320, 320];

/// How long to wait before retry number `attempt` (0 for the first retry) after `error`, or
/// `None` when the error is final. Only the "target is busy" codes are retried.
pub fn retry_delay(error: &io::Error, attempt: usize) -> Option<Duration> {
    let busy = error
        .raw_os_error()
        .is_some_and(|c| WINDOWS_BUSY.contains(&c));
    if !busy {
        return None;
    }
    BACKOFF_MS.get(attempt).map(|&ms| Duration::from_millis(ms))
}

/// Moves `from` over `to` (same directory, hence same volume). On failure `from` is removed and
/// `to` is untouched; a target that stayed busy for the whole budget says so in the error.
pub fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    // Windows reports a read-only target as "access denied", the same code as a busy one.
    if cfg!(windows) && std::fs::metadata(to).is_ok_and(|m| m.permissions().readonly()) {
        let _ = std::fs::remove_file(from);
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is read-only", to.display()),
        ));
    }
    replace_with(
        from,
        to,
        cfg!(windows),
        || os_replace(from, to),
        std::thread::sleep,
    )
}

/// The retry loop of [`replace_file`] with the replace step and the sleep injected, so the
/// policy can be tested on any platform. With `retry` false the first error is final.
fn replace_with(
    from: &Path,
    to: &Path,
    retry: bool,
    mut replace: impl FnMut() -> io::Result<()>,
    mut sleep: impl FnMut(Duration),
) -> io::Result<()> {
    let mut attempt = 0;
    let error = loop {
        let e = match replace() {
            Ok(()) => return Ok(()),
            Err(e) => e,
        };
        match retry.then(|| retry_delay(&e, attempt)).flatten() {
            Some(wait) => {
                sleep(wait);
                attempt += 1;
            }
            None => break e,
        }
    };
    let _ = std::fs::remove_file(from);
    if attempt > 0 {
        return Err(io::Error::new(
            error.kind(),
            format!(
                "{} is in use by another program and could not be replaced: {error}",
                to.display()
            ),
        ));
    }
    Err(error)
}

#[cfg(not(windows))]
fn os_replace(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(windows)]
fn os_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut core::ffi::c_void,
            reserved: *mut core::ffi::c_void,
        ) -> i32;
    }
    const REPLACEFILE_IGNORE_MERGE_ERRORS: u32 = 0x2;

    // ReplaceFileW needs an existing target; a first save is a plain rename.
    if !to.exists() {
        return std::fs::rename(from, to);
    }
    let wide = |p: &Path| p.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (replaced, replacement) = (wide(to), wide(from));
    // SAFETY: both pointers come from live, NUL-terminated UTF-16 buffers that outlive the
    // call; the backup, exclude and reserved arguments are documented as nullable.
    let ok = unsafe {
        ReplaceFileW(
            replaced.as_ptr(),
            replacement.as_ptr(),
            core::ptr::null(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn busy() -> io::Error {
        io::Error::from_raw_os_error(32)
    }

    #[test]
    fn succeeds_after_three_sharing_violations() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a.tmp"), dir.path().join("a"));
        std::fs::write(&from, "new").unwrap();
        let calls = Cell::new(0);
        let slept = RefCell::new(Vec::new());
        replace_with(
            &from,
            &to,
            true,
            || {
                calls.set(calls.get() + 1);
                if calls.get() <= 3 {
                    Err(busy())
                } else {
                    std::fs::rename(&from, &to)
                }
            },
            |d| slept.borrow_mut().push(d),
        )
        .unwrap();
        assert_eq!(calls.get(), 4);
        assert_eq!(slept.borrow().len(), 3);
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "new");
    }

    #[test]
    fn permanent_violation_stops_within_budget_and_removes_temp() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a.tmp"), dir.path().join("a"));
        std::fs::write(&from, "new").unwrap();
        let total = Cell::new(Duration::ZERO);
        let e = replace_with(
            &from,
            &to,
            true,
            || Err(busy()),
            |d| total.set(total.get() + d),
        )
        .unwrap_err();
        assert!(total.get() <= Duration::from_secs(1), "{:?}", total.get());
        assert!(e.to_string().contains("in use"), "{e}");
        assert!(!from.exists());
    }

    #[test]
    fn other_errors_return_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a.tmp"), dir.path().join("a"));
        std::fs::write(&from, "new").unwrap();
        let calls = Cell::new(0);
        let e = replace_with(
            &from,
            &to,
            true,
            || {
                calls.set(calls.get() + 1);
                Err(io::Error::from(io::ErrorKind::NotFound))
            },
            |_| panic!("must not sleep"),
        )
        .unwrap_err();
        assert_eq!(calls.get(), 1);
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        assert!(!from.exists());
    }

    #[test]
    fn without_retry_a_busy_error_is_final() {
        let calls = Cell::new(0);
        let p = Path::new("x");
        replace_with(
            p,
            p,
            false,
            || {
                calls.set(calls.get() + 1);
                Err(busy())
            },
            |_| panic!("must not sleep"),
        )
        .unwrap_err();
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn replace_file_overwrites_and_creates() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a.tmp"), dir.path().join("a"));
        std::fs::write(&from, "one").unwrap();
        replace_file(&from, &to).unwrap();
        std::fs::write(&from, "two").unwrap();
        replace_file(&from, &to).unwrap();
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "two");
        assert!(!from.exists());
    }
}
