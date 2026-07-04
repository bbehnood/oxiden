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
/// characters in and is `width` characters wide. Operates on characters,
/// not bytes, so this is safe for multi-byte UTF-8 content.
fn clip(line: &str, left: usize, width: usize) -> String {
    line.chars().skip(left).take(width).collect()
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
/// Assumes the caller (`draw`) has already scrolled the viewport so the
/// cursor is within `[top, top + height)` / `[left, left + width)` —
/// otherwise the subtraction below would underflow.
fn position_cursor<S: TextStorage>(
    stdout: &mut impl Write,
    editor: &Editor<S>,
    viewport: &Viewport,
) -> io::Result<()> {
    let cursor = editor.cursor().position();

    let row = (cursor.line - viewport.top) as u16;
    let col = (cursor.column - viewport.left) as u16;

    queue!(stdout, MoveTo(col, row), Show)
}
