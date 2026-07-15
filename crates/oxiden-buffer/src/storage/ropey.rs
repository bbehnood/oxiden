//! A [`TextStorage`] backend wrapping [`ropey::Rope`], the production rope
//! implementation used by several real text editors (e.g. Helix).
//!
//! This exists alongside the hand-rolled [`super::RopeStorage`] rather than
//! instead of it: `RopeStorage` shows what a rope actually does under the
//! hood, while `RopeyStorage` is what you'd reach for in an editor that
//! needs to be correct and fast on real-world text without maintaining that
//! data structure yourself. `ropey` also does meaningfully more than
//! `RopeStorage` does — it's chunk-based rather than a plain binary tree,
//! and it tracks byte, char, *and* UTF-16 code-unit offsets internally,
//! which `RopeStorage` doesn't need since this crate only ever indexes by
//! character.
//!
//! One behavioral difference is worth calling out: `ropey` splits lines
//! according to the Unicode line-breaking rules, which treat `\r\n`, `\r`,
//! `\u{0B}` (vertical tab), `\u{0C}` (form feed), `\u{0085}` (NEL),
//! `\u{2028}` (line separator), and `\u{2029}` (paragraph separator) as line
//! boundaries in addition to plain `\n`. [`super::VecStorage`] and
//! [`super::RopeStorage`] only ever split on `\n`. For ordinary LF-only text
//! (the common case for a terminal editor) this is unobservable; a file
//! containing bare `\r` or one of the rarer Unicode separators would be
//! reported as having more lines here than under the other two backends.

use ropey::{Rope, RopeSlice};

use crate::{BufferError, Position, Range, Result, TextStorage};

/// The char-length of the line terminator `slice` ends with, or `0` if it
/// doesn't end with one. Used to exclude the terminator from a line's
/// reported content and length, since `ropey`'s own [`Rope::line`] includes
/// it (mirroring how `\n` is excluded from a [`super::VecStorage`] or
/// [`super::RopeStorage`] line).
fn terminator_len(slice: &RopeSlice) -> usize {
    let len = slice.len_chars();
    if len == 0 {
        return 0;
    }

    match slice.char(len - 1) {
        '\n' => {
            if len >= 2 && slice.char(len - 2) == '\r' {
                2
            } else {
                1
            }
        }
        '\r' | '\u{0B}' | '\u{0C}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => 1,
        _ => 0,
    }
}

/// [`TextStorage`] implementation backed by [`ropey::Rope`].
pub struct RopeyStorage {
    rope: Rope,
}

impl RopeyStorage {
    /// Creates an empty document: one line containing no text.
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// The line at `index`, including its terminator if it has one — only
    /// the last line in the document may lack one.
    fn raw_line(&self, index: usize) -> RopeSlice<'_> {
        self.rope.line(index)
    }

    /// The number of content characters (i.e. excluding any line
    /// terminator) on line `index`.
    fn line_len(&self, index: usize) -> usize {
        let slice = self.raw_line(index);
        let is_last = index + 1 == self.rope.len_lines();
        let terminator = if is_last { 0 } else { terminator_len(&slice) };

        slice.len_chars() - terminator
    }

    /// Checks that `pos` refers to an existing line and a column that is
    /// within (or exactly at the end of) that line's content.
    fn validate_position(&self, pos: Position) -> Result<()> {
        if pos.line >= self.rope.len_lines() {
            return Err(BufferError::InvalidPosition);
        }

        if pos.column > self.line_len(pos.line) {
            return Err(BufferError::InvalidPosition);
        }

        Ok(())
    }

    /// Converts a validated `pos` into a global character offset into the
    /// rope.
    fn char_offset(&self, pos: Position) -> usize {
        self.rope.line_to_char(pos.line) + pos.column
    }
}

impl Default for RopeyStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl TextStorage for RopeyStorage {
    type Line<'a> = String;

    fn line(&self, index: usize) -> Option<Self::Line<'_>> {
        if index >= self.rope.len_lines() {
            return None;
        }

        let content_len = self.line_len(index);
        Some(self.raw_line(index).slice(..content_len).to_string())
    }

    fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    fn to_text(&self) -> String {
        self.rope.to_string()
    }

    fn insert(&mut self, pos: Position, text: &str) -> Result<()> {
        self.validate_position(pos)?;

        let offset = self.char_offset(pos);
        self.rope.insert(offset, text);

        Ok(())
    }

    fn delete(&mut self, mut range: Range) -> Result<()> {
        self.validate_position(range.start)?;
        self.validate_position(range.end)?;

        // Callers may hand us a range built from an end-to-start drag, so
        // normalize direction before doing anything else.
        if range.start > range.end {
            std::mem::swap(&mut range.start, &mut range.end);
        }

        let start = self.char_offset(range.start);
        let end = self.char_offset(range.end);
        self.rope.remove(start..end);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `RopeyStorage` from lines, joined by `\n`, through the
    /// public API — mirrors the equivalent helper in `rope.rs`/`vec.rs`.
    fn storage(lines: &[&str]) -> RopeyStorage {
        let mut storage = RopeyStorage::new();
        let text = lines.join("\n");

        if !text.is_empty() {
            storage.insert(Position::new(0, 0), &text).unwrap();
        }

        storage
    }

    // ===== Constructor =====

    #[test]
    fn new_has_one_empty_line() {
        let storage = RopeyStorage::new();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some(String::new()));
        assert_eq!(storage.len_chars(), 0);
    }

    // ===== len_chars =====

    #[test]
    fn len_chars_ascii() {
        let storage = storage(&["abc", "de"]);

        // "abc\nde"
        assert_eq!(storage.len_chars(), 6);
    }

    #[test]
    fn len_chars_unicode() {
        let storage = storage(&["😀😁", "😂"]);

        // 😀😁\n😂
        assert_eq!(storage.len_chars(), 4);
    }

    // ===== to_text =====

    #[test]
    fn to_text_single_line() {
        let storage = storage(&["Hello"]);

        assert_eq!(storage.to_text(), "Hello");
    }

    #[test]
    fn to_text_multiple_lines() {
        let storage = storage(&["one", "two", "three"]);

        assert_eq!(storage.to_text(), "one\ntwo\nthree");
    }

    #[test]
    fn to_text_empty_buffer() {
        let storage = RopeyStorage::new();

        assert_eq!(storage.to_text(), "");
    }

    #[test]
    fn to_text_preserves_blank_lines() {
        let storage = storage(&["a", "", "b"]);

        assert_eq!(storage.to_text(), "a\n\nb");
    }

    // ===== Insert =====

    #[test]
    fn insert_into_empty() {
        let mut storage = storage(&[""]);

        storage.insert(Position::new(0, 0), "Hello").unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn insert_at_beginning() {
        let mut storage = storage(&["World"]);

        storage.insert(Position::new(0, 0), "Hello ").unwrap();

        assert_eq!(storage.line(0), Some("Hello World".to_string()));
    }

    #[test]
    fn insert_at_end() {
        let mut storage = storage(&["Hello"]);

        storage.insert(Position::new(0, 5), " World").unwrap();

        assert_eq!(storage.line(0), Some("Hello World".to_string()));
    }

    #[test]
    fn insert_in_middle() {
        let mut storage = storage(&["Helo"]);

        storage.insert(Position::new(0, 2), "l").unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn insert_single_newline() {
        let mut storage = storage(&["HelloWorld"]);

        storage.insert(Position::new(0, 5), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some("Hello".to_string()));
        assert_eq!(storage.line(1), Some("World".to_string()));
    }

    #[test]
    fn insert_multiple_newlines() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 1), "\n123\n456\n").unwrap();

        assert_eq!(storage.line_count(), 4);
        assert_eq!(storage.line(0), Some("a".to_string()));
        assert_eq!(storage.line(1), Some("123".to_string()));
        assert_eq!(storage.line(2), Some("456".to_string()));
        assert_eq!(storage.line(3), Some("bc".to_string()));
    }

    #[test]
    fn insert_trailing_newline() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 3), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some("abc".to_string()));
        assert_eq!(storage.line(1), Some(String::new()));
    }

    #[test]
    fn insert_leading_newline() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 0), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some(String::new()));
        assert_eq!(storage.line(1), Some("abc".to_string()));
    }

    #[test]
    fn insert_unicode() {
        let mut storage = storage(&["😀😁"]);

        storage.insert(Position::new(0, 1), "😂").unwrap();

        assert_eq!(storage.line(0), Some("😀😂😁".to_string()));
    }

    // ===== Delete =====

    #[test]
    fn delete_empty_range() {
        let mut storage = storage(&["Hello"]);

        storage
            .delete(Range::new(Position::new(0, 2), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn delete_single_character() {
        let mut storage = storage(&["Hello"]);

        storage
            .delete(Range::new(Position::new(0, 1), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("Hllo".to_string()));
    }

    #[test]
    fn delete_middle() {
        let mut storage = storage(&["Hello World"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(0, 6)))
            .unwrap();

        assert_eq!(storage.line(0), Some("HelloWorld".to_string()));
    }

    #[test]
    fn delete_newline_between_lines() {
        let mut storage = storage(&["Hello", "World"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(1, 0)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some("HelloWorld".to_string()));
    }

    #[test]
    fn delete_across_lines() {
        let mut storage = storage(&["Hello", "Beautiful", "World"]);

        storage
            .delete(Range::new(Position::new(0, 2), Position::new(2, 3)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some("Held".to_string()));
    }

    #[test]
    fn delete_everything() {
        let mut storage = storage(&["Hello", "World"]);

        storage
            .delete(Range::new(Position::new(0, 0), Position::new(1, 5)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some(String::new()));
    }

    #[test]
    fn delete_unicode() {
        let mut storage = storage(&["😀😁😂"]);

        storage
            .delete(Range::new(Position::new(0, 1), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("😀😂".to_string()));
    }

    #[test]
    fn delete_reversed_range() {
        let mut storage = storage(&["abcdef"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("abf".to_string()));
    }

    // ===== Validation =====

    #[test]
    fn insert_invalid_line() {
        let mut storage = storage(&["abc"]);

        assert!(matches!(
            storage.insert(Position::new(5, 0), "x"),
            Err(BufferError::InvalidPosition)
        ));
    }

    #[test]
    fn insert_invalid_column() {
        let mut storage = storage(&["abc"]);

        assert!(matches!(
            storage.insert(Position::new(0, 10), "x"),
            Err(BufferError::InvalidPosition)
        ));
    }

    #[test]
    fn delete_invalid_position() {
        let mut storage = storage(&["abc"]);

        assert!(matches!(
            storage
                .delete(Range::new(Position::new(0, 0), Position::new(5, 0),)),
            Err(BufferError::InvalidPosition)
        ));
    }

    // ===== Large documents =====
    //
    // Nothing here is rope-internals-specific (ropey's chunking is private),
    // but a document large enough to span many internal chunks is worth
    // exercising end-to-end since that's the whole point of using ropey.

    #[test]
    fn large_document_survives_many_edits() {
        let mut storage = RopeyStorage::new();

        let line = "the quick brown fox jumps over the lazy dog";
        let lines: Vec<&str> = std::iter::repeat_n(line, 2000).collect();
        let text = lines.join("\n");

        storage.insert(Position::new(0, 0), &text).unwrap();
        assert_eq!(storage.line_count(), 2000);
        assert_eq!(storage.to_text(), text);

        storage.insert(Position::new(1000, 10), "EDIT").unwrap();
        storage
            .delete(Range::new(
                Position::new(1000, 10),
                Position::new(1000, 14),
            ))
            .unwrap();

        assert_eq!(storage.to_text(), text);
    }
}
