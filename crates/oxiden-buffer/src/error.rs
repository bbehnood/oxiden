/// Convenience alias for results returned by this crate's fallible
/// operations.
pub type Result<T> = std::result::Result<T, BufferError>;

/// Errors produced while reading or mutating a [`crate::Buffer`].
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    /// A [`crate::Position`] referenced a line that doesn't exist, or a
    /// column beyond the end of its line.
    #[error("invalid position")]
    InvalidPosition,

    /// Reserved for range-level validation failures (currently unused;
    /// `VecStorage` validates each endpoint individually and reports
    /// `InvalidPosition` instead).
    #[error("invalid range")]
    InvalidRange,

    /// Wraps an underlying I/O failure (e.g. reading a file into storage).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
