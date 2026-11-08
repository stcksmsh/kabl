//! Opt-in bump arena (cargo feature `arena`). A module that needs delay lines or tables
//! declares `const ARENA_BYTES: usize = N;` and allocates from [`Bump`] inside `Module::new`.
//! The memory is zeroed `.bss`: it adds nothing to the blob, and allocation only fails
//! (by returning `None`) when `new` asks for more than was declared.

use core::mem::{align_of, size_of};

mod sealed {
    pub trait Sealed {}
}

/// Types whose all-zero bytes are a valid value. Sealed: only plain numbers and arrays.
///
/// # Safety
/// Implemented in this crate only; zero must be a valid bit pattern.
pub unsafe trait Zeroable: Copy + 'static + sealed::Sealed {}

macro_rules! zeroable {
    ($($t:ty),*) => { $( impl sealed::Sealed for $t {} unsafe impl Zeroable for $t {} )* };
}
zeroable!(f32, f64, u8, u16, u32, u64, i8, i16, i32, i64, usize, isize);
impl<T: Zeroable, const N: usize> sealed::Sealed for [T; N] {}
unsafe impl<T: Zeroable, const N: usize> Zeroable for [T; N] {}

/// A bump allocator over a fixed zeroed buffer. Nothing is freed; the whole arena restarts
/// when the host re-initialises the module.
pub struct Bump {
    base: *mut u8,
    cap: usize,
    used: usize,
}

impl Bump {
    /// An arena over `buf`, which is zeroed here. Taking the exclusive `'static` borrow is what
    /// makes the slices handed out below sound.
    pub fn new(buf: &'static mut [u8]) -> Self {
        buf.fill(0);
        Bump {
            base: buf.as_mut_ptr(),
            cap: buf.len(),
            used: 0,
        }
    }

    /// Bytes still available (before alignment padding).
    pub fn remaining(&self) -> usize {
        self.cap - self.used
    }

    /// `n` zeroed values, or `None` if the arena is too small.
    pub fn alloc_slice<T: Zeroable>(&mut self, n: usize) -> Option<&'static mut [T]> {
        let start = (self.base as usize + self.used).next_multiple_of(align_of::<T>());
        let off = start - self.base as usize;
        let end = off.checked_add(n.checked_mul(size_of::<T>())?)?;
        if end > self.cap {
            return None;
        }
        self.used = end;
        // SAFETY: [off, end) lies inside the buffer this arena exclusively owns, is aligned
        // for T, is never handed out twice (`used` only grows), and zero is a valid T.
        Some(unsafe { core::slice::from_raw_parts_mut(self.base.add(off) as *mut T, n) })
    }
}
