use crate::Position;

/// A span of text between two positions, used to describe selections and
/// deletions.
///
/// `Range` does not enforce that `start <= end`; callers that care about
/// direction (e.g. `VecStorage::delete`) normalize the order themselves by
/// swapping the endpoints when `start > end`. This lets callers build a
/// `Range` directly from a drag gesture (which may go in either direction)
/// without pre-sorting the endpoints.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Range {
    /// The position where the range begins (not guaranteed to be <= `end`).
    pub start: Position,
    /// The position where the range ends (not guaranteed to be >= `start`).
    pub end: Position,
}

impl Range {
    /// Creates a new range from `start` to `end`, in whatever order given.
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}
