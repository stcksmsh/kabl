//! Records the git commit for the log's build identity (`applog::build_id`). Outside a git
//! checkout (a source tarball) the commit is "unavailable".
fn main() {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output();
    if let Some(o) = out.ok().filter(|o| o.status.success()) {
        let dirty = std::process::Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=no"])
            .output()
            .is_ok_and(|s| !s.stdout.is_empty());
        let rev = String::from_utf8_lossy(&o.stdout).trim().to_string();
        println!(
            "cargo:rustc-env=KABL_COMMIT={rev}{}",
            if dirty { "+modified" } else { "" }
        );
    }
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
}
