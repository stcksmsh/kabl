//! `demo.bitfilter`: a stateful stereo voice filter (D11 example 2).
//!
//! Two cascaded RBJ low-pass biquads per channel (24 dB/oct), a soft-clip drive in front, and a
//! cutoff that follows `cutoff_cv` at one volt per octave (the block average). `resonance` sets
//! the Q of the first section. State is a version word plus the four sections' two delay values
//! per channel, flushed of denormals after every block. ABI v0: see
//! `crates/modules/src/code/abi.rs`.
#![no_std]

const N: usize = 2048;
const STATE_VERSION: u32 = 1;

/// kabl.meta custom section: line based text, see `parse_meta` in the host.
#[link_section = "kabl.meta"]
#[used]
static META: [u8; META_TEXT.len()] = bytes(META_TEXT);

const fn bytes<const L: usize>(s: &str) -> [u8; L] {
    let b = s.as_bytes();
    let mut o = [0; L];
    let mut i = 0;
    while i < L {
        o[i] = b[i];
        i += 1;
    }
    o
}
const META_TEXT: &str = "kind demo.bitfilter
version 1
name Bit Filter
explain A steep stereo low-pass with resonance and a soft-clip drive. The cutoff follows the cutoff input at one octave per unit.
category filter
rate voice
width 6
port in audio in 2
port in cv cutoff_cv 1
port out audio out 2
param cutoff 20 20000 2000 exp 5 Hz
param resonance 0 1 0.2 linear 5 amt
param drive 0 1 0 linear 5 amt
advanced drive
";

#[repr(C)]
struct Mem {
    /// left, right, cutoff cv
    input: [[f32; N]; 3],
    /// left, right
    output: [[f32; N]; 2],
    /// cutoff, resonance, drive (written by the host before each block)
    params: [f32; 3],
    state: State,
}

#[repr(C)]
struct State {
    version: u32,
    /// `z[channel][section] = [z1, z2]`
    z: [[[f32; 2]; 2]; 2],
}

const FRESH: State = State {
    version: STATE_VERSION,
    z: [[[0.0; 2]; 2]; 2],
};

static mut MEM: Mem = Mem {
    input: [[0.0; N]; 3],
    output: [[0.0; N]; 2],
    params: [2000.0, 0.2, 0.0],
    state: FRESH,
};
static mut LAYOUT: [u32; 9] = [0; 9];
static mut SAMPLE_RATE: f32 = 48_000.0;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

fn mem() -> &'static mut Mem {
    unsafe { &mut *core::ptr::addr_of_mut!(MEM) }
}

#[no_mangle]
pub extern "C" fn init(sample_rate: f32, _max_block: i32) -> i32 {
    unsafe { SAMPLE_RATE = sample_rate.clamp(8_000.0, 384_000.0) };
    let base = core::ptr::addr_of!(MEM) as u32;
    let off = |o: usize| base + o as u32;
    let lay = unsafe { &mut *core::ptr::addr_of_mut!(LAYOUT) };
    *lay = [
        off(core::mem::offset_of!(Mem, input)),
        off(core::mem::offset_of!(Mem, output)),
        off(core::mem::offset_of!(Mem, params)),
        off(core::mem::offset_of!(Mem, state)),
        core::mem::size_of::<State>() as u32,
        N as u32,
        3,
        2,
        3,
    ];
    core::ptr::addr_of!(LAYOUT) as i32
}

#[no_mangle]
pub extern "C" fn reset() {
    mem().state = FRESH;
}

/// 2^x for |x| <= 8 (degree-7 Taylor on the fraction).
fn exp2(x: f32) -> f32 {
    let x = x.clamp(-8.0, 8.0);
    let t = x as i32;
    let i = (if (t as f32) > x { t - 1 } else { t }) as f32;
    let f = (x - i) * core::f32::consts::LN_2;
    let p = 1.0
        + f * (1.0
            + f * (0.5
                + f * (1.0 / 6.0
                    + f * (1.0 / 24.0 + f * (1.0 / 120.0 + f * (1.0 / 720.0 + f / 5040.0))))));
    p * f32::from_bits(((i as i32 + 127) as u32) << 23)
}

/// sin and cos for 0 <= x <= pi (Taylor, error under 1e-4).
fn sin_cos(x: f32) -> (f32, f32) {
    let x2 = x * x;
    let s = x
        * (1.0
            - x2 / 6.0
                * (1.0
                    - x2 / 20.0
                        * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0 * (1.0 - x2 / 156.0))))));
    let c = 1.0
        - x2 / 2.0
            * (1.0
                - x2 / 12.0
                    * (1.0 - x2 / 30.0 * (1.0 - x2 / 56.0 * (1.0 - x2 / 90.0 * (1.0 - x2 / 132.0)))));
    (s, c)
}

/// Low-pass biquad coefficients `[b0, b1, b2, a1, a2]`, already divided by a0.
fn lowpass(fc: f32, q: f32, sr: f32) -> [f32; 5] {
    let w0 = core::f32::consts::TAU * fc / sr;
    let (sin, cos) = sin_cos(w0);
    let alpha = sin / (2.0 * q);
    let a0 = 1.0 + alpha;
    let b1 = (1.0 - cos) / a0;
    [b1 * 0.5, b1, b1 * 0.5, -2.0 * cos / a0, (1.0 - alpha) / a0]
}

fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

fn flush(v: f32) -> f32 {
    if v.abs() < 1e-20 {
        0.0
    } else {
        v
    }
}

#[no_mangle]
pub extern "C" fn process(n: i32) {
    let m = mem();
    let n = (n.max(0) as usize).min(N);
    if n == 0 {
        return;
    }
    let sr = unsafe { SAMPLE_RATE };
    let [cutoff, resonance, drive] = m.params;
    let mut cv = 0.0;
    for i in 0..n {
        cv += m.input[2][i];
    }
    let fc = (cutoff * exp2(cv / n as f32)).clamp(5.0, 0.45 * sr);
    let q1 = 0.7071 + resonance.clamp(0.0, 1.0) * 9.0;
    let sections = [lowpass(fc, q1, sr), lowpass(fc, 0.7071, sr)];
    let pre = 1.0 + 3.0 * drive.clamp(0.0, 1.0);
    let mix = drive.clamp(0.0, 1.0);
    for ch in 0..2 {
        let mut z = m.state.z[ch];
        for i in 0..n {
            let dry = m.input[ch][i];
            let mut x = dry * (1.0 - mix) + soft(dry * pre) * mix * 1.5;
            for (s, c) in z.iter_mut().zip(&sections) {
                let y = c[0] * x + s[0];
                s[0] = c[1] * x - c[3] * y + s[1];
                s[1] = c[2] * x - c[4] * y;
                x = y;
            }
            m.output[ch][i] = x;
        }
        for s in z.iter_mut() {
            s[0] = flush(s[0]);
            s[1] = flush(s[1]);
        }
        m.state.z[ch] = z;
    }
}
