//! Cross-Chain Communication and Atomic Swap Implementation
//!
//! This module provides real cross-chain communication with:
//! - Atomic swaps between chains
//! - Cross-chain message routing
//! - State synchronization
//! - Merkle proof verification

pub mod atomic_swap;
pub mod bridge;
pub mod errors;
pub mod message;
pub mod routing;

pub use atomic_swap::{AtomicSwap, SwapState};
pub use bridge::{CrossChainBridge, BridgeConfig};
pub use errors::{CrossChainError, Result};
pub use message::{CrossChainMessage, MessageType};
pub use routing::MessageRouter;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrossChainInitError {
    #[error("Bridge initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Invalid bridge configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Cross-chain error: {0}")]
    CrossChainError(String),
}

pub type CrossChainInitResult<T> = std::result::Result<T, CrossChainInitError>;

/// Cross-chain configuration
#[derive(Debug, Clone, Copy)]
pub struct CrossChainConfig {
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Message timeout in seconds
    pub message_timeout_secs: u64,
    /// Maximum pending messages
    pub max_pending_messages: usize,
    /// Confirmation blocks required
    pub confirmation_blocks: u32,
    /// Enable atomic swaps
    pub enable_atomic_swaps: bool,
}

impl Default for CrossChainConfig {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024, // 1 MB
            message_timeout_secs: 3600,    // 1 hour
            max_pending_messages: 10000,
            confirmation_blocks: 6,
            enable_atomic_swaps: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CrossChainConfig::default();
        assert_eq!(config.max_message_size, 1024 * 1024);
        assert_eq!(config.message_timeout_secs, 3600);
        assert!(config.enable_atomic_swaps);
    }
}
