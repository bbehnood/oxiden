use oxiden_buffer::Position;

/// The user's current location in a document.
///
/// `Cursor` is intentionally dumb: it just holds a [`Position`] and does
/// not validate it against any buffer. Keeping the cursor consistent with
/// the document's actual content (e.g. not left pointing past the end of a
/// line after a delete) is [`crate::Editor`]'s job, since only the editor
/// has access to both the cursor and the buffer at the same time.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cursor {
    position: Position,
}

impl Cursor {
    /// Creates a cursor at the start of the document (line 0, column 0).
    pub fn new() -> Self {
        Self { position: Position::new(0, 0) }
    }

    /// Returns the cursor's current position.
    pub fn position(&self) -> Position {
        self.position
    }

    /// Moves the cursor to `pos` unconditionally (no bounds checking).
    pub fn set(&mut self, pos: Position) {
        self.position = pos;
    }

    /// Shorthand for `position().line`.
    pub fn line(&self) -> usize {
        self.position.line
    }

    /// Shorthand for `position().column`.
    pub fn column(&self) -> usize {
        self.position.column
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_origin() {
        let cursor = Cursor::new();

        assert_eq!(cursor.position(), Position::new(0, 0));
        assert_eq!(cursor.line(), 0);
        assert_eq!(cursor.column(), 0);
    }

    #[test]
    fn default_matches_new() {
        assert_eq!(Cursor::default(), Cursor::new());
    }

    #[test]
    fn set_updates_position_line_and_column() {
        let mut cursor = Cursor::new();

        cursor.set(Position::new(3, 7));

        assert_eq!(cursor.position(), Position::new(3, 7));
        assert_eq!(cursor.line(), 3);
        assert_eq!(cursor.column(), 7);
    }
}
