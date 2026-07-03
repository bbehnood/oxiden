use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};

use oxide_buffer::TextStorage;
use oxide_core::Editor;

use crate::Viewport;

pub fn draw<S: TextStorage>(
    editor: &Editor<S>,
    viewport: &Viewport,
    message: Option<&str>,
) -> io::Result<()> {
    let mut stdout = io::stdout();

    queue!(stdout, Clear(ClearType::All), Hide)?;

    draw_buffer(&mut stdout, editor, viewport)?;
    draw_status_line(&mut stdout, editor, viewport, message)?;
    position_cursor(&mut stdout, editor, viewport)?;

    stdout.flush()
}

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

fn clip(line: &str, left: usize, width: usize) -> String {
    line.chars().skip(left).take(width).collect()
}

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
