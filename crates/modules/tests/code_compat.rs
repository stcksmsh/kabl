//! The start of the compatibility corpus (D12): the committed example blobs, built by the SDK,
//! must still load in the real engine and render the same bytes. Before the 1.0 freeze this is a
//! regression check that may be regenerated (rebuild the blobs, run the test with
//! `KABL_PRINT_GOLDEN=1 -- --nocapture`, paste the new values); from 1.0 the corpus binds:
//! blobs built by past SDK versions must keep loading and rendering bit-exact.

use kabl_modules::code::state::{save, Restored};
use kabl_modules::code::{host, InstanceConfig, WasmEngine, WasmiEngine};
use kabl_modules::{registry, PortDirection, ProcessIo, QualityConfig, QualityTier, Signal};

const SR: f32 = 48_000.0;
const N: usize = 64;

/// FNV-1a over the bit patterns of every output sample of 12 blocks.
fn render_hash(kind: &str) -> u64 {
    host::register_examples().unwrap();
    let info = registry::info_for(kind).unwrap();
    let width = |d| {
        info.ports
            .iter()
            .filter(|p| p.direction == d)
            .map(|p| p.width as usize)
            .sum::<usize>()
    };
    let (n_in, n_out) = (width(PortDirection::Input), width(PortDirection::Output));
    let mut m = registry::create(kind).unwrap();
    m.prepare(
        SR,
        N,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let params: Vec<f32> = info.params.iter().map(|p| p.default).collect();
    let mut h = 0xcbf29ce484222325u64;
    let mut seed = 7u32;
    for _ in 0..12 {
        let ins: Vec<Vec<f32>> = (0..n_in)
            .map(|_| {
                (0..N)
                    .map(|_| {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
                    })
                    .collect()
            })
            .collect();
        let sig: Vec<Signal> = ins.iter().map(|v| Signal::Buffer(v)).collect();
        let ps: Vec<Signal> = params.iter().map(|&v| Signal::Scalar(v)).collect();
        let mut outs: Vec<Vec<f32>> = vec![vec![0.0; N]; n_out];
        {
            let mut refs: Vec<&mut [f32]> = outs.iter_mut().map(|v| &mut v[..]).collect();
            let mut io = ProcessIo::new(&sig, &mut refs, &ps, N);
            m.process(&mut io);
        }
        for v in outs.iter().flatten() {
            for b in v.to_bits().to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

/// `(example index, kind, version, golden render hash)`
const CORPUS: [(usize, &str, u32, u64); 2] = [
    (0, "demo.logic", 1, 0xcc8df63a05021ce7),
    (1, "demo.bitfilter", 1, 0x57d8dc69510e4d77),
];

#[test]
fn the_example_blobs_load_and_render_as_they_did() {
    let print = std::env::var_os("KABL_PRINT_GOLDEN").is_some();
    for (i, kind, version, golden) in CORPUS {
        let wasm = host::EXAMPLES[i].wasm;
        let c = WasmiEngine::new().compile(wasm).expect("loads");
        assert_eq!((c.meta().kind.as_str(), c.meta().version), (kind, version));
        // a bare instance saves, and its own settings load back as they were
        let inst = c.instantiate(&InstanceConfig::new(SR, 2048)).unwrap();
        assert!(inst.state_xfer().is_some(), "{kind} exports its settings");
        let mut inst = inst;
        let saved = save(&mut *inst).unwrap().unwrap();
        let g = kabl_modules::code::CodeGroup::new(
            c,
            InstanceConfig::new(SR, 2048),
            1_000_000,
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(g.restore(&saved).unwrap().1, Restored::Loaded);
        let got = render_hash(kind);
        if print {
            eprintln!("({i}, \"{kind}\", {version}, {got:#x}),");
        } else {
            assert_eq!(
                got, golden,
                "{kind} renders differently; see the module docs"
            );
        }
    }
}
