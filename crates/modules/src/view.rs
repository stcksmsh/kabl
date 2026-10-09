//! What an oscillator shows an interface: its current position in a table and one cycle of its
//! output. `Module::view` fills a fixed-size `ModuleView` from state the module already keeps
//! (a ring of its last output samples), so asking costs the audio thread a copy and never an
//! allocation or a lock. The engine hands it to the interface inside the signal-inspection
//! report (`kabl_engine::probe::ProbeReport::view`).

pub const VIEW_CYCLE: usize = 128;
const RING: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModuleView {
    /// The fields below describe a playing voice.
    pub valid: bool,
    /// `osc.wt`: where in its table the voice reads, 0..1 (position plus the `pos` input).
    /// 0 for sources without a table.
    pub position: f32,
    /// One period of the voice's output (resampled to `VIEW_CYCLE` points, starting at a rising
    /// zero crossing when the last periods have one).
    pub cycle: [f32; VIEW_CYCLE],
}

impl Default for ModuleView {
    fn default() -> Self {
        ModuleView {
            valid: false,
            position: 0.0,
            cycle: [0.0; VIEW_CYCLE],
        }
    }
}

/// The last 4096 output samples of a voice.
#[derive(Clone, Copy, Debug)]
pub struct Ring {
    buf: [f32; RING],
    head: usize,
}

impl Default for Ring {
    fn default() -> Self {
        Ring {
            buf: [0.0; RING],
            head: 0,
        }
    }
}

impl Ring {
    #[inline]
    pub fn push(&mut self, v: f32) {
        self.head = (self.head + 1) & (RING - 1);
        self.buf[self.head] = v;
    }

    /// Sample `back` places before the newest, linearly interpolated.
    fn at(&self, back: f32) -> f32 {
        let i = back.floor();
        let t = back - i;
        let a = self.buf[(self.head.wrapping_sub(i as usize)) & (RING - 1)];
        let b = self.buf[(self.head.wrapping_sub(i as usize + 1)) & (RING - 1)];
        a + (b - a) * t
    }

    /// One period of the output, `period` samples long (1 to ~2000), into `out`.
    pub fn cycle(&self, period: f32, out: &mut [f32; VIEW_CYCLE]) {
        let period = period.clamp(2.0, (RING / 2 - 2) as f32);
        // Newest rising zero crossing that leaves a whole period after it; else the last period.
        let mut start = period;
        let mut back = period.ceil() as usize;
        while (back as f32) < 2.0 * period {
            let (cur, prev) = (
                self.buf[self.head.wrapping_sub(back) & (RING - 1)],
                self.buf[self.head.wrapping_sub(back + 1) & (RING - 1)],
            );
            if prev < 0.0 && cur >= 0.0 {
                start = back as f32;
                break;
            }
            back += 1;
        }
        for (k, o) in out.iter_mut().enumerate() {
            *o = self.at(start - period * k as f32 / VIEW_CYCLE as f32);
        }
    }
}
