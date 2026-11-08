//! Host-free tests of the runtime behind `kabl_module!`: they call the exports' bodies directly.

use super::*;
use crate::{Category, Meta, PortType, Rate, StateReader, Taper};

/// Test module: 2 in slots (one stereo port), 1 out slot, a gain param, a counter as state.
struct T {
    count: u32,
}

impl Module for T {
    const META: Meta = Meta::new("test.t", 1)
        .name("T")
        .category(Category::Utility)
        .rate(Rate::Voice)
        .port_in(PortType::Audio, "in", 2)
        .port_out(PortType::Audio, "out", 1)
        .param("gain", "0", "4", "1", Taper::Linear, 5, "x");
    const STATE_VERSION: u32 = 7;
    const STATE_CAP: usize = 8;
    fn new(init: &mut Init) -> Self {
        T {
            count: init.max_block as u32,
        }
    }
    fn reset(&mut self) {
        self.count = 0;
    }
    fn process(&mut self, io: &mut Io) {
        let g = io.param(0);
        for i in 0..io.n {
            io.output[0][i] = (io.input[0][i] + io.input[1][i]) * g;
        }
        self.count += io.n as u32;
    }
    fn save(&self, w: &mut StateWriter) {
        w.u32(self.count);
        w.u32(!self.count);
        if self.count == 99 {
            w.u32(0); // 12 bytes into an 8-byte buffer
        }
    }
    fn load(&mut self, version: u32, bytes: &[u8]) -> bool {
        let mut r = StateReader::new(bytes);
        match (version, r.u32(), r.u32()) {
            (7, Some(a), Some(b)) if b == !a => {
                self.count = a;
                true
            }
            _ => false,
        }
    }
}

type MT = Mem<T, 2, 1, 1, 8, 0>;

// Each test gets its own static so they can run in parallel.
macro_rules! store {
    () => {{
        static S: Store<MT> = MT::store();
        &S
    }};
}

fn peek(s: &Store<MT>) -> u32 {
    unsafe { (*addr_of!((*s.ptr()).module)).assume_init_ref().count }
}

#[test]
fn init_reports_layout_and_slot_counts_from_the_builder() {
    let s = store!();
    // The returned address is a wasm32 pointer; on a 64-bit native test it is truncated, so
    // the words are read from the field instead.
    let ret = MT::init(s, 48_000.0, 64);
    let p = s.ptr();
    let w: [u32; 9] = unsafe { (*p).layout };
    assert_eq!(ret as u32, unsafe { addr_of!((*p).layout) } as usize as u32);
    let base = unsafe { addr_of!((*p).input) } as usize as u32;
    assert_eq!(w[0], base);
    assert_eq!(w[1] - w[0], 2 * MAX_BLOCK as u32 * 4);
    assert_eq!(w[2] - w[1], MAX_BLOCK as u32 * 4);
    assert_eq!(&w[5..], &[MAX_BLOCK as u32, 2, 1, 1]);
    assert_eq!(w[4] as usize, size_of::<T>());
    assert_eq!(peek(s), 64, "max_block reached new()");
}

#[test]
fn process_reads_inputs_params_and_writes_outputs() {
    let s = store!();
    MT::init(s, 48_000.0, 64);
    // Test-only: the host writes these bytes through linear memory.
    let m = unsafe { &mut *s.ptr() };
    m.input[0][..4].copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
    m.input[1][..4].copy_from_slice(&[10.0; 4]);
    m.params[0] = 0.5;
    MT::process(s, 4);
    let m = unsafe { &*s.ptr() };
    assert_eq!(&m.output[0][..4], &[5.5, 6.0, 6.5, 7.0]);
    assert_eq!(m.output[0][4], 0.0, "frames past n untouched");
    MT::process(s, -3);
    MT::process(s, i32::MAX); // clamped to MAX_BLOCK, no panic
    MT::reset(s);
    assert_eq!(peek(s), 0);
}

#[test]
fn state_round_trip_through_the_transfer_buffer() {
    let s = store!();
    MT::init(s, 48_000.0, 64);
    MT::process(s, 10);
    let n = MT::state_save(s);
    assert_eq!(n, 8);
    let p = s.ptr();
    let saved: [u8; 8] = unsafe { (*p).xfer };
    MT::process(s, 5);
    assert_eq!(peek(s), 79);
    // the host copies its stored bytes back into the buffer, then calls load
    unsafe { (*p).xfer = saved };
    assert_eq!(MT::state_load(s, n, 7), 1);
    assert_eq!(peek(s), 74);
    assert_eq!(MT::state_save(s), n);
    assert_eq!(
        unsafe { (*p).xfer },
        saved,
        "save after load is byte-identical"
    );
}

#[test]
fn load_rejections_return_zero_and_leave_the_module_alone() {
    let s = store!();
    MT::init(s, 48_000.0, 64);
    MT::state_save(s);
    assert_eq!(MT::state_load(s, 8, 6), 0, "wrong version");
    assert_eq!(MT::state_load(s, 4, 7), 0, "short data");
    assert_eq!(MT::state_load(s, 9, 7), 0, "longer than the buffer");
    assert_eq!(MT::state_load(s, -1, 7), 0);
    assert_eq!(peek(s), 64);
}

#[test]
fn save_overflow_returns_minus_one_and_is_not_truncated() {
    let s = store!();
    MT::init(s, 48_000.0, 64);
    MT::process(s, 35); // count 99: save writes 12 bytes into 8
    assert_eq!(peek(s), 99);
    assert_eq!(MT::state_save(s), -1);
}

#[test]
fn calls_before_init_are_inert() {
    let s = store!();
    MT::process(s, 8);
    MT::reset(s);
    assert_eq!(MT::state_save(s), -1);
    assert_eq!(MT::state_load(s, 0, 7), 0);
}

#[test]
fn layout_ext_table_is_count_xfer_cap_version() {
    let s = store!();
    let ret = MT::layout_ext(s);
    let p = s.ptr();
    let w: [u32; 4] = unsafe { (*p).ext };
    assert_eq!(ret as u32, unsafe { addr_of!((*p).ext) } as usize as u32);
    assert_eq!(w[0], 3);
    assert_eq!(w[1], unsafe { addr_of!((*p).xfer) } as usize as u32);
    assert_eq!((w[2], w[3]), (8, 7));
}

#[test]
fn the_const_store_is_all_zero_bytes() {
    // A const initialiser of all-zero bytes is what lets the linker put it in .bss; the wasm
    // test checks the real data section.
    let s: Store<MT> = MT::store();
    let bytes = unsafe { core::slice::from_raw_parts(s.ptr() as *const u8, size_of::<MT>()) };
    assert!(bytes.iter().all(|b| *b == 0));
}
