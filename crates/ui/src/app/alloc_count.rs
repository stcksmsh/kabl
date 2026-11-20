//! Test-only global allocator counting allocations inside the audio callback.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! { static INSIDE: Cell<bool> = const { Cell::new(false) }; }
pub static ALLOCS: AtomicUsize = AtomicUsize::new(0);
pub static DEALLOCS: AtomicUsize = AtomicUsize::new(0);
pub struct Counter;
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
            DEALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, size)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc_zeroed(layout)
    }
}
pub struct Scope(bool);
impl Scope {
    pub fn new(enabled: bool) -> Self {
        if enabled {
            INSIDE.with(|v| v.set(true));
        }
        Self(enabled)
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.0 {
            INSIDE.with(|v| v.set(false));
        }
    }
}
