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

#[cfg(test)]
mod tests {
    use oxide_buffer::VecStorage;

    use super::*;

    fn document() -> Document<VecStorage> {
        Document::new(Buffer::new(VecStorage::new()))
    }

    #[test]
    fn new_document_is_clean_and_has_no_path() {
        let document = document();

        assert!(!document.is_dirty());
        assert_eq!(document.path(), None);
    }

    #[test]
    fn insert_marks_document_dirty() {
        let mut document = document();

        document.insert(Position::new(0, 0), "hello").unwrap();

        assert!(document.is_dirty());
    }

    #[test]
    fn delete_marks_document_dirty() {
        let mut document = document();

        document.insert(Position::new(0, 0), "hello").unwrap();
        document
            .delete(Range::new(Position::new(0, 0), Position::new(0, 1)))
            .unwrap();

        assert!(document.is_dirty());
    }

    #[test]
    fn insert_with_invalid_position_returns_error_and_does_not_panic() {
        let mut document = document();

        let result = document.insert(Position::new(5, 0), "hello");

        assert!(result.is_err());
    }

    #[test]
    fn buffer_and_buffer_mut_reflect_edits() {
        let mut document = document();

        document.buffer_mut().insert(Position::new(0, 0), "hi").unwrap();

        assert_eq!(document.buffer().line(0), Some("hi"));
    }
}
