//! The `.kabl` file tools behind `kabl cat`, `kabl pack` and `kabl unpack`. The standalone binary
//! hands these three words to [`run`] before it starts audio or logging, so they are cheap enough
//! to use as a git textconv.
use crate::format;
use std::io::Write;
use std::path::{Path, PathBuf};

/// First words that mean "a file tool": the binary calls [`run`] for these only.
pub const COMMANDS: [&str; 4] = ["cat", "pack", "pack-tree", "unpack"];

const USAGE: &str = "usage: kabl cat FILE.kabl | kabl pack DIR [OUT.kabl] | kabl pack-tree SRC DST | kabl unpack FILE.kabl [DIR]";

/// Run one tool. `args` is the command line without the program name (`["cat", "a.kabl"]`);
/// `cat` writes the log to `out`. An `Err` is a message for the user: print it and exit non-zero.
pub fn run(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    match (arg(0), arg(1), arg(2), arg(3)) {
        (Some("cat"), Some(file), None, _) => cat(Path::new(file), out),
        (Some("pack"), Some(dir), out_path, None) => pack(Path::new(dir), out_path.map(Path::new)),
        (Some("pack-tree"), Some(src), Some(dst), None) => {
            let n = format::pack_tree(Path::new(src), Path::new(dst))
                .map_err(|e| format!("{src}: {e:?}"))?;
            println!("packed {n} patches into {dst}");
            Ok(())
        }
        (Some("unpack"), Some(file), dir, None) => unpack(Path::new(file), dir.map(Path::new)),
        _ => Err(USAGE.into()),
    }
}

fn open(path: &Path) -> Result<crate::PatchLog, String> {
    format::load_kabl(path)
        .map_err(|e| format!("{}: not a readable .kabl file ({e:?})", path.display()))
}

fn cat(file: &Path, out: &mut dyn Write) -> Result<(), String> {
    let log = open(file)?;
    for entry in log.entries() {
        let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        writeln!(out, "{line}").map_err(|e| format!("write failed: {e}"))?;
    }
    Ok(())
}

fn refuse_existing(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} already exists: not overwriting it",
            path.display()
        ));
    }
    Ok(())
}

fn pack(dir: &Path, out: Option<&Path>) -> Result<(), String> {
    let out: PathBuf = out.map_or_else(
        || {
            let mut s = dir.as_os_str().to_os_string();
            s.push(".kabl");
            PathBuf::from(s)
        },
        Path::to_path_buf,
    );
    refuse_existing(&out)?;
    // `sound.toml`, when the folder has one, goes into the file's metadata frame.
    format::pack_dir(dir, &out)
        .map_err(|e| format!("{}: not a readable patch folder ({e:?})", dir.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

fn unpack(file: &Path, dir: Option<&Path>) -> Result<(), String> {
    let (log, meta) = format::load_kabl_meta(file)
        .map_err(|e| format!("{}: not a readable .kabl file ({e:?})", file.display()))?;
    let dir: PathBuf = dir.map_or_else(|| file.with_extension(""), Path::to_path_buf);
    if dir.as_os_str().is_empty() || dir == file {
        return Err(format!(
            "{}: cannot pick a folder name, pass one",
            file.display()
        ));
    }
    refuse_existing(&dir)?;
    format::save(&dir, &log).map_err(|e| format!("{}: {e:?}", dir.display()))?;
    if let Some(meta) = meta {
        std::fs::write(dir.join("sound.toml"), meta)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    println!("wrote {}", dir.display());
    Ok(())
}
