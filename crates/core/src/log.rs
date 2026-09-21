use crate::op::{Entry, Op, Source};
use crate::state::PatchState;

/// Consecutive `SetParam` entries on the same target from the same source, within this many
/// ms, merge into one entry (first inverse kept, last value kept). Brief section 6.
pub const COALESCE_WINDOW_MS: u64 = 250;

/// Full-state snapshot taken every N ops so replay can start from the nearest checkpoint
/// instead of the beginning. Brief section 6, default N = 500.
pub const CHECKPOINT_EVERY: u64 = 500;

/// Append-only op log plus the state it replays to. This is the patch: state is a cache,
/// derivable at any point by replaying `entries[..cursor]` from empty. Brief section 4.1.
#[derive(Debug, Clone)]
pub struct PatchLog {
    entries: Vec<Entry>,
    /// Number of entries currently applied. `entries[cursor..]` is the redo tail.
    cursor: usize,
    state: PatchState,
    next_seq: u64,
}

impl Default for PatchLog {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchLog {
    pub fn new() -> Self {
        PatchLog {
            entries: Vec::new(),
            cursor: 0,
            state: PatchState::new(),
            next_seq: 0,
        }
    }

    pub fn state(&self) -> &PatchState {
        &self.state
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries[..self.cursor]
    }

    pub fn all_entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }

    /// Append a new user-facing op, discarding any redo tail. Coalesces with the previous
    /// entry when it's a `SetParam` on the same target, same source, within
    /// `COALESCE_WINDOW_MS`.
    pub fn append(&mut self, op: Op, t_ms: u64, source: Source) {
        self.entries.truncate(self.cursor);

        if let Op::SetParam { target, value } = &op {
            if let Some(last) = self.entries.last_mut() {
                if last.source == source && t_ms.saturating_sub(last.t_ms) <= COALESCE_WINDOW_MS {
                    if let Op::SetParam {
                        target: last_target,
                        value: last_value,
                    } = &mut last.op
                    {
                        if last_target == target {
                            *last_value = *value;
                            last.t_ms = t_ms;
                            self.state.apply(&op);
                            return;
                        }
                    }
                }
            }
        }

        let inverse = self.state.inverse_for(&op);
        self.state.apply(&op);
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push(Entry {
            seq,
            t_ms,
            op,
            inverse,
            source,
        });
        self.cursor = self.entries.len();
    }

    /// Push a fully-formed entry (from a file load) without coalescing or inverse
    /// recomputation — the entry already carries the correct inverse from when it was saved.
    pub(crate) fn append_entry_raw(&mut self, entry: Entry) {
        self.state.apply(&entry.op);
        self.next_seq = self.next_seq.max(entry.seq + 1);
        self.entries.push(entry);
        self.cursor = self.entries.len();
    }

    /// O(1): apply the inverse of the last-applied entry, no replay.
    pub fn undo(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        let inverse = self.entries[self.cursor].inverse.clone();
        self.state.apply(&inverse);
        true
    }

    /// O(1): re-apply the op that undo last reverted.
    pub fn redo(&mut self) -> bool {
        if self.cursor >= self.entries.len() {
            return false;
        }
        let op = self.entries[self.cursor].op.clone();
        self.state.apply(&op);
        self.cursor += 1;
        true
    }
}
