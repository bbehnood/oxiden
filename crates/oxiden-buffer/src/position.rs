/// A location inside a text buffer, expressed as a zero-indexed line and
/// column.
///
/// The column is a **character** offset, not a byte offset, so it is valid
/// across multi-byte UTF-8 text (e.g. `column: 1` in "😀😁" refers to the
/// position between the two emoji, not between two bytes of the first one).
///
/// `Position` derives `Ord`/`PartialOrd` using the natural field order
/// (`line` first, then `column`), so positions can be compared and sorted
/// the way they read on screen: earlier lines sort before later ones, and
/// within the same line, earlier columns sort before later ones.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    /// Zero-indexed line number.
    pub line: usize,
    /// Zero-indexed column, counted in characters (not bytes).
    pub column: usize,
}

impl Position {
    /// Creates a new position at the given line and column.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}
