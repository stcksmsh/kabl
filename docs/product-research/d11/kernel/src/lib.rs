//! D11 gate kernel: ABI-v0-shaped module. A transposed direct-form-II biquad with a
//! one-sample feedback loop around it (`x + fb * previous output`), 64 frames per call.
//! Buffers live in this module's linear memory; the host copies audio in and out.
#![no_std]

const N: usize = 64;

#[repr(C)]
struct Mem {
    input: [f32; N],
    output: [f32; N],
    /// b0, b1, b2, a1, a2, fb (written by the host before each block)
    params: [f32; 6],
    /// biquad z1, z2 and the previous output (the one-sample feedback)
    state: [f32; 3],
}

static mut MEM: Mem = Mem { input: [0.0; N], output: [0.0; N], params: [0.0; 6], state: [0.0; 3] };

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// Returns the address of the `Mem` block: input, then output, params, state.
#[no_mangle]
pub extern "C" fn init(_sample_rate: f32, _max_block: i32) -> i32 {
    core::ptr::addr_of!(MEM) as i32
}

#[no_mangle]
pub extern "C" fn reset() {
    unsafe { (*core::ptr::addr_of_mut!(MEM)).state = [0.0; 3] };
}

#[no_mangle]
pub extern "C" fn process(n: i32) {
    let m = unsafe { &mut *core::ptr::addr_of_mut!(MEM) };
    let [b0, b1, b2, a1, a2, fb] = m.params;
    let [mut z1, mut z2, mut prev] = m.state;
    let n = (n as usize).min(N);
    for i in 0..n {
        let x = m.input[i] + fb * prev;
        let y = b0 * x + z1;
        z1 = b1 * x - a1 * y + z2;
        z2 = b2 * x - a2 * y;
        m.output[i] = y;
        prev = y;
    }
    m.state = [z1, z2, prev];
}

/// Deliberately spins forever (used to confirm fuel stops it).
#[no_mangle]
pub extern "C" fn spin() {
    loop {
        core::hint::spin_loop();
    }
}
