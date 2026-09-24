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
        self.append_inner(op, t_ms, source, Some(COALESCE_WINDOW_MS));
    }

    /// Like `append`, but never coalesces: the start of a new drag gesture is its own undo
    /// step even right after a previous gesture on the same target.
    pub fn append_new(&mut self, op: Op, t_ms: u64, source: Source) {
        self.append_inner(op, t_ms, source, None);
    }

    /// Like `append`, but a `SetParam` always merges into the previous entry when that entry
    /// is a `SetParam` on the same target from the same source, however long ago it was. For
    /// the frames after the first one of a continuous drag gesture, so the whole gesture undoes
    /// as one step even when the hand pauses longer than `COALESCE_WINDOW_MS`.
    pub fn append_continuing(&mut self, op: Op, t_ms: u64, source: Source) {
        self.append_inner(op, t_ms, source, Some(u64::MAX));
    }

    /// For hardware gestures that move several targets at once (MIDI CCs): with `continuing`,
    /// a `SetParam` joins the newest entry when that is a group of `SetParam`s from the same
    /// source (replacing its value for a target already in it); otherwise it starts a new
    /// group. So interleaved turns on different controls undo as one step.
    pub fn append_to_group(&mut self, op: Op, t_ms: u64, source: Source, continuing: bool) {
        self.entries.truncate(self.cursor);
        if let (Op::SetParam { target, value }, true) = (&op, continuing) {
            let restore = self.state.inverse_for(&op);
            if let Some(last) = self.entries.last_mut().filter(|e| e.source == source) {
                if let (Op::Group { ops }, Op::Group { ops: inverse }) =
                    (&mut last.op, &mut last.inverse)
                {
                    if !ops.is_empty() && ops.iter().all(|o| matches!(o, Op::SetParam { .. })) {
                        match ops.iter_mut().find_map(|o| match o {
                            Op::SetParam {
                                target: t,
                                value: v,
                            } if t == target => Some(v),
                            _ => None,
                        }) {
                            Some(v) => *v = *value,
                            None => {
                                ops.push(op.clone());
                                inverse.push(restore);
                            }
                        }
                        last.t_ms = t_ms;
                        self.state.apply(&op);
                        return;
                    }
                }
            }
        }
        self.append_inner(Op::Group { ops: vec![op] }, t_ms, source, None);
    }

    /// `window`: merge a `SetParam` into the previous same-target, same-source `SetParam` if it
    /// is at most this many ms older. `None` never merges.
    fn append_inner(&mut self, op: Op, t_ms: u64, source: Source, window: Option<u64>) {
        self.entries.truncate(self.cursor);

        if let (Op::SetParam { target, value }, Some(window)) = (&op, window) {
            if let Some(last) = self.entries.last_mut() {
                if last.source == source && t_ms.saturating_sub(last.t_ms) <= window {
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

    /// The entry the next `undo()` would revert, if any.
    pub fn undo_entry(&self) -> Option<&Entry> {
        self.cursor.checked_sub(1).map(|i| &self.entries[i])
    }

    /// The entry the next `redo()` would re-apply, if any.
    pub fn redo_entry(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }

    /// Push an entry loaded from a file without coalescing. The inverse is recomputed from the
    /// replayed state rather than trusted from the file: schema-v1 files stored lossy inverses
    /// (`0.0` for an absent param, a bare `AddModule` for a removed module), and the replayed
    /// state is exactly what the op was originally applied to.
    pub(crate) fn append_entry_raw(&mut self, mut entry: Entry) {
        entry.inverse = self.state.inverse_for(&entry.op);
        self.state.apply(&entry.op);
        self.next_seq = self.next_seq.max(entry.seq + 1);
        self.entries.push(entry);
        self.cursor = self.entries.len();
    }

    /// Reverts the last entry and forgets it, leaving nothing to redo: for a gesture the user
    /// cancelled (Escape mid-drag). Only acts when that entry is the newest one.
    pub fn discard_last(&mut self) -> bool {
        if self.cursor == 0 || self.cursor != self.entries.len() {
            return false;
        }
        self.undo();
        self.entries.truncate(self.cursor);
        true
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
