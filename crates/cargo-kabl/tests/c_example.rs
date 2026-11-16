//! The one-page C module (`sdk/examples/c-gain`) loads in the real host engine and scales its
//! input. Needs `clang` and rustup's `rust-lld` (see that directory's `build.sh`); without them
//! the test says so and passes, so a machine with no C toolchain stays green.

use kabl_modules::code::{InstanceConfig, WasmEngine, WasmiEngine};
use std::path::PathBuf;
use std::process::Command;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/examples/c-gain")
}

#[test]
fn the_c_example_loads_and_scales_its_input() {
    let dir = example_dir();
    let built = Command::new("sh").arg(dir.join("build.sh")).output();
    let wasm = match built {
        Ok(o) if o.status.success() => std::fs::read(dir.join("c-gain.wasm")).unwrap(),
        _ => {
            eprintln!("skipped: no clang or rust-lld to build sdk/examples/c-gain");
            return;
        }
    };

    let compiled = WasmiEngine::new()
        .compile(&wasm)
        .expect("host loads the module");
    assert_eq!(compiled.meta().kind, "example.cgain");
    let mut inst = compiled
        .instantiate(&InstanceConfig::new(48_000.0, 2048))
        .expect("instantiates");
    let layout = inst.layout();
    let mem = inst.memory();
    for (i, v) in [1.0f32, -0.5, 0.25, 0.0].iter().enumerate() {
        let at = layout.input as usize + i * 4;
        mem[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    let p = layout.params as usize;
    mem[p..p + 4].copy_from_slice(&2.0f32.to_le_bytes());
    inst.set_fuel(10_000_000);
    inst.call_block(4).expect("process runs");
    let mem = inst.memory();
    let out: Vec<f32> = (0..4)
        .map(|i| {
            let at = layout.output as usize + i * 4;
            f32::from_le_bytes(mem[at..at + 4].try_into().unwrap())
        })
        .collect();
    assert_eq!(out, [2.0, -1.0, 0.5, 0.0]);
}
