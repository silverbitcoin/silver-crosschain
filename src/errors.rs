//! Cross-chain error types

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum CrossChainError {
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Message routing failed: {0}")]
    RoutingFailed(String),

    #[error("Atomic swap failed: {0}")]
    SwapFailed(String),

    #[error("Bridge not found: {0}")]
    BridgeNotFound(String),

    #[error("Chain not connected: {0}")]
    ChainNotConnected(String),

    #[error("Merkle proof verification failed")]
    ProofVerificationFailed,

    #[error("State synchronization failed: {0}")]
    SyncFailed(String),

    #[error("Message timeout")]
    MessageTimeout,

    #[error("Invalid swap state: {0}")]
    InvalidSwapState(String),

    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),

    #[error("Double spend detected")]
    DoubleSpend,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Message already processed")]
    DuplicateMessage,

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CrossChainError>;

impl From<std::io::Error> for CrossChainError {
    fn from(err: std::io::Error) -> Self {
        CrossChainError::Internal(err.to_string())
    }
}
