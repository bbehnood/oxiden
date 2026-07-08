mod rope;
mod vec;

pub use rope::RopeStorage;
pub use vec::VecStorage;

use crate::{Position, Range, Result};

/// Abstraction over how a document's text is physically stored.
///
/// Implementations are expected to always contain at least one line (an
/// empty document is one empty line, not zero lines) so that `Position`s
/// with `column: 0` are always valid on a fresh buffer.
///
/// All positions and ranges passed to this trait use **character** offsets,
/// not byte offsets, so implementations must count Unicode scalar values
/// rather than bytes when interpreting `column`.
pub trait TextStorage {
    /// Borrowed view of a single line's text. Implementations may return a
    /// borrow of internal storage (e.g. `&str`) rather than an owned
    /// `String`, since a line is looked up far more often than it is
    /// mutated wholesale.
    type Line<'a>: AsRef<str>
    where
        Self: 'a;

    /// Returns the text of the line at `index`, or `None` if it doesn't
    /// exist.
    fn line(&self, index: usize) -> Option<Self::Line<'_>>;

    /// Returns the number of lines currently stored. Always >= 1.
    fn line_count(&self) -> usize;

    /// Returns the total number of characters in the document, including
    /// one implicit `\n` per line boundary (but not a trailing newline
    /// after the last line).
    fn len_chars(&self) -> usize;

    /// Inserts `text` at `pos`, shifting subsequent content as needed.
    ///
    /// `text` may contain `\n` characters, in which case the line at `pos`
    /// is split and new lines are created. Returns
    /// [`crate::BufferError::InvalidPosition`] if `pos` doesn't refer to a
    /// valid location in the current content.
    fn insert(&mut self, pos: Position, text: &str) -> Result<()>;

    /// Removes the text spanned by `range`.
    ///
    /// If `range` crosses one or more line boundaries, the lines are joined
    /// into one. Returns [`crate::BufferError::InvalidPosition`] if either
    /// endpoint is invalid.
    fn delete(&mut self, range: Range) -> Result<()>;

    /// Renders the entire document as a single `String`, with lines joined
    /// by `\n`. This never includes a trailing newline; callers that need
    /// one (e.g. when writing a file) add it themselves.
    fn to_text(&self) -> String;
}
