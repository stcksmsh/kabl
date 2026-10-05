//! Kabl's opt-in CLAP profile. No waiting or retry loops on the consumer/audio side.
//! CLAP serializes one instance's audio callbacks and inactive main-thread lifecycle.
//! The gate additionally rejects overlapping/reentrant calls without creating aliases.
use atomic_refcell::{AtomicRefCell, AtomicRefMut};
use nice_plug_core::plugin::ProcessStatus;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

pub const INACTIVE: u8 = 0;
pub const ACTIVE: u8 = 1;
pub const PROCESSING: u8 = 2;

/// Transferable exclusive storage. No shared-reference accessor exists.
pub struct Owner<T>(AtomicRefCell<T>);
// SAFETY: T: Send allows thread transfer. AtomicRefCell rejects a second mutable
// borrow; Owner never exposes borrow()/get()/a shared T reference. All access to
// a non-Sync T therefore stays exclusive, including invalid/reentrant host calls.
unsafe impl<T: Send> Sync for Owner<T> {}
impl<T> Owner<T> {
    pub fn new(value: T) -> Self {
        Self(AtomicRefCell::new(value))
    }
    pub fn try_borrow_mut(&self) -> Option<AtomicRefMut<'_, T>> {
        self.0.try_borrow_mut().ok()
    }
}

#[derive(Default)]
pub struct Lifecycle {
    phase: AtomicU8,
    entered: AtomicBool,
}
pub struct Callback<'a>(&'a Lifecycle);
impl Lifecycle {
    /// Single attempt, never a callback spin/retry. Phase mask: 1 << phase.
    pub fn enter(&self, allowed: u8) -> Option<Callback<'_>> {
        if allowed & (1 << self.phase.load(Ordering::Acquire)) == 0
            || self
                .entered
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
        {
            return None;
        }
        if allowed & (1 << self.phase.load(Ordering::Acquire)) == 0 {
            self.entered.store(false, Ordering::Release);
            return None;
        }
        Some(Callback(self))
    }
}
impl Callback<'_> {
    pub fn transition(&self, phase: u8) {
        self.0.phase.store(phase, Ordering::Release);
    }
}
impl Drop for Callback<'_> {
    fn drop(&mut self) {
        self.0.entered.store(false, Ordering::Release);
    }
}

/// Mutated only under an inactive lifecycle gate, read in serialized callbacks.
/// AtomicRefCell checks aliases even for a host violating the lifecycle contract.
pub struct Snapshot<T>(AtomicRefCell<T>);
impl<T: Copy> Snapshot<T> {
    pub fn new(value: T) -> Self {
        Self(AtomicRefCell::new(value))
    }
    pub fn load(&self) -> T {
        *self.0.borrow()
    }
    pub fn store(&self, value: T) {
        *self.0.borrow_mut() = value;
    }
}

/// The tail extension needs only a scalar sample count, not an Error string.
pub struct TailStatus(AtomicU32);
impl TailStatus {
    pub fn new(status: ProcessStatus) -> Self {
        let result = Self(AtomicU32::new(0));
        result.store(status);
        result
    }
    pub fn store(&self, status: ProcessStatus) {
        self.0.store(
            match status {
                ProcessStatus::Tail(n) => n,
                ProcessStatus::KeepAlive => u32::MAX,
                _ => 0,
            },
            Ordering::Release,
        );
    }
    pub fn load(&self) -> ProcessStatus {
        match self.0.load(Ordering::Acquire) {
            0 => ProcessStatus::Normal,
            u32::MAX => ProcessStatus::KeepAlive,
            n => ProcessStatus::Tail(n),
        }
    }
}

struct Producer<T> {
    ring: strict_rtrb::Producer<T>,
    pending: VecDeque<T>,
}
/// Producers are serialized OFF audio. Overflow remains in their owned queue.
/// Only the fixed-capacity SPSC consumer enters process/flush. Failed host pushes
/// keep the head event untouched, preserving gesture order across callbacks.
pub struct Handoff<T> {
    producer: Mutex<Producer<T>>,
    consumer: Owner<strict_rtrb::Consumer<T>>,
    pending: AtomicBool,
}
impl<T> Handoff<T> {
    pub fn new(capacity: usize) -> Self {
        let (ring, consumer) = strict_rtrb::RingBuffer::new(capacity);
        Self {
            producer: Mutex::new(Producer {
                ring,
                pending: VecDeque::new(),
            }),
            consumer: Owner::new(consumer),
            pending: AtomicBool::new(false),
        }
    }
    /// Called by GUI producers, never an audio callback. No accepted gesture is discarded.
    pub fn push(&self, value: T) -> Result<(), T> {
        let mut producer = self.producer.lock();
        if producer.pending.is_empty() {
            match producer.ring.push(value) {
                Ok(()) => return Ok(()),
                Err(strict_rtrb::PushError::Full(value)) => producer.pending.push_back(value),
            }
        } else {
            producer.pending.push_back(value);
        }
        self.pending.store(true, Ordering::Release);
        Ok(())
    }
    /// Called only by host main callback. Allocation/destruction stays off audio.
    pub fn pump(&self, budget: usize) -> usize {
        let mut producer = self.producer.lock();
        let mut published = 0;
        for _ in 0..budget {
            let Some(value) = producer.pending.pop_front() else {
                break;
            };
            if let Err(strict_rtrb::PushError::Full(value)) = producer.ring.push(value) {
                producer.pending.push_front(value);
                break;
            }
            published += 1;
        }
        self.pending
            .store(!producer.pending.is_empty(), Ordering::Release);
        published
    }
    pub fn has_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }
    pub fn consumer(&self) -> Option<AtomicRefMut<'_, strict_rtrb::Consumer<T>>> {
        self.consumer.try_borrow_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    #[test]
    fn phases_and_overlap_fail_without_waiting() {
        let lifecycle = Arc::new(Lifecycle::default());
        assert!(lifecycle.enter(1 << PROCESSING).is_none());
        let guard = lifecycle.enter(1 << INACTIVE).unwrap();
        let other = lifecycle.clone();
        assert!(
            !std::thread::spawn(move || other.enter(7).is_some())
                .join()
                .unwrap()
        );
        guard.transition(ACTIVE);
        drop(guard);
        let guard = lifecycle.enter(1 << ACTIVE).unwrap();
        guard.transition(PROCESSING);
        drop(guard);
        assert!(lifecycle.enter(1 << INACTIVE).is_none());
        let guard = lifecycle.enter(1 << PROCESSING).unwrap();
        guard.transition(ACTIVE);
        drop(guard);
        let guard = lifecycle.enter(1 << ACTIVE).unwrap();
        guard.transition(INACTIVE);
        drop(guard);
        assert!(lifecycle.enter(1 << INACTIVE).is_some());
    }
    #[test]
    fn paused_producer_does_not_block_consumer() {
        let handoff = Arc::new(Handoff::<u32>::new(4));
        let barrier = Arc::new(Barrier::new(2));
        let other = handoff.clone();
        let sync = barrier.clone();
        let thread = std::thread::spawn(move || {
            let _producer = other.producer.lock();
            sync.wait();
            sync.wait();
        });
        barrier.wait();
        assert!(handoff.consumer().unwrap().peek().is_err());
        barrier.wait();
        thread.join().unwrap();
    }
    #[test]
    fn saturation_and_rejected_head_preserve_complete_gestures() {
        let handoff = Handoff::new(2);
        for event in 0..30 {
            handoff.push(event).unwrap();
        }
        {
            let consumer = handoff.consumer().unwrap();
            assert_eq!(consumer.peek(), Ok(&0));
        }
        let mut received = Vec::new();
        while received.len() < 30 {
            {
                let mut consumer = handoff.consumer().unwrap();
                for _ in 0..2 {
                    match consumer.pop() {
                        Ok(event) => received.push(event),
                        Err(_) => break,
                    }
                }
            }
            handoff.pump(2);
        }
        assert_eq!(received, (0..30).collect::<Vec<_>>());
        assert!(!handoff.has_pending());
    }
    #[test]
    fn replenishment_cannot_extend_callback_budget() {
        let handoff = Arc::new(Handoff::new(8));
        let producer = handoff.clone();
        let thread = std::thread::spawn(move || {
            for event in 0..1000 {
                producer.push(event).unwrap();
            }
        });
        let mut count = 0;
        {
            let mut consumer = handoff.consumer().unwrap();
            for _ in 0..8 {
                if consumer.pop().is_ok() {
                    count += 1;
                }
            }
        }
        assert!(count <= 8);
        thread.join().unwrap();
        let mut received = Vec::new();
        while received.len() < 1000 - count {
            handoff.pump(8);
            let mut consumer = handoff.consumer().unwrap();
            for _ in 0..8 {
                if let Ok(value) = consumer.pop() {
                    received.push(value);
                }
            }
        }
        assert_eq!(received, (count..1000).collect::<Vec<_>>());
    }
}
