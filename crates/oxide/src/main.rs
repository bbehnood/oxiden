use std::io;

use crossterm::event::{self, Event, KeyEventKind};

use oxide_buffer::{Buffer, TextStorage, VecStorage};
use oxide_core::{Command, Document, DocumentError, Editor};
use oxide_terminal::input::{self, Action};
use oxide_terminal::{Terminal, Viewport, render};

fn main() -> io::Result<()> {
    let document = match std::env::args().nth(1) {
        Some(path) => match Document::<VecStorage>::open(&path) {
            Ok(document) => document,

            Err(DocumentError::Io(err))
                if err.kind() == io::ErrorKind::NotFound =>
            {
                Document::new_at(path)
            }

            Err(err) => {
                eprintln!("oxide: couldn't open {path}: {err}");
                std::process::exit(1);
            }
        },

        None => Document::new(Buffer::new(VecStorage::new())),
    };

    let mut editor = Editor::new(document);

    let terminal = Terminal::enter()?;

    std::panic::set_hook(Box::new(|info| {
        Terminal::restore();
        eprintln!("{info}");
    }));

    let result = run(&mut editor);

    drop(terminal);

    result
}

fn run<S: TextStorage>(editor: &mut Editor<S>) -> io::Result<()> {
    let (cols, rows) = Terminal::size()?;

    let mut viewport =
        Viewport::new(cols as usize, rows.saturating_sub(1) as usize);

    let mut status: Option<String> = None;
    let mut quit_pending = false;

    loop {
        viewport.scroll_to(editor.cursor().position());
        render::draw(editor, &viewport, status.as_deref())?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = input::map_key(key);

                if matches!(action, Action::Quit) {
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

                        let _ = editor.execute(Command::MoveTo(target));
                    }

                    Action::Save => {
                        status = Some(match editor.document_mut().save() {
                            Ok(()) => "Saved".to_string(),
                            Err(err) => err.to_string(),
                        });
                    }

                    Action::Quit | Action::Noop => {}
                }
            }

            Event::Resize(cols, rows) => {
                viewport.resize(cols as usize, rows.saturating_sub(1) as usize);
            }

            _ => {}
        }
    }
}
