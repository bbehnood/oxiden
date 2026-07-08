//! Translation from `crossterm` key events into editor-level actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use oxiden_buffer::{Buffer, Position, TextStorage};
use oxiden_core::Command;

/// The result of interpreting a key press: either an edit to apply, a
/// cursor motion to resolve, or a UI-level request (save/quit/nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Apply this command to the editor.
    Edit(Command),
    /// Move the cursor; resolved against the buffer via
    /// [`motion_target`] since the target position depends on line
    /// lengths, not just the key pressed.
    Move(Move),
    /// Save the current document.
    Save,
    /// Save the current document to a new, user-chosen path
    SaveAs,
    /// Quit the application.
    Quit,
    /// The key press doesn't map to anything (e.g. an unhandled
    /// modifier combination).
    Noop,
}

/// A cursor movement requested by the user, independent of the buffer's
/// actual content. Resolving this into an absolute [`Position`] is
/// [`motion_target`]'s job, since e.g. "Right" at the end of a line means
/// "wrap to the next line" only if a next line exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
    LineStart,
    LineEnd,
}

/// Maps a raw key event to an [`Action`].
///
/// Plain characters (no modifiers, or Shift only — needed for uppercase
/// letters and shifted symbols) become [`Command::Insert`]; everything
/// else is either a fixed editing command, a cursor motion, or one of
/// the application-level shortcuts (Ctrl+Q to quit, Ctrl+S to save,
/// Ctrl+Shift+S to save as). Any combination not covered here maps to
/// [`Action::Noop`].
///
/// Note: Ctrl+Shift+S is only distinguishable from plain Ctrl+S on
/// terminals that report the Shift modifier alongside Ctrl for letter
/// keys (see [`crate::Terminal`], which opts into this where
/// supported). On terminals that don't, this arm never matches and
/// Ctrl+Shift+S falls through to plain `Save` instead.
pub fn map_key(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => Action::Save,
        (KeyCode::F(2), _) => Action::SaveAs,

        (KeyCode::Char(c), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
            Action::Edit(Command::Insert(c))
        }

        (KeyCode::Tab, _) => Action::Edit(Command::Insert('\t')),
        (KeyCode::Enter, _) => Action::Edit(Command::NewLine),
        (KeyCode::Backspace, _) => Action::Edit(Command::Backspace),
        (KeyCode::Delete, _) => Action::Edit(Command::Delete),

        (KeyCode::Left, _) => Action::Move(Move::Left),
        (KeyCode::Right, _) => Action::Move(Move::Right),
        (KeyCode::Up, _) => Action::Move(Move::Up),
        (KeyCode::Down, _) => Action::Move(Move::Down),
        (KeyCode::Home, _) => Action::Move(Move::LineStart),
        (KeyCode::End, _) => Action::Move(Move::LineEnd),

        _ => Action::Noop,
    }
}

/// Resolves a [`Move`] against `buffer`'s actual content, starting from
/// `cursor`, and returns the resulting absolute position.
///
/// Horizontal motion (`Left`/`Right`) wraps across line boundaries: moving
/// left from column 0 goes to the end of the previous line, and moving
/// right from the end of a line goes to the start of the next one (unless
/// there's no such line, in which case the cursor stays put). Vertical
/// motion (`Up`/`Down`) preserves the column where possible but clamps it
/// to the target line's length, so moving through a shorter line doesn't
/// leave the cursor past its end.
pub fn motion_target<S: TextStorage>(
    buffer: &Buffer<S>,
    cursor: Position,
    movement: Move,
) -> Position {
    match movement {
        Move::Left => {
            if cursor.column > 0 {
                Position::new(cursor.line, cursor.column - 1)
            } else if cursor.line > 0 {
                let previous = cursor.line - 1;
                let len = buffer.line_len(previous).unwrap_or(0);

                Position::new(previous, len)
            } else {
                cursor
            }
        }

        Move::Right => {
            let len = buffer.line_len(cursor.line).unwrap_or(0);

            if cursor.column < len {
                Position::new(cursor.line, cursor.column + 1)
            } else if !buffer.is_last_line(cursor.line) {
                Position::new(cursor.line + 1, 0)
            } else {
                cursor
            }
        }

        Move::Up => {
            if cursor.line == 0 {
                cursor
            } else {
                let line = cursor.line - 1;
                let len = buffer.line_len(line).unwrap_or(0);

                Position::new(line, cursor.column.min(len))
            }
        }

        Move::Down => {
            if buffer.is_last_line(cursor.line) {
                cursor
            } else {
                let line = cursor.line + 1;
                let len = buffer.line_len(line).unwrap_or(0);

                Position::new(line, cursor.column.min(len))
            }
        }

        Move::LineStart => Position::new(cursor.line, 0),

        Move::LineEnd => {
            let len = buffer.line_len(cursor.line).unwrap_or(0);

            Position::new(cursor.line, len)
        }
    }
}

#[cfg(test)]
mod tests {
    use oxiden_buffer::VecStorage;

    use super::*;

    fn buffer(lines: &[&str]) -> Buffer<VecStorage> {
        let mut buffer = Buffer::new(VecStorage::new());

        buffer.insert(Position::new(0, 0), &lines.join("\n")).unwrap();

        buffer
    }

    #[test]
    fn right_wraps_to_next_line() {
        let buffer = buffer(&["ab", "cd"]);

        let target = motion_target(&buffer, Position::new(0, 2), Move::Right);

        assert_eq!(target, Position::new(1, 0));
    }

    #[test]
    fn right_at_end_of_buffer_stays_put() {
        let buffer = buffer(&["ab"]);

        let target = motion_target(&buffer, Position::new(0, 2), Move::Right);

        assert_eq!(target, Position::new(0, 2));
    }

    #[test]
    fn left_wraps_to_previous_line() {
        let buffer = buffer(&["ab", "cd"]);

        let target = motion_target(&buffer, Position::new(1, 0), Move::Left);

        assert_eq!(target, Position::new(0, 2));
    }

    #[test]
    fn down_clamps_to_shorter_line() {
        let buffer = buffer(&["abcdef", "gh"]);

        let target = motion_target(&buffer, Position::new(0, 5), Move::Down);

        assert_eq!(target, Position::new(1, 2));
    }

    #[test]
    fn up_at_first_line_stays_put() {
        let buffer = buffer(&["abc"]);

        let target = motion_target(&buffer, Position::new(0, 1), Move::Up);

        assert_eq!(target, Position::new(0, 1));
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn ctrl_s_saves() {
        let action = map_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));

        assert_eq!(action, Action::Save);
    }

    #[test]
    fn f2_saves_as() {
        let action = map_key(key(
            KeyCode::F(2),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));

        assert_eq!(action, Action::SaveAs);
    }

    #[test]
    fn plain_s_inserts_character() {
        let action = map_key(key(KeyCode::Char('s'), KeyModifiers::NONE));

        assert_eq!(action, Action::Edit(Command::Insert('s')));
    }
}
