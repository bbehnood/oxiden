use oxide_buffer::{Position, Range, Result, TextStorage};

use crate::{Command, Cursor, Document};

pub struct Editor<S: TextStorage> {
    document: Document<S>,
    cursor: Cursor,
}

impl<S: TextStorage> Editor<S> {
    pub fn new(document: Document<S>) -> Self {
        Self { document, cursor: Cursor::new() }
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn document(&self) -> &Document<S> {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document<S> {
        &mut self.document
    }

    fn backspace(&mut self) -> Result<()> {
        let pos = self.cursor.position();

        if pos.line == 0 && pos.column == 0 {
            return Ok(());
        }

        if pos.column > 0 {
            let start = Position::new(pos.line, pos.column - 1);

            self.document.delete(Range::new(start, pos))?;

            self.cursor.set(start);

            return Ok(());
        }

        let previous_line = pos.line - 1;

        let previous_len = self
            .document
            .buffer()
            .line_len(previous_line)
            .expect("previous line must exist");

        let start = Position::new(previous_line, previous_len);

        self.document.delete(Range::new(start, pos))?;

        self.cursor.set(start);

        Ok(())
    }

    fn delete(&mut self) -> Result<()> {
        let pos = self.cursor.position();

        let buffer = self.document.buffer();

        if buffer.is_last_line(pos.line) && buffer.is_last_column(pos) {
            return Ok(());
        }

        let end = if buffer.is_last_column(pos) {
            Position::new(pos.line + 1, 0)
        } else {
            Position::new(pos.line, pos.column + 1)
        };

        self.document.delete(Range::new(pos, end))
    }

    pub fn execute(&mut self, command: Command) -> Result<()> {
        match command {
            Command::MoveTo(pos) => self.cursor.set(pos),

            Command::Insert(ch) => {
                let pos = self.cursor.position();

                self.document.insert(pos, &ch.to_string())?;

                self.cursor.set(Position::new(pos.line, pos.column + 1));
            }

            Command::InsertText(text) => {
                let pos = self.cursor.position();

                self.document.insert(pos, &text)?;

                let parts: Vec<&str> = text.split('\n').collect();

                if parts.len() == 1 {
                    self.cursor.set(Position::new(
                        pos.line,
                        pos.column + parts[0].chars().count(),
                    ));
                } else {
                    self.cursor.set(Position::new(
                        pos.line + parts.len() - 1,
                        parts.last().unwrap().chars().count(),
                    ));
                }
            }

            Command::Backspace => self.backspace()?,

            Command::Delete => self.delete()?,

            Command::DeleteRange(mut range) => {
                if range.start > range.end {
                    std::mem::swap(&mut range.start, &mut range.end);
                }

                self.document.delete(range)?;
                self.cursor.set(range.start);
            }

            Command::NewLine => {
                let pos = self.cursor.position();

                self.document.insert(pos, "\n")?;

                self.cursor.set(Position::new(pos.line + 1, 0));
            }
        }

        Ok(())
    }
}
