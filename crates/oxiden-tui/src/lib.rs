//! Terminal front end for the Oxiden editor, built on `crossterm`.
//!
//! This crate is the glue between raw terminal I/O and `oxiden-core`:
//!
//! - [`terminal::Terminal`]: enters/exits raw mode and the alternate
//!   screen, restoring the terminal on drop (including on panic).
//! - [`input`]: translates `crossterm` key events into
//!   [`oxiden_core::Command`]s ([`Action`]) and resolves cursor motions
//!   ([`Move`]) against a buffer.
//! - [`Viewport`]: tracks which region of the document is visible and
//!   scrolls to keep the cursor in view.
//! - [`render`]: draws the visible buffer, a status line, and positions
//!   the terminal cursor.
//!
//! None of these types know about `Editor` directly except through the
//! `Command`/buffer types they consume — the actual event loop that ties
//! them together lives in the `oxiden` binary crate.

pub mod input;
pub mod render;
pub mod terminal;
pub mod viewport;

pub use input::{Action, Move};
pub use terminal::Terminal;
pub use viewport::Viewport;
