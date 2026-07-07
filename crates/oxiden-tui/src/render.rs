//! Draws the current editor state to the terminal using `crossterm`.
//!
//! Drawing is unconditional and full-screen on every call (clear, redraw
//! buffer, redraw status line, reposition cursor) rather than diffed —
//! simple, and fast enough for an editor's redraw rate, at the cost of
//! some flicker-prone terminals not being ideal candidates for this
//! approach at large sizes.

use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};

use oxiden_buffer::TextStorage;
use oxiden_core::Editor;

use crate::Viewport;

/// How many columns a tab advances to (rounding up to the next stop),
/// matching common terminal defaults.
const TAB_WIDTH: usize = 4;

/// Redraws the entire screen: buffer contents, status line, and cursor
/// position, then flushes stdout so the frame actually appears.
///
/// `message` overrides the default status line (e.g. "Saved" or an error)
/// for one frame; pass `None` to show the default filename/position
/// status.
pub fn draw<S: TextStorage>(
    editor: &Editor<S>,
    viewport: &Viewport,
    message: Option<&str>,
) -> io::Result<()> {
    let mut stdout = io::stdout();

    // Hide the cursor while redrawing so it doesn't visibly jump around
    // mid-frame; `position_cursor` shows it again at the end once it's in
    // its final spot.
    queue!(stdout, Clear(ClearType::All), Hide)?;

    draw_buffer(&mut stdout, editor, viewport)?;
    draw_status_line(&mut stdout, editor, viewport, message)?;
    position_cursor(&mut stdout, editor, viewport)?;

    stdout.flush()
}

/// Draws each visible line of the buffer, clipped to the viewport's
/// scroll offset and size. Stops early once lines run out (e.g. near the
/// end of a short document).
fn draw_buffer<S: TextStorage>(
    stdout: &mut impl Write,
    editor: &Editor<S>,
    viewport: &Viewport,
) -> io::Result<()> {
    let buffer = editor.document().buffer();

    for row in 0..viewport.height {
        let Some(line) = buffer.line(viewport.top + row) else {
            break;
        };

        let text = clip(line.as_ref(), viewport.left, viewport.width);

        queue!(stdout, MoveTo(0, row as u16), Print(text))?;
    }

    Ok(())
}

/// Returns the portion of `line` visible in a window that starts `left`
/// columns in and is `width` columns wide.
///
/// Tabs are expanded to spaces first (see [`expand_tabs`]) so that one
/// `char` of the result always corresponds to exactly one terminal
/// column — printing a raw `'\t'` and letting the terminal expand it
/// itself would silently desync our column math from what's actually on
/// screen, since every other calculation here (including cursor
/// placement) assumes 1 char == 1 column.
fn clip(line: &str, left: usize, width: usize) -> String {
    expand_tabs(line, TAB_WIDTH).chars().skip(left).take(width).collect()
}

/// Replaces each tab in `line` with spaces out to the next stop of
/// `tab_width`, so the result has one `char` per terminal column.
///
/// Tab stops are computed from the start of the (unwrapped) line, which
/// matches how a terminal would expand the same raw text.
fn expand_tabs(line: &str, tab_width: usize) -> String {
    let mut expanded = String::with_capacity(line.len());
    let mut col = 0;

    for ch in line.chars() {
        if ch == '\t' {
            let width = tab_width - (col % tab_width);
            expanded.extend(std::iter::repeat_n(' ', width));
            col += width;
        } else {
            expanded.push(ch);
            col += 1;
        }
    }

    expanded
}

/// Converts a character-index `column` within `line` into the display
/// column it lands on once tabs before it are expanded — e.g. column 1
/// (the char right after a leading tab) is display column 4, not 1.
///
/// Used to place the terminal's real cursor under the correct glyph;
/// without this, a tab anywhere before the cursor on the same line would
/// leave the visible cursor sitting to the left of where it should be.
fn display_column(line: &str, column: usize, tab_width: usize) -> usize {
    let mut col = 0;

    for ch in line.chars().take(column) {
        if ch == '\t' {
            col += tab_width - (col % tab_width);
        } else {
            col += 1;
        }
    }

    col
}

/// Draws the bottom status line: either an override `message`, or by
/// default the file name (or `[No Name]`), a `[+]` marker if there are
/// unsaved changes, and the 1-indexed cursor line/column.
fn draw_status_line<S: TextStorage>(
    stdout: &mut impl Write,
    editor: &Editor<S>,
    viewport: &Viewport,
    message: Option<&str>,
) -> io::Result<()> {
    let document = editor.document();
    let cursor = editor.cursor().position();

    let status = match message {
        Some(message) => message.to_string(),

        None => {
            let name = document
                .path()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("[No Name]");

            let dirty = if document.is_dirty() { " [+]" } else { "" };

            format!(
                "{name}{dirty}  Ln {}, Col {}",
                cursor.line + 1,
                cursor.column + 1
            )
        }
    };

    let status = clip(&status, 0, viewport.width);

    queue!(stdout, MoveTo(0, viewport.height as u16), Print(status))
}

/// Moves the terminal's real cursor to match the editor cursor's position
/// within the viewport, then makes it visible again.
///
/// The editor cursor's column is a character index, but the terminal
/// column it corresponds to can be further right if the line has tabs
/// before it, so it's converted via [`display_column`] first.
///
/// Assumes the caller (`draw`) has already scrolled the viewport so the
/// cursor is within `[top, top + height)` / `[left, left + width)` —
/// otherwise the subtraction below would underflow.
fn position_cursor<S: TextStorage>(
    stdout: &mut impl Write,
    editor: &Editor<S>,
    viewport: &Viewport,
) -> io::Result<()> {
    let cursor = editor.cursor().position();

    let line = editor.document().buffer().line(cursor.line);
    let column = line
        .map(|line| display_column(line.as_ref(), cursor.column, TAB_WIDTH))
        .unwrap_or(cursor.column);

    let row = (cursor.line - viewport.top) as u16;
    let col = (column - viewport.left) as u16;

    queue!(stdout, MoveTo(col, row), Show)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tabs_pads_to_next_stop() {
        assert_eq!(expand_tabs("\tx", 4), "    x");
        assert_eq!(expand_tabs("ab\tx", 4), "ab  x");
        assert_eq!(expand_tabs("abcd\tx", 4), "abcd    x");
    }

    #[test]
    fn expand_tabs_leaves_plain_text_untouched() {
        assert_eq!(expand_tabs("hello", 4), "hello");
    }

    #[test]
    fn clip_expands_tabs_before_slicing() {
        // A leading tab (width 4) pushes "x" to column 4, so a window
        // starting at column 0 with width 5 should show it.
        assert_eq!(clip("\tx", 0, 5), "    x");
    }

    #[test]
    fn clip_is_still_char_safe_on_utf8() {
        assert_eq!(clip("héllo", 1, 3), "éll");
    }

    #[test]
    fn display_column_matches_char_column_without_tabs() {
        assert_eq!(display_column("hello", 3, 4), 3);
    }

    #[test]
    fn display_column_accounts_for_leading_tab() {
        // One char (the tab) in, but it expanded to 4 columns.
        assert_eq!(display_column("\tx", 1, 4), 4);
    }

    #[test]
    fn display_column_accounts_for_tab_mid_line() {
        // "ab" (2 cols) + a tab that pads out to the next stop of 4 (2
        // more cols) = display column 4 by the time we reach "x".
        assert_eq!(display_column("ab\tx", 3, 4), 4);
    }

    #[test]
    fn display_column_zero_is_always_zero() {
        assert_eq!(display_column("\t\tx", 0, 4), 0);
    }
}
