//! `demo.logic`: a control-rate logic module (D11 example 1). Global, scalar-ish signals.
//!
//! Inputs `a`, `b` (CV), `gate`; params `mode`, `threshold`, `offset`; outputs `out` (CV) and
//! `trig` (gate). While `gate` is high the output holds its last value (a latch); otherwise it
//! follows the selected function of `a` and `b`. State is one float (the latched value) behind a
//! version word. ABI v0: see `crates/modules/src/code/abi.rs`.
#![no_std]

const N: usize = 2048;

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
const META_TEXT: &str = "kind demo.logic
version 1
name Logic
explain Combines two control signals: min, max, AND, XOR or a threshold test. Hold the gate to freeze the output.
category utility
rate global
width 4
port in cv a 1
port in cv b 1
port in gate gate 1
port out cv out 1
port out gate trig 1
param mode 0 4 0 stepped 0 mode
param threshold -1 1 0.5 linear 5 cv
param offset -1 1 0 linear 5 cv
";

const STATE_VERSION: u32 = 1;

#[repr(C)]
struct Mem {
    input: [[f32; N]; 3],
    output: [[f32; N]; 2],
    /// mode, threshold, offset (written by the host before each block)
    params: [f32; 3],
    state: State,
}

#[repr(C)]
struct State {
    version: u32,
    latch: f32,
}

static mut MEM: Mem = Mem {
    input: [[0.0; N]; 3],
    output: [[0.0; N]; 2],
    params: [0.0; 3],
    state: State {
        version: STATE_VERSION,
        latch: 0.0,
    },
};

static mut LAYOUT: [u32; 9] = [0; 9];

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

fn mem() -> &'static mut Mem {
    unsafe { &mut *core::ptr::addr_of_mut!(MEM) }
}

#[no_mangle]
pub extern "C" fn init(_sample_rate: f32, _max_block: i32) -> i32 {
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
    let s = &mut mem().state;
    s.version = STATE_VERSION;
    s.latch = 0.0;
}

#[no_mangle]
pub extern "C" fn process(n: i32) {
    let m = mem();
    let n = (n.max(0) as usize).min(N);
    let mode = (m.params[0] + 0.5) as i32;
    let thr = m.params[1];
    let offset = m.params[2];
    let mut latch = m.state.latch;
    for i in 0..n {
        let a = m.input[0][i];
        let b = m.input[1][i];
        let held = m.input[2][i] > 0.5;
        let value = match mode {
            0 => a.min(b),
            1 => a.max(b),
            2 => f32::from(a > thr && b > thr),
            3 => f32::from((a > thr) != (b > thr)),
            _ => f32::from(a > thr),
        };
        if !held {
            latch = value + offset;
        }
        m.output[0][i] = latch;
        m.output[1][i] = f32::from(latch > thr);
    }
    m.state.latch = latch;
}
