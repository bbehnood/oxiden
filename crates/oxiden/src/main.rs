//! Oxiden's binary entry point: wires the buffer, core, and terminal crates
//! together into a runnable terminal text editor and drives the main event
//! loop.

use std::io;

use crossterm::event::{self, Event, KeyEventKind};

use oxiden_buffer::{Buffer, TextStorage, VecStorage};
use oxiden_core::{Command, Document, DocumentError, Editor};
use oxiden_tui::input::{self, Action};
use oxiden_tui::{Terminal, Viewport, render};

fn main() -> io::Result<()> {
    // With a file argument: open it, or start a new (unsaved-to-disk)
    // document at that path if it doesn't exist yet. Any other I/O error
    // (e.g. permissions) is fatal. With no argument: start a scratch
    // buffer with no associated path.
    let document = match std::env::args().nth(1) {
        Some(path) => match Document::<VecStorage>::open(&path) {
            Ok(document) => document,

            Err(DocumentError::Io(err))
                if err.kind() == io::ErrorKind::NotFound =>
            {
                Document::new_at(path)
            }

            Err(err) => {
                eprintln!("oxiden: couldn't open {path}: {err}");
                std::process::exit(1);
            }
        },

        None => Document::new(Buffer::new(VecStorage::new())),
    };

    let mut editor = Editor::new(document);

    let terminal = Terminal::enter()?;

    // Make sure the terminal is restored even if we panic mid-frame,
    // rather than leaving the user's shell stuck in raw mode.
    std::panic::set_hook(Box::new(|info| {
        Terminal::restore();
        eprintln!("{info}");
    }));

    let result = run(&mut editor);

    // Explicit drop (rather than letting it happen at end of scope) so the
    // terminal is restored before `result`'s error, if any, gets printed
    // to a shell that's back in cooked mode.
    drop(terminal);

    result
}

/// The main input/render loop: draw the current state, wait for the next
/// terminal event, apply it, repeat until the user quits.
fn run<S: TextStorage>(editor: &mut Editor<S>) -> io::Result<()> {
    let (cols, rows) = Terminal::size()?;

    // Reserve the last row for the status line.
    let mut viewport =
        Viewport::new(cols as usize, rows.saturating_sub(1) as usize);

    let mut status: Option<String> = None;
    let mut quit_pending = false;

    loop {
        viewport.scroll_to(editor.cursor().position());
        render::draw(editor, &viewport, status.as_deref())?;

        match event::read()? {
            // `KeyEventKind::Press` filters out the release/repeat events
            // some terminals report, so each physical key press is only
            // handled once.
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = input::map_key(key);

                if matches!(action, Action::Quit) {
                    // Require confirmation (a second Ctrl+Q) before
                    // discarding unsaved changes.
                    if editor.document().is_dirty() && !quit_pending {
                        status = Some(
                            "Unsaved changes — Ctrl+Q again to quit"
                                .to_string(),
                        );
                        quit_pending = true;
                        continue;
                    }

                    return Ok(());
                }

                // Any key other than the pending quit confirmation cancels
                // it, so the user has to press Ctrl+Q twice *in a row*.
                quit_pending = false;

                match action {
                    Action::Edit(command) => {
                        status = editor
                            .execute(command)
                            .err()
                            .map(|err| err.to_string());
                    }

                    Action::Move(movement) => {
                        let target = input::motion_target(
                            editor.document().buffer(),
                            editor.cursor().position(),
                            movement,
                        );

                        // MoveTo never fails, so the result can be
                        // discarded.
                        let _ = editor.execute(Command::MoveTo(target));
                    }

                    Action::Save => {
                        status = Some(match editor.document_mut().save() {
                            Ok(()) => "Saved".to_string(),
                            Err(err) => err.to_string(),
                        });
                    }

                    // `Quit` is fully handled above; `Noop` intentionally
                    // does nothing.
                    Action::Quit | Action::Noop => {}
                }
            }

            Event::Resize(cols, rows) => {
                viewport.resize(cols as usize, rows.saturating_sub(1) as usize);
            }

            // Ignore other event kinds (mouse, focus, paste, key
            // release/repeat).
            _ => {}
        }
    }
}
