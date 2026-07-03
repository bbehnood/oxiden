use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use oxide_buffer::{Buffer, Position, TextStorage};
use oxide_core::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Edit(Command),
    Move(Move),
    Save,
    Quit,
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
    LineStart,
    LineEnd,
}

pub fn map_key(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => Action::Save,

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
    use oxide_buffer::VecStorage;

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
}
