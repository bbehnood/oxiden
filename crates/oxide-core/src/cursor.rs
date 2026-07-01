use oxide_buffer::Position;

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
