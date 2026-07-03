use std::fs;
use std::path::{Path, PathBuf};

use crate::{DocumentError, Result};
use oxide_buffer::{Buffer, Position, Range, TextStorage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

pub struct Document<S: TextStorage> {
    buffer: Buffer<S>,
    path: Option<PathBuf>,
    dirty: bool,
    line_ending: LineEnding,
    trailing_newline: bool,
}

impl LineEnding {
    fn detect(text: &str) -> Self {
        if text.contains("\r\n") { LineEnding::CrLf } else { LineEnding::Lf }
    }

    fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }
}

impl<S: TextStorage> Document<S> {
    pub fn new(buffer: Buffer<S>) -> Self {
        Self {
            buffer,
            path: None,
            dirty: false,
            line_ending: LineEnding::Lf,
            trailing_newline: true,
        }
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

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn insert(
        &mut self,
        pos: Position,
        text: &str,
    ) -> oxide_buffer::Result<()> {
        self.buffer.insert(pos, text)?;
        self.dirty = true;
        Ok(())
    }

    pub fn delete(&mut self, range: Range) -> oxide_buffer::Result<()> {
        self.buffer.delete(range)?;
        self.dirty = true;
        Ok(())
    }
}

impl<S: TextStorage + Default> Document<S> {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();

        let raw = fs::read_to_string(&path)?;

        let line_ending = LineEnding::detect(&raw);

        let mut content = raw.replace("\r\n", "\n");
        let trailing_newline = content.ends_with('\n');

        if trailing_newline {
            content.pop();
        }

        let mut buffer = Buffer::new(S::default());

        if !content.is_empty() {
            buffer.insert(Position::new(0, 0), &content)?;
        }

        Ok(Self {
            buffer,
            path: Some(path),
            dirty: false,
            line_ending,
            trailing_newline,
        })
    }

    pub fn new_at(path: impl Into<PathBuf>) -> Self {
        Self { path: Some(path.into()), ..Self::new(Buffer::new(S::default())) }
    }
}

impl<S: TextStorage> Document<S> {
    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or(DocumentError::NoPath)?;

        self.write_to(&path)?;
        self.dirty = false;

        Ok(())
    }

    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();

        self.write_to(&path)?;

        self.path = Some(path);
        self.dirty = false;

        Ok(())
    }

    fn write_to(&self, path: &Path) -> Result<()> {
        let mut content = self.buffer.to_text();

        if self.line_ending == LineEnding::CrLf {
            content = content.replace('\n', "\r\n");
        }

        if self.trailing_newline {
            content.push_str(self.line_ending.as_str());
        }

        fs::write(path, content)?;

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

    // ===== File I/O =====

    /// Returns a path in the system temp dir that's unique to this test
    /// process/thread, and removes any leftover file from a previous run.
    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("oxide-core-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn open_reads_existing_file() {
        let path = temp_path("open_reads_existing_file");
        fs::write(&path, "hello\nworld").unwrap();

        let document = Document::<VecStorage>::open(&path).unwrap();

        assert_eq!(document.buffer().line(0), Some("hello"));
        assert_eq!(document.buffer().line(1), Some("world"));
        assert_eq!(document.path(), Some(&path));
        assert!(!document.is_dirty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_missing_file_returns_error() {
        let path = temp_path("open_missing_file_returns_error");

        let result = Document::<VecStorage>::open(&path);

        assert!(matches!(result, Err(DocumentError::Io(_))));
    }

    #[test]
    fn new_at_sets_path_without_touching_disk() {
        let path = temp_path("new_at_sets_path_without_touching_disk");

        let document = Document::<VecStorage>::new_at(&path);

        assert_eq!(document.path(), Some(&path));
        assert!(!document.is_dirty());
        assert!(!path.exists());
    }

    #[test]
    fn new_at_then_save_creates_the_file() {
        let path = temp_path("new_at_then_save_creates_the_file");

        let mut document = Document::<VecStorage>::new_at(&path);
        document.insert(Position::new(0, 0), "hello").unwrap();
        document.save().unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_without_path_returns_no_path_error() {
        let mut document = document();

        let result = document.save();

        assert!(matches!(result, Err(DocumentError::NoPath)));
    }

    #[test]
    fn save_as_writes_file_and_sets_path() {
        let path = temp_path("save_as_writes_file_and_sets_path");
        let mut document = document();

        document.insert(Position::new(0, 0), "hello\nworld").unwrap();
        document.save_as(&path).unwrap();

        assert_eq!(document.path(), Some(&path));
        assert!(!document.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\nworld\n");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_writes_to_existing_path_and_clears_dirty() {
        let path = temp_path("save_writes_to_existing_path_and_clears_dirty");
        fs::write(&path, "original\n").unwrap();

        let mut document = Document::<VecStorage>::open(&path).unwrap();

        document
            .insert(
                Position::new(0, document.buffer().line_len(0).unwrap()),
                "!",
            )
            .unwrap();

        assert!(document.is_dirty());

        document.save().unwrap();

        assert!(!document.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original!\n");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_save_roundtrip_preserves_missing_trailing_newline() {
        let path =
            temp_path("open_save_roundtrip_preserves_missing_trailing_newline");
        fs::write(&path, "no trailing newline").unwrap();

        let mut document = Document::<VecStorage>::open(&path).unwrap();
        document.save_as(&path).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "no trailing newline");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_save_roundtrip_preserves_crlf() {
        let path = temp_path("open_save_roundtrip_preserves_crlf");
        fs::write(&path, "one\r\ntwo\r\nthree\r\n").unwrap();

        let mut document = Document::<VecStorage>::open(&path).unwrap();

        assert_eq!(document.line_ending(), LineEnding::CrLf);
        assert_eq!(document.buffer().line(1), Some("two"));

        document.save_as(&path).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "one\r\ntwo\r\nthree\r\n"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn open_empty_file() {
        let path = temp_path("open_empty_file");
        fs::write(&path, "").unwrap();

        let document = Document::<VecStorage>::open(&path).unwrap();

        assert_eq!(document.buffer().line_count(), 1);
        assert_eq!(document.buffer().line(0), Some(""));

        let _ = fs::remove_file(&path);
    }
}
