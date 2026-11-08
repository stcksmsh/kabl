//! Saved settings: a module writes bytes with [`StateWriter`] and reads them with
//! [`StateReader`]. The bytes travel through the module's transfer buffer, whose size is the
//! module's `STATE_CAP`. Overflow is an error (the host keeps the previous settings), never
//! a silent truncation.

/// Appends little-endian values to the transfer buffer. Writing past the end sets a flag instead
/// of panicking; `state_save` then reports failure.
pub struct StateWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
    over: bool,
}

impl<'a> StateWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        StateWriter {
            buf,
            len: 0,
            over: false,
        }
    }

    pub fn bytes(&mut self, b: &[u8]) {
        match self.buf.get_mut(self.len..self.len + b.len()) {
            Some(dst) if !self.over => {
                dst.copy_from_slice(b);
                self.len += b.len();
            }
            _ => self.over = true,
        }
    }

    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.bytes(&v.to_le_bytes());
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True once a write did not fit.
    pub fn overflowed(&self) -> bool {
        self.over
    }
}

/// Reads little-endian values back. Every getter returns `None` at the end of the data, so a
/// short or corrupt blob makes `load` return `false` rather than panic.
pub struct StateReader<'a> {
    buf: &'a [u8],
}

impl<'a> StateReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        StateReader { buf }
    }

    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.buf.split_at_checked(n)?;
        self.buf = tail;
        Some(head)
    }

    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    pub fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }

    /// Unread bytes.
    pub fn remaining(&self) -> usize {
        self.buf.len()
    }
}
