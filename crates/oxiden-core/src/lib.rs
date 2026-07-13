//! Editor-level logic built on top of `oxiden-buffer`.
//!
//! Where `oxiden-buffer` only knows about raw text storage, this crate adds
//! the concepts an actual editor needs:
//!
//! - [`Cursor`]: the user's current position in the document.
//! - [`Document`]: a [`oxiden_buffer::Buffer`] plus file metadata (path,
//!   dirty flag, line-ending style) and load/save logic.
//! - [`Command`]: the set of edit operations a UI can request.
//! - [`Editor`]: ties a `Document` and `Cursor` together and applies
//!   `Command`s to both, keeping the cursor consistent with each edit.
//! - [`History`]: an undo/redo log of small reversible [`Edit`]s, owned by
//!   `Document` and updated automatically by every insert/delete.
//!
//! This crate has no dependency on any particular UI toolkit — `Command`
//! and `Editor` are input-agnostic, which is what lets `oxiden-tui`
//! translate key presses into `Command`s without `oxiden-core` knowing
//! anything about terminals or key codes.

pub mod command;
pub mod cursor;
pub mod document;
pub mod editor;
pub mod error;
pub mod history;

pub use command::Command;
pub use cursor::Cursor;
pub use document::Document;
pub use editor::Editor;
pub use error::{DocumentError, Result};
pub use history::{Edit, History};
