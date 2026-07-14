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
    preferred_column: usize,
}

impl Cursor {
    /// Creates a cursor at the start of the document (line 0, column 0).
    pub fn new() -> Self {
        Self { position: Position::new(0, 0), preferred_column: 0 }
    }

    /// Returns the cursor's current position.
    pub fn position(&self) -> Position {
        self.position
    }

    /// Returns the cursor's preferred column.
    pub fn preferred_column(&self) -> usize {
        self.preferred_column
    }
    /// Moves the cursor to `pos`, updating the preferred column.
    ///
    /// This should be used for movements that intentionally establish a new
    /// horizontal position, such as left/right movement, mouse clicks, or
    /// "go to" commands.
    pub fn move_to(&mut self, pos: Position) {
        self.position = pos;
        self.preferred_column = pos.column;
    }

    /// Moves the cursor vertically without changing the preferred column.
    ///
    /// This is used for vertical navigation, allowing the cursor to return to
    /// its original column when moving onto longer lines.
    pub(crate) fn move_vertical_to(&mut self, pos: Position) {
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
    fn move_to_updates_position_line_and_column() {
        let mut cursor = Cursor::new();

        cursor.move_to(Position::new(3, 7));

        assert_eq!(cursor.position(), Position::new(3, 7));
        assert_eq!(cursor.line(), 3);
        assert_eq!(cursor.column(), 7);
    }

    #[test]
    fn move_to_updates_preferred_column() {
        let mut cursor = Cursor::new();

        cursor.move_to(Position::new(3, 7));

        assert_eq!(cursor.preferred_column(), 7);
    }

    #[test]
    fn move_vertical_to_preserves_preferred_column() {
        let mut cursor = Cursor::new();

        cursor.move_to(Position::new(0, 8));
        cursor.move_vertical_to(Position::new(1, 3));

        assert_eq!(cursor.position(), Position::new(1, 3));
        assert_eq!(cursor.preferred_column(), 8);
    }
}
