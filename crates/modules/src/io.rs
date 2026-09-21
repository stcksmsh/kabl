//! `ProcessIo` (brief section 8): "exposes each input as either a scalar or a buffer (control-
//! rate tier). Provide helpers so module authors handle both without branching per sample."
//! `Signal::at(i)` is that helper — same idea S2's spike (`engine::potato`) hand-rolled per
//! signal; here it's a reusable, generic type any `Module` impl can use.

/// One input or param value for a block: either a genuinely audio-rate buffer, or a value the
/// compiler (not built yet) classified as slow enough to hold as one block-scalar (brief section
/// 7's control-rate tier). `Copy` — cheap to pass around, nothing here owns data.
#[derive(Debug, Clone, Copy)]
pub enum Signal<'a> {
    Scalar(f32),
    Buffer(&'a [f32]),
}

impl<'a> Signal<'a> {
    /// The value at sample `i` within the current block, regardless of which representation
    /// this is — the helper brief section 8 asks for, so module authors write one code path
    /// instead of branching on `Scalar` vs. `Buffer` per sample.
    #[inline]
    pub fn at(&self, i: usize) -> f32 {
        match self {
            Signal::Scalar(v) => *v,
            Signal::Buffer(b) => b[i],
        }
    }

    #[inline]
    pub fn is_scalar(&self) -> bool {
        matches!(self, Signal::Scalar(_))
    }
}

/// What a `Module::process` call sees: its input signals (audio-rate or held-scalar), its
/// param values (same), its output buffers to fill, and the block length actually in effect
/// this call (`<=` the module's prepared `max_block`, brief section 8: `prepare(sample_rate,
/// max_block, ...)`).
///
/// Indices into `inputs`/`outputs`/`params` match the module's `ModuleInfo.ports`/`.params`
/// order — e.g. `io.input(0)` is `ModuleInfo.ports[k]` for the k-th input port. Constructed
/// fresh by the compiler (not built yet) for each `process()` call; doesn't outlive it.
pub struct ProcessIo<'a> {
    inputs: &'a [Signal<'a>],
    outputs: &'a mut [&'a mut [f32]],
    params: &'a [Signal<'a>],
    block_len: usize,
}

impl<'a> ProcessIo<'a> {
    pub fn new(
        inputs: &'a [Signal<'a>],
        outputs: &'a mut [&'a mut [f32]],
        params: &'a [Signal<'a>],
        block_len: usize,
    ) -> Self {
        ProcessIo {
            inputs,
            outputs,
            params,
            block_len,
        }
    }

    #[inline]
    pub fn block_len(&self) -> usize {
        self.block_len
    }

    #[inline]
    pub fn input(&self, index: usize) -> Signal<'a> {
        self.inputs[index]
    }

    #[inline]
    pub fn param(&self, index: usize) -> Signal<'a> {
        self.params[index]
    }

    #[inline]
    pub fn output(&mut self, index: usize) -> &mut [f32] {
        self.outputs[index]
    }
}
