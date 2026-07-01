use crate::{BufferError, Position, Range, Result, TextStorage};

pub struct VecStorage {
    lines: Vec<String>,
}

impl VecStorage {
    pub fn new() -> Self {
        Self { lines: vec![String::new()] }
    }

    fn validate_position(&self, pos: Position) -> Result<()> {
        let Some(line) = self.lines.get(pos.line) else {
            return Err(BufferError::InvalidPosition);
        };

        if pos.column > line.chars().count() {
            return Err(BufferError::InvalidPosition);
        };

        Ok(())
    }

    fn byte_index(&self, line: usize, column: usize) -> usize {
        self.lines[line]
            .char_indices()
            .nth(column)
            .map(|(i, _)| i)
            .unwrap_or(self.lines[line].len())
    }

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

    fn len_chars(&self) -> usize {
        let chars: usize = self.lines.iter().map(|l| l.chars().count()).sum();

        chars + self.lines.len().saturating_sub(1)
    }

    fn insert(&mut self, pos: Position, text: &str) -> Result<()> {
        self.validate_position(pos)?;

        let byte = self.byte_index(pos.line, pos.column);

        let tail = self.lines[pos.line].split_off(byte);

        let parts: Vec<&str> = text.split('\n').collect();

        self.lines[pos.line].push_str(parts[0]);

        if parts.len() == 1 {
            self.lines[pos.line].push_str(&tail);
            return Ok(());
        }

        let mut insert_at = pos.line + 1;

        for part in &parts[1..parts.len() - 1] {
            self.lines.insert(insert_at, (*part).to_string());
            insert_at += 1;
        }

        let mut last = parts.last().unwrap().to_string();
        last.push_str(&tail);

        self.lines.insert(insert_at, last);

        Ok(())
    }

    fn delete(&mut self, mut range: Range) -> Result<()> {
        self.validate_position(range.start)?;
        self.validate_position(range.end)?;

        if range.start > range.end {
            std::mem::swap(&mut range.start, &mut range.end);
        }

        if range.start.line == range.end.line {
            let start = self.byte_index_at(range.start);
            let end = self.byte_index_at(range.end);

            self.lines[range.start.line].replace_range(start..end, "");

            return Ok(());
        }

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
