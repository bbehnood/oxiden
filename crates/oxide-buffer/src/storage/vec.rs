use crate::{BufferError, Position, Range, Result, TextStorage};

pub struct VecStorage {
    lines: Vec<String>,
}

impl VecStorage {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
        }
    }
}

impl Default for VecStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl TextStorage for VecStorage {
    fn line(&self, index: usize) -> Option<String> {
        self.lines.get(index).cloned()
    }

    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn len_chars(&self) -> usize {
        self.lines.iter().map(|l| l.chars().count()).sum()
    }

    fn insert(&mut self, _pos: Position, _text: &str) -> Result<()> {
        Err(BufferError::InvalidPosition)
    }

    fn delete(&mut self, _range: Range) -> Result<()> {
        Err(BufferError::InvalidRange)
    }
}
