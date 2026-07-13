//! Undo/redo support, built as a log of small reversible [`Edit`]s rather
//! than full-buffer snapshots.
//!
//! Every mutation [`crate::Document`] makes goes through exactly one shape:
//! "at `pos`, `removed` was replaced by `inserted`" — that's enough to
//! describe an insert (`removed` empty), a delete (`inserted` empty), or a
//! replacement, and enough to invert any of them by swapping the two
//! strings. Keeping only the changed text (rather than the whole buffer)
//! keeps undo cheap regardless of document size or storage backend.

use oxiden_buffer::Position;

/// A single reversible change: `removed` (the text that used to be at
/// `pos`) was replaced by `inserted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub pos: Position,
    pub removed: String,
    pub inserted: String,
}

impl Edit {
    /// The edit that undoes this one: applying `self` and then `self.inverse()`
    /// (or vice versa) is a no-op on the buffer.
    pub fn inverse(&self) -> Edit {
        Edit {
            pos: self.pos,
            removed: self.inserted.clone(),
            inserted: self.removed.clone(),
        }
    }
}

/// Tracks undo/redo state as a stack of edit groups.
///
/// Edits are coalesced into groups so that, e.g., typing "hello" undoes in
/// one step rather than five. A group is closed (no further edits will
/// merge into it) whenever [`Self::break_group`] is called — `Editor` does
/// this on cursor moves, so an edit made after moving the cursor elsewhere
/// never merges with one made before it.
pub struct History {
    undo_stack: Vec<Vec<Edit>>,
    redo_stack: Vec<Vec<Edit>>,
    current: Vec<Edit>,
}

impl History {
    /// An empty history: nothing to undo or redo yet.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current: Vec::new(),
        }
    }

    /// Records an edit that just happened, merging it into the
    /// in-progress group when it's a natural continuation (e.g. the next
    /// character typed right after the last one). Recording any edit
    /// clears the redo stack, since it invalidates the "future" that
    /// stack represented.
    pub fn record(&mut self, edit: Edit) {
        self.redo_stack.clear();

        match self.current.last() {
            Some(last) if Self::coalesces(last, &edit) => {
                self.current.push(edit)
            }
            Some(_) => {
                self.commit();
                self.current.push(edit);
            }
            None => self.current.push(edit),
        }
    }

    /// Closes the in-progress group, if any, so a later edit starts a new
    /// one instead of merging with it. Call this on cursor moves, saves,
    /// and around undo/redo itself.
    pub fn break_group(&mut self) {
        self.commit();
    }

    /// Pops the most recent undo group, closing the in-progress group
    /// first so a partially-typed run isn't left stranded.
    pub fn pop_undo(&mut self) -> Option<Vec<Edit>> {
        self.commit();
        self.undo_stack.pop()
    }

    /// Pops the most recent redo group.
    pub fn pop_redo(&mut self) -> Option<Vec<Edit>> {
        self.redo_stack.pop()
    }

    /// Pushes a group onto the undo stack — used by [`crate::Document::redo`]
    /// to record what redoing just did, so it can be undone again.
    pub fn push_undo(&mut self, group: Vec<Edit>) {
        self.undo_stack.push(group);
    }

    /// Pushes a group onto the redo stack — used by [`crate::Document::undo`]
    /// to record what undoing just did, so it can be redone.
    pub fn push_redo(&mut self, group: Vec<Edit>) {
        self.redo_stack.push(group);
    }

    /// Whether there is anything to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty() || !self.current.is_empty()
    }

    /// Whether there is anything to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn commit(&mut self) {
        if !self.current.is_empty() {
            self.undo_stack.push(std::mem::take(&mut self.current));
        }
    }

    /// Whether `next` is a natural continuation of `last` and should be
    /// merged into the same undo group. Deliberately conservative: only a
    /// straight run of single-line inserts immediately following one
    /// another, or a run of deletes from holding Backspace or Delete,
    /// coalesce. Anything else — including mixing inserts with deletes —
    /// starts a new group.
    fn coalesces(last: &Edit, next: &Edit) -> bool {
        let is_pure_insert =
            |e: &Edit| e.removed.is_empty() && !e.inserted.is_empty();
        let is_pure_delete =
            |e: &Edit| e.inserted.is_empty() && !e.removed.is_empty();

        if is_pure_insert(last) && is_pure_insert(next) {
            let end_of_last = Position::new(
                last.pos.line,
                last.pos.column + last.inserted.chars().count(),
            );

            return next.pos == end_of_last && !last.inserted.ends_with('\n');
        }

        if is_pure_delete(last) && is_pure_delete(next) {
            // Holding Delete: the cursor never moves, so each new block
            // is removed from the same position as the last.
            if next.pos == last.pos {
                return true;
            }

            // Holding Backspace: the cursor moves left with each press, so
            // each new block sits immediately *before* the last one.
            let end_of_next = Position::new(
                next.pos.line,
                next.pos.column + next.removed.chars().count(),
            );

            return end_of_next == last.pos;
        }

        false
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert(line: usize, column: usize, text: &str) -> Edit {
        Edit {
            pos: Position::new(line, column),
            removed: String::new(),
            inserted: text.into(),
        }
    }

    fn delete(line: usize, column: usize, text: &str) -> Edit {
        Edit {
            pos: Position::new(line, column),
            removed: text.into(),
            inserted: String::new(),
        }
    }

    #[test]
    fn inverse_swaps_removed_and_inserted() {
        let edit = Edit {
            pos: Position::new(0, 2),
            removed: "ab".into(),
            inserted: "xyz".into(),
        };

        let inverse = edit.inverse();

        assert_eq!(inverse.pos, edit.pos);
        assert_eq!(inverse.removed, "xyz");
        assert_eq!(inverse.inserted, "ab");
    }

    #[test]
    fn consecutive_single_line_inserts_coalesce_into_one_group() {
        let mut history = History::new();

        history.record(insert(0, 0, "h"));
        history.record(insert(0, 1, "e"));
        history.record(insert(0, 2, "l"));

        let group = history.pop_undo().unwrap();

        assert_eq!(group.len(), 3);
        assert!(history.pop_undo().is_none());
    }

    #[test]
    fn non_adjacent_inserts_start_a_new_group() {
        let mut history = History::new();

        history.record(insert(0, 0, "a"));
        history.record(insert(5, 0, "b")); // unrelated position

        assert_eq!(history.pop_undo().unwrap().len(), 1);
        assert_eq!(history.pop_undo().unwrap().len(), 1);
    }

    #[test]
    fn break_group_stops_further_coalescing() {
        let mut history = History::new();

        history.record(insert(0, 0, "a"));
        history.break_group();
        history.record(insert(0, 1, "b"));

        assert_eq!(history.pop_undo().unwrap().len(), 1);
        assert_eq!(history.pop_undo().unwrap().len(), 1);
    }

    #[test]
    fn repeated_backspace_coalesces() {
        // Backspacing through "abc" one character at a time: the cursor
        // moves left, so each new block sits just before the last one.
        let mut history = History::new();

        history.record(delete(0, 2, "c"));
        history.record(delete(0, 1, "b"));
        history.record(delete(0, 0, "a"));

        assert_eq!(history.pop_undo().unwrap().len(), 3);
    }

    #[test]
    fn repeated_delete_key_at_same_position_coalesces() {
        // Holding Delete: the cursor doesn't move, so each new block is
        // removed from the same position as the last.
        let mut history = History::new();

        history.record(delete(0, 3, "c"));
        history.record(delete(0, 3, "b"));

        assert_eq!(history.pop_undo().unwrap().len(), 2);
    }

    #[test]
    fn recording_an_edit_clears_the_redo_stack() {
        let mut history = History::new();

        history.record(insert(0, 0, "a"));
        let group = history.pop_undo().unwrap();
        history.push_redo(group);

        assert!(history.can_redo());

        history.record(insert(0, 0, "b"));

        assert!(!history.can_redo());
    }
}
