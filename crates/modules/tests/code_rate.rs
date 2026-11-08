//! A registered code module is built for the running device's sample rate: `init` receives it,
//! also when the device rate changes after the module was registered (`--patch` registers before
//! the device is opened).

use kabl_modules::code::{self, host};
use kabl_modules::{Module, ProcessIo, QualityConfig, QualityTier, Signal};

const KIND: &str = "test.rate";
const N: usize = 64;

/// `init` stores its sample rate; every output sample is that rate divided by 1000.
fn fixture() -> Vec<u8> {
    let meta =
        "kind test.rate\\nversion 1\\nname Rate\\nexplain Test fixture.\\ncategory utility\\n\
rate voice\\nwidth 4\\nport in audio in 1\\nport out audio out 1\\n";
    wat::parse_str(format!(
        r#"(module
 (@custom "kabl.meta" "{meta}")
 (memory (export "memory") 1)
 (func (export "init") (param $sr f32) (param i32) (result i32)
  (f32.store (i32.const 16452) (f32.div (local.get $sr) (f32.const 1000)))
  (i32.store (i32.const 0) (i32.const 64)) (i32.store (i32.const 4) (i32.const 8256))
  (i32.store (i32.const 8) (i32.const 16448)) (i32.store (i32.const 12) (i32.const 16452))
  (i32.store (i32.const 16) (i32.const 4)) (i32.store (i32.const 20) (i32.const 2048))
  (i32.store (i32.const 24) (i32.const 1)) (i32.store (i32.const 28) (i32.const 1))
  (i32.store (i32.const 32) (i32.const 0))
  (i32.const 0))
 (func (export "reset"))
 (func (export "process") (param $n i32) (local $i i32)
  (block $done (loop $l
   (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
   (f32.store (i32.add (i32.const 8256) (i32.shl (local.get $i) (i32.const 2)))
              (f32.load (i32.const 16452)))
   (local.set $i (i32.add (local.get $i) (i32.const 1)))
   (br $l))))
)"#
    ))
    .unwrap()
}

fn render() -> Vec<f32> {
    let mut m = code::group(KIND).unwrap().voice().unwrap();
    m.prepare(
        96_000.0,
        N,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let input = [0.0f32; N];
    let sig = [Signal::Buffer(&input)];
    let mut out = [0f32; N];
    {
        let mut outs: [&mut [f32]; 1] = [&mut out];
        let mut io = ProcessIo::new(&sig, &mut outs, &[], N);
        m.process(&mut io);
    }
    out.to_vec()
}

#[test]
fn switching_the_device_rate_gives_the_same_output_as_registering_at_that_rate() {
    // Created at 96 kHz.
    host::device_changed(96_000.0, 64);
    host::register_blob(&fixture()).unwrap();
    let at_96k = render();
    assert_eq!(at_96k[0], 96.0);

    // Registered at 96 kHz, then the device moves to 44.1 kHz and back.
    host::device_changed(44_100.0, 64);
    let at_44k = render();
    assert_eq!(at_44k[0], 44.1);
    host::device_changed(96_000.0, 64);
    let back = render();
    assert_eq!(
        back.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        at_96k.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    assert_ne!(at_44k, at_96k);
}
