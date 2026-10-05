//! Diagnostic for the pinned framework's callback-facing AtomicCell types.
//! Compile against the workspace-built nice_plug/crossbeam_utils rlibs (see RT-OWNERSHIP.md).
use crossbeam_utils::atomic::AtomicCell;
use nice_plug::prelude::{AudioIOLayout, BufferConfig, ProcessMode, ProcessStatus};
use nice_plug::wrapper::clap::strict::{Lifecycle, Owner, Snapshot, TailStatus, Handoff, INACTIVE, ACTIVE, PROCESSING};

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
    println!("Above: retained negative controls, not the repaired callback storage.");
    assert!(AtomicCell::<u32>::is_lock_free() && AtomicCell::<u8>::is_lock_free() && AtomicCell::<usize>::is_lock_free());
    let lifecycle = Lifecycle::default();
    assert!(lifecycle.enter(1 << PROCESSING).is_none());
    let guard = lifecycle.enter(1 << INACTIVE).unwrap();
    assert!(lifecycle.enter(7).is_none());
    guard.transition(ACTIVE); drop(guard);
    let guard = lifecycle.enter(1 << ACTIVE).unwrap();
    guard.transition(PROCESSING); drop(guard);
    assert!(lifecycle.enter(1 << INACTIVE).is_none());
    let storage = Owner::new(std::cell::Cell::new(0));
    let owned = storage.try_borrow_mut().unwrap();
    assert!(storage.try_borrow_mut().is_none());
    owned.set(7); drop(owned);
    assert_eq!(storage.try_borrow_mut().unwrap().get(), 7);
    let snapshot = Snapshot::new(AudioIOLayout::default());
    assert_eq!(snapshot.load(), AudioIOLayout::default());
    let status = TailStatus::new(ProcessStatus::Tail(42));
    assert!(matches!(status.load(), ProcessStatus::Tail(42)));
    let queue = Handoff::new(2);
    for n in 0..6 { queue.push(n).unwrap(); }
    for n in 0..6 {
        let mut consumer = queue.consumer().unwrap();
        assert_eq!(consumer.peek(), Ok(&n));
        assert_eq!(consumer.peek(), Ok(&n)); // A rejected host push keeps the same head.
        assert_eq!(consumer.pop(), Ok(n));
        drop(consumer); queue.pump(2);
    }
    println!("Repaired profile primitives PASS: native scalar atomics, snapshots, checked ownership, saturation/rejection ordering.");
}
