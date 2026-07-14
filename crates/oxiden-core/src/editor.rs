//! [`Editor`]: applies [`Command`]s to a [`Document`] while keeping the
//! [`Cursor`] consistent with the result of each edit.

use oxiden_buffer::{Position, Range, Result, TextStorage};

use crate::{Command, Cursor, Document};

/// Combines a [`Document`] and a [`Cursor`], and is the single place that
/// knows how each [`Command`] should affect both.
///
/// Keeping this logic in one place (rather than, say, letting the terminal
/// front end move the cursor itself after an edit) is what lets
/// `oxiden-tui` stay a thin translation layer: it only has to turn key
/// presses into `Command`s, not reimplement cursor bookkeeping for every
/// edit operation.
pub struct Editor<S: TextStorage> {
    document: Document<S>,
    cursor: Cursor,
}

impl<S: TextStorage> Editor<S> {
    /// Wraps `document` with a cursor starting at the origin (0, 0).
    pub fn new(document: Document<S>) -> Self {
        Self { document, cursor: Cursor::new() }
    }

    /// The cursor's current state.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Read-only access to the document being edited.
    pub fn document(&self) -> &Document<S> {
        &self.document
    }

    /// Mutable access to the document, for operations outside the
    /// `Command` move_to (e.g. saving).
    pub fn document_mut(&mut self) -> &mut Document<S> {
        &mut self.document
    }

    /// Applies `command` to the document and updates the cursor to match.
    ///
    /// On a buffer error (e.g. a `DeleteRange` with an out-of-bounds
    /// position), the document is left unchanged and the cursor is not
    /// moved.
    pub fn execute(&mut self, command: Command) -> Result<()> {
        match command {
            Command::MoveTo { position, vertical } => {
                // A cursor move with no edit means whatever comes next
                // shouldn't merge into the previous undo group (e.g.
                // typing, moving away, then typing again should undo as
                // two separate steps).
                self.document.break_undo_group();

                let buffer = self.document.buffer();

                let line = position.line.min(buffer.line_count() - 1);

                let column = if vertical {
                    self.cursor
                        .preferred_column()
                        .min(buffer.line_len(line).unwrap_or(0))
                } else {
                    position.column.min(buffer.line_len(line).unwrap_or(0))
                };

                if vertical {
                    self.cursor.move_vertical_to(Position::new(line, column));
                } else {
                    self.cursor.move_to(Position::new(line, column));
                }
            }

            Command::Insert(ch) => {
                let pos = self.cursor.position();

                self.document.insert(pos, &ch.to_string())?;

                self.cursor.move_to(Position::new(pos.line, pos.column + 1));
            }

            Command::InsertText(text) => {
                let pos = self.cursor.position();

                self.document.insert(pos, &text)?;

                // Figure out where the cursor should land after the
                // insert: if `text` had no newlines, it just advances
                // along the current line by however many characters were
                // inserted; otherwise it ends up on the new last line,
                // at the length of that line's inserted content.
                let parts: Vec<&str> = text.split('\n').collect();

                if parts.len() == 1 {
                    self.cursor.move_to(Position::new(
                        pos.line,
                        pos.column + parts[0].chars().count(),
                    ));
                } else {
                    self.cursor.move_to(Position::new(
                        pos.line + parts.len() - 1,
                        parts.last().unwrap().chars().count(),
                    ));
                }
            }

            Command::Backspace => self.backspace()?,

            Command::Delete => self.delete()?,

            Command::DeleteRange(mut range) => {
                // Normalize direction so the cursor consistently ends up
                // at the earlier endpoint, regardless of how the caller
                // built the range (e.g. a right-to-left drag selection).
                if range.start > range.end {
                    std::mem::swap(&mut range.start, &mut range.end);
                }

                self.document.delete(range)?;
                self.cursor.move_to(range.start);
            }

            Command::NewLine => {
                let pos = self.cursor.position();

                self.document.insert(pos, "\n")?;

                self.cursor.move_to(Position::new(pos.line + 1, 0));
            }

            Command::Undo => {
                if let Some(pos) = self.document.undo()? {
                    self.cursor.move_to(pos);
                }
            }

            Command::Redo => {
                if let Some(pos) = self.document.redo()? {
                    self.cursor.move_to(pos);
                }
            }
        }

        Ok(())
    }

    /// Deletes the character before the cursor.
    ///
    /// At the very start of the document this is a no-op. At the start of
    /// any other line, it removes the newline and joins with the previous
    /// line, placing the cursor at the previous line's original end.
    /// Otherwise it just removes the preceding character on the same line.
    fn backspace(&mut self) -> Result<()> {
        let pos = self.cursor.position();

        if pos.line == 0 && pos.column == 0 {
            return Ok(());
        }

        if pos.column > 0 {
            let start = Position::new(pos.line, pos.column - 1);

            self.document.delete(Range::new(start, pos))?;

            self.cursor.move_to(start);

            return Ok(());
        }

        // At column 0 of a non-first line: join with the previous line by
        // deleting the newline between them. The previous line is
        // guaranteed to exist because `pos.line > 0` here.
        let previous_line = pos.line - 1;

        let previous_len = self
            .document
            .buffer()
            .line_len(previous_line)
            .expect("previous line must exist");

        let start = Position::new(previous_line, previous_len);

        self.document.delete(Range::new(start, pos))?;

        self.cursor.move_to(start);

        Ok(())
    }

    /// Deletes the character at (i.e. immediately after) the cursor,
    /// without moving the cursor.
    ///
    /// At the very end of the document this is a no-op. At the end of any
    /// other line, it removes the newline and joins with the next line.
    /// Otherwise it just removes the following character on the same line.
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
}
