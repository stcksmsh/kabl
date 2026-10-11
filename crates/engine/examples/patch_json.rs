//! Prints a patch folder's state as JSON (the `patch` field of a plugin state chunk).
//!
//!     cargo run --release -p kabl-engine --example patch_json -- scratch/effects-demos/eq-wet
fn main() {
    let dir = std::env::args().nth(1).expect("patch folder");
    let log = kabl_core::load(std::path::Path::new(&dir)).expect("patch");
    println!("{}", serde_json::to_string(log.state()).unwrap());
}
