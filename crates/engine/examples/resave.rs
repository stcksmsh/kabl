//! Loads a patch directory and saves it again, which writes `checkpoint.json` (a generated
//! patch has only `log.jsonl` and `meta.toml`) and stamps the current schema version.
//!
//!     cargo run -p kabl-engine --example resave -- DIR [OUT]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::Path::new(&args[0]);
    let out = args.get(1).map_or(dir, |o| std::path::Path::new(o));
    let log = kabl_core::load(dir).expect("patch");
    kabl_core::save(out, &log).expect("save");
}
