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
    FileStart,
    FileEnd,
}

/// Maps a raw key event to an [`Action`].
///
/// Plain characters (no modifiers, or Shift only — needed for uppercase
/// letters and shifted symbols) become [`Command::Insert`]; everything
/// else is either a fixed editing command, a cursor motion, or one of
/// the application-level shortcuts (Ctrl+Q to quit, Ctrl+S to save,
/// F2 to save as). Any combination not covered here maps to
/// [`Action::Noop`].
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
        (KeyCode::PageUp, _) => Action::Move(Move::FileStart),
        (KeyCode::PageDown, _) => Action::Move(Move::FileEnd),

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

        Move::FileStart => Position::new(0, 0),

        Move::FileEnd => {
            let line = buffer.line_count() - 1;
            let len = buffer.line_len(line).unwrap_or(0);

            Position::new(line, len)
        }
    }
}

/// Which way to search for a word boundary in, used by [`word_boundary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

/// The three character classes a word boundary is defined in terms of. A
/// maximal run of same-class characters is a single "token"; a boundary
/// sits at every transition between tokens. This mirrors the classic vim
/// `w`/`b` motions (punctuation is its own token, distinct from words), and
/// treats the break between one line and the next as equivalent to
/// whitespace, so scanning glides over line breaks and blank lines the
/// same way it glides over runs of spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Punctuation,
    Whitespace,
}

fn char_class(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

fn line_chars<S: TextStorage>(buffer: &Buffer<S>, line: usize) -> Vec<char> {
    buffer
        .line(line)
        .map(|line| line.as_ref().chars().collect())
        .unwrap_or_default()
}

/// The class of the character at `pos`, or `None` if `pos` is at/past the
/// end of its line.
fn class_at<S: TextStorage>(
    buffer: &Buffer<S>,
    pos: Position,
) -> Option<CharClass> {
    line_chars(buffer, pos.line).get(pos.column).map(|&c| char_class(c))
}

/// The class of the character immediately before `pos`, or `None` if `pos`
/// is at column 0 (the caller is expected to have already skipped
/// backwards over any preceding whitespace/line breaks, so column 0 here
/// only ever means "start of the buffer").
fn class_before<S: TextStorage>(
    buffer: &Buffer<S>,
    pos: Position,
) -> Option<CharClass> {
    if pos.column == 0 {
        return None;
    }

    line_chars(buffer, pos.line).get(pos.column - 1).map(|&c| char_class(c))
}

/// Advances `pos` one character at a time for as long as `matches` holds
/// for the character there, crossing into the next line if `matches`
/// holds for `'\n'` (used to let a whitespace scan glide over line
/// breaks, while a same-token scan — which never matches `'\n'` — stops
/// at the end of the line instead).
fn advance_while<S: TextStorage>(
    buffer: &Buffer<S>,
    mut pos: Position,
    mut matches: impl FnMut(char) -> bool,
) -> Position {
    loop {
        match line_chars(buffer, pos.line).get(pos.column) {
            Some(&c) if matches(c) => {
                pos = Position::new(pos.line, pos.column + 1);
            }
            Some(_) => return pos,
            None if matches('\n') && !buffer.is_last_line(pos.line) => {
                pos = Position::new(pos.line + 1, 0);
            }
            None => return pos,
        }
    }
}

/// The mirror image of [`advance_while`]: steps `pos` backwards for as
/// long as `matches` holds for the character just behind it, crossing
/// into the previous line's end on the same `'\n'`-matching basis.
fn retreat_while<S: TextStorage>(
    buffer: &Buffer<S>,
    mut pos: Position,
    mut matches: impl FnMut(char) -> bool,
) -> Position {
    loop {
        if pos.column > 0 {
            let c = line_chars(buffer, pos.line)[pos.column - 1];

            if matches(c) {
                pos = Position::new(pos.line, pos.column - 1);
                continue;
            }

            return pos;
        }

        if pos.line == 0 {
            return pos;
        }

        if matches('\n') {
            let previous = pos.line - 1;
            let len = buffer.line_len(previous).unwrap_or(0);

            pos = Position::new(previous, len);
        } else {
            return pos;
        }
    }
}

/// Finds the next word boundary from `cursor` in `direction`, for
/// Ctrl+Left/Right-style motion (and, paired with [`Buffer::delete`],
/// Ctrl+Backspace/Delete).
///
/// `Direction::Right` skips the remainder of whatever token the cursor is
/// currently inside (if any), then any whitespace/line breaks after it,
/// landing on the start of the next token — or the end of the buffer if
/// there isn't one. `Direction::Left` is the mirror image: it skips
/// backwards over whitespace/line breaks first, then the token behind
/// that, landing on *its* start.
///
/// Since punctuation is its own token distinct from word characters, a
/// press moves through `foo, bar` stopping at the start of `foo`, `,`, and
/// `bar` in turn, matching vim's `w`/`b` rather than treating `foo,` as
/// one word.
pub fn word_boundary<S: TextStorage>(
    buffer: &Buffer<S>,
    cursor: Position,
    direction: Direction,
) -> Position {
    match direction {
        Direction::Right => {
            let mut pos = cursor;

            if let Some(class) = class_at(buffer, pos) {
                if class != CharClass::Whitespace {
                    pos = advance_while(buffer, pos, |c| {
                        char_class(c) == class
                    });
                }
            }

            advance_while(buffer, pos, |c| {
                char_class(c) == CharClass::Whitespace
            })
        }

        Direction::Left => {
            let mut pos = retreat_while(buffer, cursor, |c| {
                char_class(c) == CharClass::Whitespace
            });

            if let Some(class) = class_before(buffer, pos) {
                pos = retreat_while(buffer, pos, |c| char_class(c) == class);
            }

            pos
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

    // ===== word_boundary =====

    #[test]
    fn word_right_from_start_of_word_skips_to_next_word() {
        let buffer = buffer(&["hello world"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 0),
            Direction::Right,
        );

        assert_eq!(target, Position::new(0, 6));
    }

    #[test]
    fn word_right_from_middle_of_word_skips_to_next_word() {
        let buffer = buffer(&["hello world"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 2),
            Direction::Right,
        );

        assert_eq!(target, Position::new(0, 6));
    }

    #[test]
    fn word_right_stops_at_punctuation_as_its_own_token() {
        let buffer = buffer(&["foo, bar"]);

        // "foo, bar": f-o-o-,-space-b-a-r, so "," starts at column 3.
        let target = word_boundary(
            &buffer,
            Position::new(0, 0),
            Direction::Right,
        );

        assert_eq!(target, Position::new(0, 3));
    }

    #[test]
    fn word_right_from_punctuation_skips_to_next_word() {
        let buffer = buffer(&["foo, bar"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 3),
            Direction::Right,
        );

        assert_eq!(target, Position::new(0, 5));
    }

    #[test]
    fn word_right_crosses_line_boundary() {
        let buffer = buffer(&["abc", "   def"]);

        // End of "abc"; should land past the leading whitespace of the
        // next line, at the start of "def".
        let target = word_boundary(
            &buffer,
            Position::new(0, 3),
            Direction::Right,
        );

        assert_eq!(target, Position::new(1, 3));
    }

    #[test]
    fn word_right_skips_blank_lines() {
        let buffer = buffer(&["abc", "", "def"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 3),
            Direction::Right,
        );

        assert_eq!(target, Position::new(2, 0));
    }

    #[test]
    fn word_right_at_end_of_buffer_stays_put() {
        let buffer = buffer(&["abc"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 3),
            Direction::Right,
        );

        assert_eq!(target, Position::new(0, 3));
    }

    #[test]
    fn word_left_from_end_of_word_skips_to_its_start() {
        let buffer = buffer(&["hello world"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 11),
            Direction::Left,
        );

        assert_eq!(target, Position::new(0, 6));
    }

    #[test]
    fn word_left_from_start_of_word_skips_to_previous_word() {
        let buffer = buffer(&["hello world"]);

        // Column 6 is already the start of "world"; Left should jump back
        // to the start of "hello", not stay put.
        let target = word_boundary(
            &buffer,
            Position::new(0, 6),
            Direction::Left,
        );

        assert_eq!(target, Position::new(0, 0));
    }

    #[test]
    fn word_left_crosses_line_boundary() {
        let buffer = buffer(&["abc", "def"]);

        let target = word_boundary(
            &buffer,
            Position::new(1, 0),
            Direction::Left,
        );

        assert_eq!(target, Position::new(0, 0));
    }

    #[test]
    fn word_left_skips_blank_lines() {
        let buffer = buffer(&["abc", "", "def"]);

        let target = word_boundary(
            &buffer,
            Position::new(2, 0),
            Direction::Left,
        );

        assert_eq!(target, Position::new(0, 0));
    }

    #[test]
    fn word_left_at_start_of_buffer_stays_put() {
        let buffer = buffer(&["abc"]);

        let target = word_boundary(
            &buffer,
            Position::new(0, 0),
            Direction::Left,
        );

        assert_eq!(target, Position::new(0, 0));
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
