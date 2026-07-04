/// Convenience alias for results returned by this crate's fallible
/// operations.
pub type Result<T> = std::result::Result<T, DocumentError>;

/// Errors produced while loading, saving, or editing a [`crate::Document`].
#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    /// [`crate::Document::save`] was called on a document that has never
    /// been given a path (neither opened from disk nor saved with
    /// `save_as`).
    #[error("document has no path; use `save_as` instead")]
    NoPath,

    /// Reading from or writing to disk failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// An edit was rejected by the underlying buffer (e.g. an out-of-range
    /// position).
    #[error(transparent)]
    Buffer(#[from] oxiden_buffer::BufferError),
}
