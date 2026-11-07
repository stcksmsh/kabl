//! Records the git commit for the About box (`about::version_line`). Outside a git checkout
//! (a source tarball) the commit is "unavailable".
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

    // The factory sounds are folders in git; the app reads only `.kabl` files. Pack them (with
    // the code behind `kabl pack-tree`) so a development build and the tests find them.
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("factory");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let src = std::path::Path::new("../../patches");
    if src.is_dir() {
        kabl_core::format::pack_tree(src, &out).expect("pack the factory sounds");
    }
    println!("cargo:rustc-env=KABL_FACTORY_BUILD={}", out.display());
    println!("cargo:rerun-if-changed=../../patches");
}
