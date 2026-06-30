pub type Result<T> = std::result::Result<T, BufferError>;

#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("invalid position")]
    InvalidPosition,

    #[error("invalid range")]
    InvalidRange,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
