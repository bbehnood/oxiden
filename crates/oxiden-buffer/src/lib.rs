//! Core text-storage primitives for the Oxiden editor.
//!
//! This crate is UI-agnostic: it only knows how to hold text, address
//! locations within it (via character-based [`Position`]s), and apply
//! inserts/deletes. It has no notion of files, cursors, or key bindings —
//! those live in `oxiden-core` and `oxiden-tui`, which build on top of
//! the [`Buffer`] type here.
//!
//! The storage backend is pluggable via the [`TextStorage`] trait, so
//! alternative representations can be swapped in without changing
//! `Buffer`'s API. Three implementations exist today: [`VecStorage`], a
//! plain `Vec<String>` with one entry per line; [`RopeStorage`], a
//! hand-rolled rope that scales better for large documents; and
//! [`RopeyStorage`], which wraps the `ropey` crate's production rope
//! implementation.

pub mod buffer;
pub mod error;
pub mod position;
pub mod range;
pub mod storage;

pub use buffer::Buffer;
pub use error::{BufferError, Result};
pub use position::Position;
pub use range::Range;
pub use storage::RopeStorage;
pub use storage::RopeyStorage;
pub use storage::TextStorage;
pub use storage::VecStorage;
