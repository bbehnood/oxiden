pub mod buffer;
pub mod error;
pub mod position;
pub mod range;
pub mod storage;

pub use buffer::Buffer;
pub use error::{BufferError, Result};
pub use position::Position;
pub use range::Range;
pub use storage::TextStorage;
pub use storage::VecStorage;
