//! Errors.

/// SLX errors.
#[derive(Debug, thiserror::Error)]
pub enum SlxError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a valid SLX/OPC package: {0}")]
    Package(String),
    #[error("XML error in {part}: {message}")]
    Xml { part: String, message: String },
    #[error("unsupported SLX content: {0}")]
    Unsupported(String),
}
