//! [`Buffer`]: a thin, storage-agnostic wrapper that adds a handful of
//! query helpers (line length, "is this the last line/column", position
//! validity) on top of a raw [`TextStorage`].

use crate::{Position, Range, Result, TextStorage};

/// A document's text, generic over its storage backend `S`.
///
/// `Buffer` itself holds no editing state beyond the text (no cursor, no
/// undo history, no file path) — those concerns live in `oxiden-core`. It
/// exists mainly to pair a [`TextStorage`] with convenience queries that
/// are useful to callers navigating the text (e.g. cursor motion needs to
/// know a line's length and whether it's the last line).
pub struct Buffer<S: TextStorage> {
    storage: S,
}

impl<S: TextStorage> Buffer<S> {
    /// Wraps an existing storage backend in a `Buffer`.
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Returns the text of the line at `index`, or `None` if out of range.
    pub fn line(&self, index: usize) -> Option<S::Line<'_>> {
        self.storage.line(index)
    }

    /// Number of lines currently in the buffer (always >= 1).
    pub fn line_count(&self) -> usize {
        self.storage.line_count()
    }

    /// Total character count across the whole buffer, including implicit
    /// newlines between lines.
    pub fn len_chars(&self) -> usize {
        self.storage.len_chars()
    }

    /// Inserts `text` at `pos`. See [`TextStorage::insert`] for details.
    pub fn insert(&mut self, pos: Position, text: &str) -> Result<()> {
        self.storage.insert(pos, text)
    }

    /// Deletes the text spanned by `range`. See [`TextStorage::delete`] for
    /// details.
    pub fn delete(&mut self, range: Range) -> Result<()> {
        self.storage.delete(range)
    }

    /// Character length of the given line, or `None` if the line doesn't
    /// exist.
    pub fn line_len(&self, line: usize) -> Option<usize> {
        self.line(line).map(|line| line.as_ref().chars().count())
    }

    /// Whether `line` is the last line in the buffer.
    pub fn is_last_line(&self, line: usize) -> bool {
        line + 1 == self.line_count()
    }

    /// Whether `pos` sits exactly at the end of its line (one past the
    /// last character). Used to detect "end of line" for cursor motion and
    /// deletion, where crossing this boundary means joining with the next
    /// line rather than deleting within the current one.
    pub fn is_last_column(&self, pos: Position) -> bool {
        self.line_len(pos.line).is_some_and(|len| pos.column == len)
    }

    /// Whether `pos` refers to an existing line and a column within (or at
    /// the end of) that line — i.e. whether it would be accepted by
    /// [`Self::insert`]/[`Self::delete`].
    pub fn is_valid_position(&self, pos: Position) -> bool {
        self.line_len(pos.line).is_some_and(|len| pos.column <= len)
    }

    /// Renders the entire buffer as a single `String` (lines joined by
    /// `\n`, no trailing newline).
    pub fn to_text(&self) -> String {
        self.storage.to_text()
    }

    /// Returns the text spanned by `range` (assumed to already be
    /// normalized, i.e. `range.start <= range.end`).
    ///
    /// Used by undo/redo to capture what a deletion is about to remove,
    /// before it's gone.
    pub fn text_in_range(&self, range: Range) -> String {
        if range.start.line == range.end.line {
            return self
                .line(range.start.line)
                .map(|line| {
                    line.as_ref()
                        .chars()
                        .skip(range.start.column)
                        .take(range.end.column - range.start.column)
                        .collect()
                })
                .unwrap_or_default();
        }

        let mut text = String::new();

        if let Some(first) = self.line(range.start.line) {
            text.extend(first.as_ref().chars().skip(range.start.column));
        }

        for line in range.start.line + 1..range.end.line {
            text.push('\n');
            if let Some(line) = self.line(line) {
                text.push_str(line.as_ref());
            }
        }

        text.push('\n');

        if let Some(last) = self.line(range.end.line) {
            text.extend(last.as_ref().chars().take(range.end.column));
        }

        text
    }
}
