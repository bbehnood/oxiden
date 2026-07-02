use oxide_buffer::Position;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cursor {
    position: Position,
}

impl Cursor {
    pub fn new() -> Self {
        Self { position: Position::new(0, 0) }
    }

    pub fn position(&self) -> Position {
        self.position
    }

    pub fn set(&mut self, pos: Position) {
        self.position = pos;
    }

    pub fn line(&self) -> usize {
        self.position.line
    }

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
