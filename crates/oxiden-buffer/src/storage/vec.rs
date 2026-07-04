//! A simple [`TextStorage`] backend that keeps each line as a separate
//! `String` inside a `Vec`.
//!
//! This is the "obvious" representation: easy to reason about and fast
//! enough for everyday editing, at the cost of O(line length) inserts near
//! the start of a long line and O(line count) for operations that shift
//! many lines (e.g. deleting across a large range). It's a fine default for
//! a small editor; a rope or piece table would scale better for very large
//! files.

use crate::{BufferError, Position, Range, Result, TextStorage};

/// [`TextStorage`] implementation backed by a `Vec<String>`, one entry per
/// line.
///
/// Invariant: `lines` always contains at least one element. A brand-new
/// document is represented as a single empty line, never as zero lines.
pub struct VecStorage {
    lines: Vec<String>,
}

impl VecStorage {
    /// Creates an empty document: one line containing no text.
    pub fn new() -> Self {
        Self { lines: vec![String::new()] }
    }

    /// Checks that `pos` refers to an existing line and a column that is
    /// within (or exactly at the end of) that line's character count.
    fn validate_position(&self, pos: Position) -> Result<()> {
        let Some(line) = self.lines.get(pos.line) else {
            return Err(BufferError::InvalidPosition);
        };

        if pos.column > line.chars().count() {
            return Err(BufferError::InvalidPosition);
        };

        Ok(())
    }

    /// Converts a character `column` on `line` into a byte offset into that
    /// line's underlying `String`, so it can be used with byte-oriented
    /// `String` APIs like `split_off`/`replace_range`.
    ///
    /// A `column` equal to the line's character length maps to the line's
    /// byte length (i.e. "end of line" is a valid target).
    fn byte_index(&self, line: usize, column: usize) -> usize {
        self.lines[line]
            .char_indices()
            .nth(column)
            .map(|(i, _)| i)
            .unwrap_or(self.lines[line].len())
    }

    /// Convenience wrapper around [`Self::byte_index`] for a [`Position`].
    fn byte_index_at(&self, pos: Position) -> usize {
        self.byte_index(pos.line, pos.column)
    }
}

impl Default for VecStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl TextStorage for VecStorage {
    type Line<'a> = &'a str;

    fn line(&self, index: usize) -> Option<Self::Line<'_>> {
        self.lines.get(index).map(String::as_str)
    }

    fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Sums each line's character count, then adds one implicit `\n` for
    /// every line boundary (`lines.len() - 1` of them; `saturating_sub`
    /// keeps this at 0 rather than underflowing if `lines` were ever
    /// empty).
    fn len_chars(&self) -> usize {
        let chars: usize = self.lines.iter().map(|l| l.chars().count()).sum();

        chars + self.lines.len().saturating_sub(1)
    }

    fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Splits the target line at `pos`, splices in `text` (which may itself
    /// span multiple lines), and reattaches the original tail of the line
    /// after the last inserted line.
    ///
    /// Example: inserting `"X\nY"` at column 2 of `"abcd"` produces
    /// `["abX", "Ycd"]` — the text before the cursor (`"ab"`) and after it
    /// (`"cd"`) are preserved on the first and last resulting lines
    /// respectively, with any middle lines of `text` inserted verbatim in
    /// between.
    fn insert(&mut self, pos: Position, text: &str) -> Result<()> {
        self.validate_position(pos)?;

        let byte = self.byte_index(pos.line, pos.column);

        // Cut the target line in two at the insertion point; `tail` is
        // everything after the cursor, which needs to end up after
        // whatever we insert.
        let tail = self.lines[pos.line].split_off(byte);

        let parts: Vec<&str> = text.split('\n').collect();

        // The first part of the inserted text always lands on the
        // (now-truncated) original line.
        self.lines[pos.line].push_str(parts[0]);

        if parts.len() == 1 {
            // No newlines in `text`: just reattach the tail and we're done.
            self.lines[pos.line].push_str(&tail);
            return Ok(());
        }

        // `text` contains newlines: insert each middle part as its own new
        // line after `pos.line`.
        let mut insert_at = pos.line + 1;

        for part in &parts[1..parts.len() - 1] {
            self.lines.insert(insert_at, (*part).to_string());
            insert_at += 1;
        }

        // The last part of `text` gets the original tail appended, since
        // that's where the old line's remainder now belongs.
        let mut last = parts.last().unwrap().to_string();
        last.push_str(&tail);

        self.lines.insert(insert_at, last);

        Ok(())
    }

    /// Removes the text spanned by `range`, joining lines if the range
    /// crosses a line boundary.
    fn delete(&mut self, mut range: Range) -> Result<()> {
        self.validate_position(range.start)?;
        self.validate_position(range.end)?;

        // Callers may hand us a range built from an end-to-start drag, so
        // normalize direction before doing anything else.
        if range.start > range.end {
            std::mem::swap(&mut range.start, &mut range.end);
        }

        if range.start.line == range.end.line {
            // Fast path: deletion is entirely within one line, so a plain
            // byte-range removal suffices.
            let start = self.byte_index_at(range.start);
            let end = self.byte_index_at(range.end);

            self.lines[range.start.line].replace_range(start..end, "");

            return Ok(());
        }

        // Cross-line deletion: keep the part of the first line before
        // `range.start` and the part of the last line after `range.end`,
        // then collapse every line in between (including the first and
        // last) into a single joined line.
        let first_prefix = {
            let start = self.byte_index_at(range.start);
            self.lines[range.start.line][..start].to_owned()
        };

        let last_suffix = {
            let end = self.byte_index_at(range.end);
            self.lines[range.end.line][end..].to_owned()
        };

        self.lines.splice(
            range.start.line..=range.end.line,
            std::iter::once(first_prefix + &last_suffix),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage(lines: &[&str]) -> VecStorage {
        VecStorage { lines: lines.iter().map(ToString::to_string).collect() }
    }

    // ===== Constructor =====

    #[test]
    fn new_has_one_empty_line() {
        let storage = VecStorage::new();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some(""));
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
        let storage = VecStorage::new();

        assert_eq!(storage.to_text(), "");
    }

    #[test]
    fn to_text_preserves_blank_lines() {
        let storage = storage(&["a", "", "b"]);

        assert_eq!(storage.to_text(), "a\n\nb");
    }

    #[test]
    fn to_text_roundtrips_with_insert() {
        let mut storage = storage(&[""]);

        let original = "line one\nline two\nline three";

        storage.insert(Position::new(0, 0), original).unwrap();

        assert_eq!(storage.to_text(), original);
    }

    // ===== Insert =====

    #[test]
    fn insert_into_empty() {
        let mut storage = storage(&[""]);

        storage.insert(Position::new(0, 0), "Hello").unwrap();

        assert_eq!(storage.line(0), Some("Hello"));
    }

    #[test]
    fn insert_at_beginning() {
        let mut storage = storage(&["World"]);

        storage.insert(Position::new(0, 0), "Hello ").unwrap();

        assert_eq!(storage.line(0), Some("Hello World"));
    }

    #[test]
    fn insert_at_end() {
        let mut storage = storage(&["Hello"]);

        storage.insert(Position::new(0, 5), " World").unwrap();

        assert_eq!(storage.line(0), Some("Hello World"));
    }

    #[test]
    fn insert_in_middle() {
        let mut storage = storage(&["Helo"]);

        storage.insert(Position::new(0, 2), "l").unwrap();

        assert_eq!(storage.line(0), Some("Hello"));
    }

    #[test]
    fn insert_empty_string() {
        let mut storage = storage(&["Hello"]);

        storage.insert(Position::new(0, 2), "").unwrap();

        assert_eq!(storage.line(0), Some("Hello"));
    }

    #[test]
    fn insert_single_newline() {
        let mut storage = storage(&["HelloWorld"]);

        storage.insert(Position::new(0, 5), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some("Hello"));
        assert_eq!(storage.line(1), Some("World"));
    }

    #[test]
    fn insert_multiple_newlines() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 1), "\n123\n456\n").unwrap();

        assert_eq!(storage.line_count(), 4);
        assert_eq!(storage.line(0), Some("a"));
        assert_eq!(storage.line(1), Some("123"));
        assert_eq!(storage.line(2), Some("456"));
        assert_eq!(storage.line(3), Some("bc"));
    }

    #[test]
    fn insert_trailing_newline() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 3), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some("abc"));
        assert_eq!(storage.line(1), Some(""));
    }

    #[test]
    fn insert_leading_newline() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 0), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some(""));
        assert_eq!(storage.line(1), Some("abc"));
    }

    #[test]
    fn insert_unicode() {
        let mut storage = storage(&["😀😁"]);

        storage.insert(Position::new(0, 1), "😂").unwrap();

        assert_eq!(storage.line(0), Some("😀😂😁"));
    }

    // ===== Delete =====

    #[test]
    fn delete_empty_range() {
        let mut storage = storage(&["Hello"]);

        storage
            .delete(Range::new(Position::new(0, 2), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("Hello"));
    }

    #[test]
    fn delete_single_character() {
        let mut storage = storage(&["Hello"]);

        storage
            .delete(Range::new(Position::new(0, 1), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("Hllo"));
    }

    #[test]
    fn delete_middle() {
        let mut storage = storage(&["Hello World"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(0, 6)))
            .unwrap();

        assert_eq!(storage.line(0), Some("HelloWorld"));
    }

    #[test]
    fn delete_newline_between_lines() {
        let mut storage = storage(&["Hello", "World"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(1, 0)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some("HelloWorld"));
    }

    #[test]
    fn delete_across_lines() {
        let mut storage = storage(&["Hello", "Beautiful", "World"]);

        storage
            .delete(Range::new(Position::new(0, 2), Position::new(2, 3)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some("Held"));
    }

    #[test]
    fn delete_everything() {
        let mut storage = storage(&["Hello", "World"]);

        storage
            .delete(Range::new(Position::new(0, 0), Position::new(1, 5)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some(""));
    }

    #[test]
    fn delete_unicode() {
        let mut storage = storage(&["😀😁😂"]);

        storage
            .delete(Range::new(Position::new(0, 1), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("😀😂"));
    }

    #[test]
    fn delete_reversed_range() {
        let mut storage = storage(&["abcdef"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("abf"));
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
}
