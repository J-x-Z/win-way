//! Error types for win-way

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WinWayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid message format")]
    InvalidMessage,
}

pub type Result<T> = std::result::Result<T, WinWayError>;
