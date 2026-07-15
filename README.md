# Oxiden

A small terminal text editor written in Rust.

Oxiden is organized as a Cargo workspace split into focused crates, each
with a single responsibility: text storage, editing logic, and the
terminal UI are all independent of one another.

## Features

- Basic text editing: insert, backspace, delete, and newlines
- Undo/redo, with consecutive typing and deletion coalesced into a single
  undo step
- Arrow-key and Home/End cursor navigation, with wrapping across line
  boundaries and column clamping on shorter lines
- Open an existing file or start a new one at a given path
- Preserves the original file's line-ending style (`\n` vs `\r\n`) and
  whether it ended with a trailing newline
- Unsaved-changes indicator and a confirmation prompt before quitting with
  unsaved changes
- Full Unicode support — positions are tracked in characters, not bytes
- Configurable via an optional file (tab width, spaces-vs-tabs, default
  storage backend); see [Configuration](#configuration)

## Installation

Requires a recent Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo build --release
```

The binary is produced at `target/release/oxiden`.

## Usage

```sh
# Open an existing file (or create it on first save if it doesn't exist)
oxiden path/to/file.txt

# Start with an empty, unnamed buffer
oxiden

# Choose a storage backend (overrides the config file, defaults to
# ropey): vec, rope, or ropey
oxiden --backend vec path/to/file.txt
```

### Key bindings

| Key                | Action                                          |
| ------------------ | ------------------------------------------------ |
| Any character       | Insert at cursor                                 |
| `Tab`               | Insert a tab character (or spaces, if configured) |
| `Enter`             | Insert a newline                                 |
| `Backspace`         | Delete the character before the cursor           |
| `Delete`            | Delete the character at the cursor               |
| `Ctrl+delete`       | Delete the next word                             |
| `←` `→` `↑` `↓`     | Move the cursor                                  |
| `Ctrl+←` / `Ctrl+→` | Move to the next / previous word                 |
| `Home` / `End`      | Move to the start / end of the line              |
| `PgUp` / `PgDn`     | Move to the start / end of the file              |
| `Ctrl+S`            | Save                                             |
| `F2`                | Save as                                          |
| `Ctrl+Z`            | Undo                                             |
| `Ctrl+R`            | Redo                                             |
| `Ctrl+Q`            | Quit (press twice if there are unsaved changes)  |

## Configuration

Oxiden reads an optional config file at startup:

- `$XDG_CONFIG_HOME/oxiden/config.toml`, or `~/.config/oxiden/config.toml`
  if `XDG_CONFIG_HOME` isn't set.
- Set `OXIDEN_CONFIG=/path/to/file` to point at a config file somewhere
  else instead.

A missing file is fine — every setting has a default. The format is a
flat, TOML-compatible `key = value` syntax (no tables), one setting per
line, with `#` for comments:

```toml
# ~/.config/oxiden/config.toml
tab_width = 2
insert_spaces_for_tab = true
backend = "ropey"
```

| Key                     | Default  | Meaning                                              |
| ------------------------ | -------- | ----------------------------------------------------- |
| `tab_width`              | `4`      | Columns a tab advances to on screen, and the number of spaces used for it when `insert_spaces_for_tab` is set. |
| `insert_spaces_for_tab`  | `false`  | Whether `Tab` inserts spaces instead of a literal tab character. |
| `backend`                | `ropey`  | Default storage backend (`vec`, `rope`, or `ropey`); overridden by `--backend` on the command line. |

A malformed file (an unknown key, or a value of the wrong type) is
reported on startup rather than silently ignored.

## Architecture

The workspace is split into four crates, layered from the bottom up:

```
oxiden-buffer   text storage: positions, ranges, insert/delete
      ^
oxiden-core     documents, cursor, commands, and the editor
      ^
oxiden-tui      crossterm-based terminal UI: input mapping, rendering, viewport
      ^
oxiden          the binary: wires everything together, runs the event loop
```

Each layer only depends on the ones below it, and only the top layer
(`oxiden-tui`) knows anything about terminals or key codes — `oxiden-core`
and `oxiden-buffer` are UI-agnostic and could back a different front end
(GUI, web, etc.) without modification.

### `oxiden-buffer`

The lowest-level crate. Defines:

- `Position` — a zero-indexed `(line, column)` location, using
  **character** offsets so it's Unicode-safe.
- `Range` — a span between two `Position`s.
- `TextStorage` — a trait for pluggable text storage backends.
- `VecStorage` — one `String` per line in a `Vec`. Simple, and fast enough
  for everyday editing.
- `RopeStorage` — a hand-rolled rope (a tree of text chunks) that scales
  better for large documents and edits away from the end of a line.
- `RopeyStorage` — the same idea as `RopeStorage`, but backed by the
  battle-tested `ropey` crate instead of a from-scratch implementation.
- `Buffer<S>` — wraps a `TextStorage` with convenience queries (line
  length, "is this the last line", position validity).

### `oxiden-core`

Editor semantics built on top of a `Buffer`:

- `Cursor` — the user's current position.
- `Document<S>` — a buffer plus file metadata: path, dirty flag, and the
  original line-ending/trailing-newline style, with `open`/`save`/`save_as`.
- `Command` — the set of edit operations a front end can request
  (`Insert`, `Backspace`, `Delete`, `DeleteRange`, `NewLine`, `MoveTo`,
  `Undo`, `Redo`, …).
- `Editor<S>` — applies `Command`s to a `Document` and keeps the `Cursor`
  consistent with each edit.
- `Config` — user-configurable settings (tab width, storage backend, …),
  loaded from an optional file; see [Configuration](#configuration).

### `oxiden-tui`

The terminal front end, built on [`crossterm`](https://docs.rs/crossterm):

- `input` — maps raw key events to `Action`s (`Edit(Command)`, `Move`,
  `Save`, `Quit`) and resolves cursor motions against a buffer.
- `Viewport` — tracks the visible region of the document and scrolls to
  keep the cursor on screen.
- `render` — draws the visible buffer, a status line, and positions the
  terminal cursor.
- `Terminal` — an RAII guard that enters/exits raw mode and the alternate
  screen, restoring the terminal on drop (including on panic).

### `oxiden`

The binary crate. Opens a document (or starts a new one), then runs the
main loop: draw the current state, wait for the next terminal event,
translate it into an `Action`, apply it, repeat.

## Testing

```sh
cargo test --workspace
```

`oxiden-buffer` and `oxiden-core` have thorough unit and integration test
coverage, including Unicode round-tripping, multi-line insert/delete, and
file I/O edge cases (missing trailing newline, CRLF preservation, empty
files).

## License

Dual-licensed under either the [MIT license](LICENSE_MIT) or the
[Apache License, Version 2.0](LICENSE_APACHE), at your option.
