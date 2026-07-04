use oxiden_buffer::{Position, Range};

/// A single editing action a UI can ask an [`crate::Editor`] to perform.
///
/// `Command` is the boundary between input handling and editing logic: a
/// front end (e.g. `oxiden-tui`) translates raw input events into
/// `Command`s, and [`crate::Editor::execute`] applies them, updating both
/// the document and the cursor as appropriate. This keeps `oxiden-core`
/// free of any knowledge of keyboards, mice, or terminal escape codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Move the cursor to an absolute position, without editing text.
    MoveTo(Position),

    /// Insert a single character at the cursor.
    Insert(char),
    /// Insert a (possibly multi-line) string at the cursor.
    InsertText(String),

    /// Delete the character before the cursor, joining with the previous
    /// line if at column 0.
    Backspace,
    /// Delete the character at/after the cursor, joining with the next
    /// line if at the end of the current one.
    Delete,

    /// Delete an explicit range of text (e.g. a selection), regardless of
    /// where the cursor currently is. The cursor is moved to the earlier
    /// endpoint of the range after deletion.
    DeleteRange(Range),

    /// Insert a newline at the cursor, splitting the current line.
    NewLine,
}
