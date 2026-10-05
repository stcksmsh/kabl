//! Diagnostic for the pinned framework's callback-facing AtomicCell types.
//! Compile against the workspace-built nice_plug/crossbeam_utils rlibs (see RT-OWNERSHIP.md).
use crossbeam_utils::atomic::AtomicCell;
use nice_plug::prelude::{AudioIOLayout, BufferConfig, ProcessMode, ProcessStatus};

fn report<T>(name: &str) {
    println!(
        "{name}: size={} align={} lock_free={}",
        std::mem::size_of::<T>(),
        std::mem::align_of::<T>(),
        AtomicCell::<T>::is_lock_free()
    );
}

fn main() {
    report::<AudioIOLayout>("AudioIOLayout");
    report::<Option<BufferConfig>>("Option<BufferConfig>");
    report::<ProcessStatus>("ProcessStatus");
    report::<ProcessMode>("ProcessMode");
    // A failure is the acceptance result, not a failed diagnostic execution.
    if !AtomicCell::<AudioIOLayout>::is_lock_free()
        || !AtomicCell::<Option<BufferConfig>>::is_lock_free()
        || !AtomicCell::<ProcessStatus>::is_lock_free()
    {
        eprintln!("R6 FAIL: callback-facing AtomicCell types use fallback locks");
        std::process::exit(2);
    }
}
