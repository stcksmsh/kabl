//! Host-free tests of the meta builder, state helpers, math and (with the `arena` feature)
//! the bump arena.

use crate::*;

mod meta {
    use super::*;

    const M: Meta = Meta::new("acme.t", 3)
        .name("Test Thing")
        .explain("Does a thing.")
        .category(Category::Effect)
        .rate(Rate::Global)
        .width(5)
        .port_in(PortType::Audio, "in", 2)
        .port_in(PortType::Cv, "mod", 1)
        .port_out(PortType::Gate, "g", 1)
        .param("a", "0", "1", "0.5", Taper::Linear, 5, "")
        .param("b", "-1", "1", "0", Taper::Exp, 0, "Hz")
        .advanced("b");

    #[test]
    fn text_matches_the_host_format_exactly() {
        assert_eq!(
            M.text(),
            "kind acme.t\nversion 3\nabi_minor 1\nname Test Thing\nexplain Does a thing.\n\
             category effect\nrate global\nwidth 5\nport in audio in 2\nport in cv mod 1\n\
             port out gate g 1\nparam a 0 1 0.5 linear 5\nparam b -1 1 0 exp 0 Hz\nadvanced b\n"
        );
    }

    #[test]
    fn counts_are_slots_not_ports() {
        assert_eq!((M.in_slots, M.out_slots, M.n_params), (3, 1, 2));
    }

    #[test]
    fn bytes_are_the_text() {
        let b: [u8; M.len] = M.bytes();
        assert_eq!(&b[..], M.text().as_bytes());
    }

    #[test]
    fn notices_carry_the_mit_text() {
        assert!(NOTICE.contains("Permission is hereby granted"));
        let b: [u8; NOTICE.len()] = str_bytes(NOTICE);
        assert_eq!(&b[..], NOTICE.as_bytes());
    }

    #[test]
    fn port_index_checks_accept_the_right_slots() {
        M.assert_in(0, "in");
        M.assert_in(2, "mod"); // after a width-2 port
        M.assert_out(0, "g");
        M.assert_param(1, "b");
    }

    #[test]
    #[should_panic(expected = "different slot")]
    fn port_index_check_rejects_a_reorder() {
        M.assert_in(1, "mod");
    }

    #[test]
    #[should_panic(expected = "no such input port")]
    fn port_index_check_rejects_a_rename() {
        M.assert_in(0, "input");
    }

    #[test]
    #[should_panic(expected = "different index")]
    fn param_index_check_rejects_a_reorder() {
        M.assert_param(0, "b");
    }

    #[test]
    #[should_panic(expected = "spaces")]
    fn names_cannot_hold_spaces() {
        let _ = Meta::new("a.b", 1).param("two words", "0", "1", "0", Taper::Linear, 0, "");
    }

    #[test]
    #[should_panic(expected = "not a decimal number")]
    fn numbers_are_checked() {
        let _ = Meta::new("a.b", 1).param("p", "2OOO", "1", "0", Taper::Linear, 0, "");
    }

    #[test]
    #[should_panic(expected = "too long")]
    fn overlong_text_is_refused() {
        let long = std::string::String::from_utf8(std::vec![b'x'; META_CAP]).unwrap();
        let _ = Meta::new("a.b", 1).explain(&long);
    }
}

mod state {
    use super::*;

    #[test]
    fn writer_and_reader_round_trip() {
        let mut buf = [0u8; 12];
        let mut w = StateWriter::new(&mut buf);
        w.u32(7);
        w.f32(-1.5);
        w.bytes(&[1, 2, 3, 4]);
        assert_eq!((w.len(), w.overflowed()), (12, false));
        let mut r = StateReader::new(&buf);
        assert_eq!((r.u32(), r.f32()), (Some(7), Some(-1.5)));
        assert_eq!(r.bytes(4), Some(&[1u8, 2, 3, 4][..]));
        assert_eq!((r.remaining(), r.u32()), (0, None));
    }

    #[test]
    fn overflow_is_sticky_and_writes_nothing_partial() {
        let mut buf = [0u8; 6];
        let mut w = StateWriter::new(&mut buf);
        w.u32(1);
        w.u32(2); // does not fit
        w.bytes(&[9]); // would fit, but the writer already failed
        assert!(w.overflowed());
        assert_eq!(w.len(), 4);
        assert_eq!(buf, [1, 0, 0, 0, 0, 0]);
    }
}

mod maths {
    use super::*;

    #[test]
    fn exp2_and_sin_cos_are_close() {
        for k in -16..=16 {
            let x = k as f32 * 0.5;
            assert!((exp2(x) / 2f32.powf(x) - 1.0).abs() < 1e-4, "exp2({x})");
        }
        for k in 0..=31 {
            let x = k as f32 * 0.1;
            let (s, c) = sin_cos(x);
            assert!(
                (s - x.sin()).abs() < 1e-4 && (c - x.cos()).abs() < 1e-4,
                "{x}"
            );
        }
        assert_eq!(flush(1e-30), 0.0);
        assert_eq!(flush(-0.5), -0.5);
    }
}

#[cfg(feature = "arena")]
mod arena {
    use super::*;

    fn arena(n: usize) -> Bump {
        Bump::new(std::boxed::Box::leak(
            std::vec![0xAAu8; n].into_boxed_slice(),
        ))
    }

    #[test]
    fn allocations_are_zeroed_disjoint_and_aligned() {
        let mut a = arena(64);
        let x = a.alloc_slice::<u8>(3).unwrap();
        let y = a.alloc_slice::<f32>(4).unwrap();
        assert!(x.iter().all(|v| *v == 0), "arena is zeroed on creation");
        assert!(y.iter().all(|v| *v == 0.0));
        assert_eq!(y.as_ptr() as usize % 4, 0);
        x.fill(1);
        y.fill(2.0);
        assert!(x.iter().all(|v| *v == 1), "writes to y did not touch x");
    }

    #[test]
    fn exhaustion_returns_none_and_exact_fit_works() {
        let mut a = arena(16);
        assert!(a.alloc_slice::<f32>(5).is_none());
        assert!(a.alloc_slice::<f32>(4).is_some());
        assert_eq!(a.remaining(), 0);
        assert!(a.alloc_slice::<u8>(1).is_none());
        assert!(a.alloc_slice::<u8>(0).is_some());
    }

    #[test]
    fn size_overflow_is_none_not_a_panic() {
        let mut a = arena(16);
        assert!(a.alloc_slice::<f64>(usize::MAX).is_none());
    }

    #[test]
    fn a_module_with_an_arena_gets_it_in_new() {
        use crate::rt::{Mem, Store};
        struct A(&'static mut [f32]);
        impl Module for A {
            const META: Meta = Meta::new("test.a", 1);
            const ARENA_BYTES: usize = 64;
            fn new(init: &mut Init) -> Self {
                A(init.arena.alloc_slice(16).unwrap())
            }
            fn reset(&mut self) {}
            fn process(&mut self, _: &mut Io) {
                self.0[15] += 1.0;
            }
        }
        type MA = Mem<A, 0, 0, 0, 0, 64>;
        static S: Store<MA> = MA::store();
        MA::init(&S, 48_000.0, 64);
        MA::process(&S, 1);
        MA::process(&S, 1);
    }
}
