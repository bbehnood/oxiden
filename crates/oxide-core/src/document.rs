use std::path::PathBuf;

use oxide_buffer::{Buffer, Position, Range, Result, TextStorage};

pub struct Document<S: TextStorage> {
    buffer: Buffer<S>,
    path: Option<PathBuf>,
    dirty: bool,
}

impl<S: TextStorage> Document<S> {
    pub fn new(buffer: Buffer<S>) -> Self {
        Self { buffer, path: None, dirty: false }
    }

    pub fn buffer(&self) -> &Buffer<S> {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer<S> {
        &mut self.buffer
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn insert(&mut self, pos: Position, text: &str) -> Result<()> {
        self.buffer.insert(pos, text)?;
        self.dirty = true;
        Ok(())
    }

    pub fn delete(&mut self, range: Range) -> Result<()> {
        self.buffer.delete(range)?;
        self.dirty = true;
        Ok(())
    }
}
