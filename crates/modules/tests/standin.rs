//! Stand-ins (D11): audio passes through in order, every other output is silent, the ports and
//! params come from the module's metadata, and the audio path allocates nothing.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_modules::code::{host, standin};
use kabl_modules::code::{CodeMeta, CodeParam, CodePort};
use kabl_modules::info::{Category, PortDirection, PortType, Rate, Taper};
use kabl_modules::{registry, ProcessIo, QualityConfig, QualityTier, Signal};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

fn port(name: &str, ty: PortType, dir: PortDirection, width: u8) -> CodePort {
    CodePort {
        name: name.into(),
        port_type: ty,
        direction: dir,
        width,
    }
}

#[test]
fn audio_passes_through_in_order_and_everything_else_is_silent() {
    use PortDirection::{Input, Output};
    let meta = CodeMeta {
        kind: "test.standin.shape".into(),
        version: 2,
        name: "Shape".into(),
        explain: String::new(),
        category: Category::Effect,
        rate: Rate::Voice,
        ports: vec![
            port("cv_in", PortType::Cv, Input, 1),
            port("in", PortType::Audio, Input, 2),
            port("cv_out", PortType::Cv, Output, 1),
            port("out", PortType::Audio, Output, 2),
            port("aux", PortType::Audio, Output, 1),
        ],
        params: vec![CodeParam {
            name: "amount".into(),
            min: 0.0,
            max: 1.0,
            default: 0.5,
            unit: String::new(),
            taper: Taper::Linear,
            smoothing_ms: 0.0,
        }],
        advanced: vec![],
        width_units: 4,
    };
    let r = host::register_stand_in("test.standin.shape", 2, None, "test", Some(meta));
    assert_eq!((r.kind.as_str(), r.version), ("test.standin.shape", 2));
    assert!(r.to_string().contains("test.standin.shape v2"));

    // Ports and params are those of the metadata.
    let info = registry::info_for("test.standin.shape").unwrap();
    assert_eq!(info.params[0].name, "amount");
    assert_eq!(info.slots(Input), 3);
    assert_eq!(info.slots(Output), 4);

    let mut m = registry::create("test.standin.shape").unwrap();
    m.prepare(
        48000.0,
        64,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let cv = [0.7f32; 64];
    let l: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
    let rr: Vec<f32> = (0..64).map(|i| -(i as f32) * 0.02).collect();
    let ins = [Signal::Buffer(&cv), Signal::Buffer(&l), Signal::Buffer(&rr)];
    let params = [Signal::Scalar(0.5)];
    let mut bufs = [[9.0f32; 64]; 4];
    let [b0, b1, b2, b3] = &mut bufs;
    let mut outs: [&mut [f32]; 4] = [&mut b0[..], &mut b1[..], &mut b2[..], &mut b3[..]];
    assert_no_alloc(|| {
        let mut io = ProcessIo::new(&ins, &mut outs, &params, 64);
        m.process(&mut io);
    });
    assert!(bufs[0].iter().all(|v| *v == 0.0), "cv out is silent");
    assert_eq!(
        bufs[1].map(f32::to_bits),
        <[f32; 64]>::try_from(l).unwrap().map(f32::to_bits)
    );
    assert_eq!(
        bufs[2].map(f32::to_bits),
        <[f32; 64]>::try_from(rr).unwrap().map(f32::to_bits)
    );
    assert!(
        bufs[3].iter().all(|v| *v == 0.0),
        "extra audio out is silent"
    );
    assert_eq!(
        standin::reports()
            .iter()
            .filter(|r| r.kind == "test.standin.shape")
            .count(),
        1
    );
}
