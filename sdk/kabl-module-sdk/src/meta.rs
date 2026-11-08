//! Compile-time builder for the `kabl.meta` custom section.
//!
//! Everything here is `const fn`: the text is built by rustc, so a module cannot disagree with
//! its own code, and a malformed token (a space in a name, an overlong section) is a compile
//! error. Float limits stay text (`"0.5"`) because there is no const float formatting, so a typo
//! like `"2OOO"` is caught by the host parser, not by rustc.
//!
//! ```
//! use kabl_module_sdk::{Category, Meta, PortType, Rate, Taper};
//! const META: Meta = Meta::new("acme.gain", 1)
//!     .name("Gain")
//!     .category(Category::Utility)
//!     .rate(Rate::Voice)
//!     .port_in(PortType::Audio, "in", 1)
//!     .port_out(PortType::Audio, "out", 1)
//!     .param("gain", "0", "4", "1", Taper::Linear, 5, "x");
//! assert_eq!(META.in_slots, 1);
//! assert!(META.text().contains("param gain 0 4 1 linear 5 x\n"));
//! ```

/// Minor revision of the module ABI this SDK targets. Emitted as `abi_minor`; a host with a
/// smaller minor refuses the module with a "rebuild" message.
pub const ABI_MINOR: u32 = 1;

/// Longest meta text the builder accepts, in bytes.
pub const META_CAP: usize = 2048;
const MAX_PORTS: usize = 16;
const MAX_PARAMS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Source,
    Filter,
    Modulator,
    Sequencer,
    Utility,
    Effect,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rate {
    Voice,
    Global,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortType {
    Audio,
    Cv,
    Unipolar,
    Gate,
    Pitch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Taper {
    Linear,
    Exp,
    Stepped,
    Cubic,
}

/// A `(start, len)` byte range of a name inside the text, plus the first io slot it occupies.
#[derive(Clone, Copy)]
struct Entry {
    start: usize,
    len: usize,
    slot: usize,
}

const NONE: Entry = Entry {
    start: 0,
    len: 0,
    slot: 0,
};

/// The meta text plus the slot tables used by the port-index const check.
#[derive(Clone, Copy)]
pub struct Meta {
    buf: [u8; META_CAP],
    /// Length of the text in bytes.
    pub len: usize,
    /// Number of input io slots (a width-2 port takes two).
    pub in_slots: usize,
    /// Number of output io slots.
    pub out_slots: usize,
    pub n_params: usize,
    n_in: usize,
    n_out: usize,
    ins: [Entry; MAX_PORTS],
    outs: [Entry; MAX_PORTS],
    params: [Entry; MAX_PARAMS],
}

impl Meta {
    /// Starts a description of module `kind` (`githubhandle.module`) at `version` (1, 2, ...).
    pub const fn new(kind: &str, version: u32) -> Self {
        let mut m = Meta {
            buf: [0; META_CAP],
            len: 0,
            in_slots: 0,
            out_slots: 0,
            n_params: 0,
            n_in: 0,
            n_out: 0,
            ins: [NONE; MAX_PORTS],
            outs: [NONE; MAX_PORTS],
            params: [NONE; MAX_PARAMS],
        };
        m.key("kind");
        m.token(kind);
        m.nl();
        m.key("version");
        m.num(version);
        m.nl();
        m.key("abi_minor");
        m.num(ABI_MINOR);
        m.nl();
        m
    }

    /// Display name (rest of line).
    pub const fn name(mut self, v: &str) -> Self {
        self.key("name");
        self.text_field(v);
        self
    }

    /// One-sentence description shown in the browser (rest of line).
    pub const fn explain(mut self, v: &str) -> Self {
        self.key("explain");
        self.text_field(v);
        self
    }

    pub const fn category(mut self, c: Category) -> Self {
        self.key("category");
        self.raw(match c {
            Category::Source => "source",
            Category::Filter => "filter",
            Category::Modulator => "modulator",
            Category::Sequencer => "sequencer",
            Category::Utility => "utility",
            Category::Effect => "effect",
        });
        self.nl();
        self
    }

    pub const fn rate(mut self, r: Rate) -> Self {
        self.key("rate");
        self.raw(match r {
            Rate::Voice => "voice",
            Rate::Global => "global",
        });
        self.nl();
        self
    }

    /// Panel width in rack units.
    pub const fn width(mut self, units: u32) -> Self {
        self.key("width");
        self.num(units);
        self.nl();
        self
    }

    pub const fn port_in(mut self, ty: PortType, name: &str, width: u32) -> Self {
        assert!(self.n_in < MAX_PORTS, "too many input ports");
        self.ins[self.n_in] = Entry {
            start: self.len + 8 + port_type(ty).len() + 1,
            len: name.len(),
            slot: self.in_slots,
        };
        self.n_in += 1;
        self.in_slots += width as usize;
        self.port("in", ty, name, width);
        self
    }

    pub const fn port_out(mut self, ty: PortType, name: &str, width: u32) -> Self {
        assert!(self.n_out < MAX_PORTS, "too many output ports");
        self.outs[self.n_out] = Entry {
            start: self.len + 9 + port_type(ty).len() + 1,
            len: name.len(),
            slot: self.out_slots,
        };
        self.n_out += 1;
        self.out_slots += width as usize;
        self.port("out", ty, name, width);
        self
    }

    /// A parameter. `min`, `max` and `default` are decimal text (`"0.5"`); `unit` may be `""`.
    #[allow(clippy::too_many_arguments)] // one flat line per param is the readable form
    pub const fn param(
        mut self,
        name: &str,
        min: &str,
        max: &str,
        default: &str,
        taper: Taper,
        smoothing_ms: u32,
        unit: &str,
    ) -> Self {
        assert!(self.n_params < MAX_PARAMS, "too many params");
        self.key("param");
        self.params[self.n_params] = Entry {
            start: self.len,
            len: name.len(),
            slot: self.n_params,
        };
        self.n_params += 1;
        self.token(name);
        self.raw(" ");
        self.number_text(min);
        self.raw(" ");
        self.number_text(max);
        self.raw(" ");
        self.number_text(default);
        self.raw(" ");
        self.raw(match taper {
            Taper::Linear => "linear",
            Taper::Exp => "exp",
            Taper::Stepped => "stepped",
            Taper::Cubic => "cubic",
        });
        self.raw(" ");
        self.num(smoothing_ms);
        if !unit.is_empty() {
            self.raw(" ");
            self.text_field_inline(unit);
        }
        self.nl();
        self
    }

    /// Marks a parameter as advanced (hidden behind the panel's expander).
    pub const fn advanced(mut self, param: &str) -> Self {
        self.key("advanced");
        self.token(param);
        self.nl();
        self
    }

    /// The text written to `kabl.meta`.
    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// The text as a fixed array of exactly `L` bytes (`L` must equal `self.len`; the
    /// `kabl_module!` macro uses this to size the `kabl.meta` static).
    pub const fn bytes<const L: usize>(&self) -> [u8; L] {
        assert!(L == self.len, "meta length mismatch");
        let mut o = [0; L];
        let mut i = 0;
        while i < L {
            o[i] = self.buf[i];
            i += 1;
        }
        o
    }

    /// Compile error unless input port `name` starts at io slot `slot`.
    ///
    /// Creators write `const CUTOFF_CV: usize = 2;` and one line
    /// `const _: () = META.assert_in(CUTOFF_CV, "cutoff_cv");`, so renaming or reordering a
    /// port breaks the build instead of silently swapping signals.
    pub const fn assert_in(&self, slot: usize, name: &str) {
        let mut i = 0;
        while i < self.n_in {
            if self.is(&self.ins[i], name) {
                assert!(
                    self.ins[i].slot == slot,
                    "input port is at a different slot"
                );
                return;
            }
            i += 1;
        }
        panic!("no such input port");
    }

    /// As [`Meta::assert_in`], for output ports.
    pub const fn assert_out(&self, slot: usize, name: &str) {
        let mut i = 0;
        while i < self.n_out {
            if self.is(&self.outs[i], name) {
                assert!(
                    self.outs[i].slot == slot,
                    "output port is at a different slot"
                );
                return;
            }
            i += 1;
        }
        panic!("no such output port");
    }

    /// As [`Meta::assert_in`], for parameters (`index` is the position in declaration order).
    pub const fn assert_param(&self, index: usize, name: &str) {
        let mut i = 0;
        while i < self.n_params {
            if self.is(&self.params[i], name) {
                assert!(
                    self.params[i].slot == index,
                    "param is at a different index"
                );
                return;
            }
            i += 1;
        }
        panic!("no such param");
    }

    const fn is(&self, e: &Entry, name: &str) -> bool {
        let mut i = 0;
        let n = name.as_bytes();
        if e.len != n.len() {
            return false;
        }
        while i < e.len {
            if self.buf[e.start + i] != n[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    // ---- writers ----

    const fn port(&mut self, dir: &str, ty: PortType, name: &str, width: u32) {
        assert!(width == 1 || width == 2, "port width is 1 or 2");
        self.key("port");
        self.raw(dir);
        self.raw(" ");
        self.raw(port_type(ty));
        self.raw(" ");
        self.token(name);
        self.raw(" ");
        self.num(width);
        self.nl();
    }

    const fn raw(&mut self, s: &str) {
        let b = s.as_bytes();
        assert!(self.len + b.len() <= META_CAP, "meta text too long");
        let mut i = 0;
        while i < b.len() {
            self.buf[self.len + i] = b[i];
            i += 1;
        }
        self.len += b.len();
    }

    const fn key(&mut self, k: &str) {
        self.raw(k);
        self.raw(" ");
    }

    const fn nl(&mut self) {
        self.raw("\n");
    }

    /// A whitespace-free word.
    const fn token(&mut self, s: &str) {
        let b = s.as_bytes();
        assert!(!b.is_empty(), "empty name");
        let mut i = 0;
        while i < b.len() {
            assert!(
                b[i] > b' ' && b[i] != 0x7f,
                "names cannot hold spaces or control characters"
            );
            i += 1;
        }
        self.raw(s);
    }

    /// A decimal number written as text: digits, sign, point, exponent only.
    const fn number_text(&mut self, s: &str) {
        let b = s.as_bytes();
        assert!(!b.is_empty(), "empty number");
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            assert!(
                c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' || c == b'e' || c == b'E',
                "not a decimal number"
            );
            i += 1;
        }
        self.raw(s);
    }

    /// A free-text value with no line break.
    const fn text_field_inline(&mut self, s: &str) {
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            assert!(b[i] != b'\n' && b[i] != b'\r', "no line breaks in text");
            i += 1;
        }
        self.raw(s);
    }

    const fn text_field(&mut self, s: &str) {
        self.text_field_inline(s);
        self.nl();
    }

    const fn num(&mut self, mut v: u32) {
        let mut digits = [0u8; 10];
        let mut n = 0;
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            n += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        assert!(self.len + n <= META_CAP, "meta text too long");
        while n > 0 {
            n -= 1;
            self.buf[self.len] = digits[n];
            self.len += 1;
        }
    }
}

const fn port_type(t: PortType) -> &'static str {
    match t {
        PortType::Audio => "audio",
        PortType::Cv => "cv",
        PortType::Unipolar => "unipolar",
        PortType::Gate => "gate",
        PortType::Pitch => "pitch",
    }
}
