//! Oxiden's binary entry point: wires the buffer, core, and terminal crates
//! together into a runnable terminal text editor and drives the main event
//! loop.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use oxiden_buffer::{
    Buffer, Range, RopeStorage, RopeyStorage, TextStorage, VecStorage,
};
use oxiden_core::{Backend, Command, Config, Document, DocumentError, Editor};
use oxiden_tui::input::{self, Action, Move};
use oxiden_tui::{Terminal, Viewport, render};

fn main() -> io::Result<()> {
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("oxiden: {err}");
            std::process::exit(1);
        }
    };

    // A tiny hand-rolled parser rather than pulling in an args crate for
    // two flags: `--backend <vec|rope|ropey>` (or `--backend=<value>`),
    // and a single positional file path. `--backend` overrides whatever
    // the config file (or its default) says.
    let mut backend = config.backend;
    let mut path: Option<String> = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        let requested = if arg == "--backend" {
            let Some(value) = args.next() else {
                eprintln!("oxiden: --backend requires a value");
                std::process::exit(1);
            };
            Some(value)
        } else {
            arg.strip_prefix("--backend=").map(str::to_string)
        };

        if let Some(value) = requested {
            match Backend::parse(&value) {
                Some(parsed) => backend = parsed,
                None => {
                    eprintln!(
                        "oxiden: unknown backend {value:?} \
                         (expected vec, rope, or ropey)"
                    );
                    std::process::exit(1);
                }
            }
        } else {
            path = Some(arg);
        }
    }

    match backend {
        Backend::Vec => run_with::<VecStorage>(path, &config),
        Backend::Rope => run_with::<RopeStorage>(path, &config),
        Backend::Ropey => run_with::<RopeyStorage>(path, &config),
    }
}

/// Opens `path` (or starts a scratch buffer if `path` is `None`) with
/// storage backend `S`, then drives the editor until it quits.
fn run_with<S: TextStorage + Default>(
    path: Option<String>,
    config: &Config,
) -> io::Result<()> {
    // With a file argument: open it, or start a new (unsaved-to-disk)
    // document at that path if it doesn't exist yet. Any other I/O error
    // (e.g. permissions) is fatal. With no argument: start a scratch
    // buffer with no associated path.
    let document = match path {
        Some(path) => match Document::<S>::open(&path) {
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

        None => Document::new(Buffer::new(S::default())),
    };

    let mut editor = Editor::new(document);

    let terminal = Terminal::enter()?;

    // Make sure the terminal is restored even if we panic mid-frame,
    // rather than leaving the user's shell stuck in raw mode.
    std::panic::set_hook(Box::new(|info| {
        Terminal::restore();
        eprintln!("{info}");
    }));

    let result = run(&mut editor, config);

    // Explicit drop (rather than letting it happen at end of scope) so the
    // terminal is restored before `result`'s error, if any, gets printed
    // to a shell that's back in cooked mode.
    drop(terminal);

    result
}

/// Interactive single-line text input shown on the status line in place
/// of the usual filename/position display, used to gather a path for
/// [`Action::SaveAs`]. `None` means the editor is in its normal mode and
/// keys should be interpreted through [`input::map_key`] as usual.
struct Prompt {
    /// Text shown before the user's typed input, e.g. `"Save as: "`.
    label: &'static str,
    /// What the user has typed so far.
    input: String,
}

impl Prompt {
    fn save_as() -> Self {
        Self { label: "Save as: ", input: String::new() }
    }

    /// Renders this prompt as a status-line string, e.g. `"Save as:
    /// notes.txt"`.
    fn render(&self) -> String {
        format!("{}{}", self.label, self.input)
    }
}

/// The main input/render loop: draw the current state, wait for the next
/// terminal event, apply it, repeat until the user quits.
fn run<S: TextStorage>(
    editor: &mut Editor<S>,
    config: &Config,
) -> io::Result<()> {
    let (cols, rows) = Terminal::size()?;

    // Reserve the last row for the status line.
    let mut viewport =
        Viewport::new(cols as usize, rows.saturating_sub(1) as usize);

    let mut status: Option<String> = None;
    let mut quit_pending = false;
    let mut prompt: Option<Prompt> = None;

    loop {
        viewport.scroll_to(editor.cursor().position());

        // A prompt in progress takes over the status line so the user
        // can see what they're typing.
        let status_line =
            prompt.as_ref().map(Prompt::render).or_else(|| status.clone());
        render::draw(
            editor,
            &viewport,
            status_line.as_deref(),
            config.tab_width,
        )?;

        match event::read()? {
            // `KeyEventKind::Press` filters out the release/repeat events
            // some terminals report, so each physical key press is only
            // handled once.
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                // While a prompt is active, every key edits its text
                // buffer instead of being interpreted as an editor
                // command — otherwise typing a filename would also move
                // the cursor, trigger shortcuts, etc.
                if let Some(active) = prompt.as_mut() {
                    match key.code {
                        KeyCode::Enter => {
                            let path = active.input.clone();
                            prompt = None;

                            status = Some(if path.is_empty() {
                                "Save as cancelled: no filename given"
                                    .to_string()
                            } else {
                                match editor.document_mut().save_as(path) {
                                    Ok(()) => "Saved".to_string(),
                                    Err(err) => err.to_string(),
                                }
                            });
                        }

                        KeyCode::Esc => {
                            prompt = None;
                            status = Some("Save as cancelled".to_string());
                        }

                        KeyCode::Backspace => {
                            active.input.pop();
                        }

                        KeyCode::Char(c) => active.input.push(c),

                        // Anything else (arrows, function keys, etc.)
                        // isn't meaningful input for a bare filename
                        // prompt, so it's ignored rather than falling
                        // through to editor commands.
                        _ => {}
                    }

                    continue;
                }

                let action = input::map_key(key, config);

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

                        let vertical =
                            matches!(movement, Move::Up | Move::Down);

                        // MoveTo never fails, so the result can be
                        // discarded.
                        let _ = editor.execute(Command::MoveTo {
                            position: target,
                            vertical,
                        });
                    }

                    Action::DeleteTo(movement) => {
                        let cursor = editor.cursor().position();
                        let target = input::motion_target(
                            editor.document().buffer(),
                            cursor,
                            movement,
                        );

                        // Normalize the input since any motion is being accepted
                        let (start, end) = if target < cursor {
                            (target, cursor)
                        } else {
                            (cursor, target)
                        };

                        let _ = editor.execute(Command::DeleteRange(
                            Range::new(start, end),
                        ));
                    }

                    Action::Save => {
                        status = Some(match editor.document_mut().save() {
                            Ok(()) => "Saved".to_string(),
                            Err(err) => err.to_string(),
                        });
                    }

                    Action::SaveAs => {
                        prompt = Some(Prompt::save_as());
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
