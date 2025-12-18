//! Atomic swap implementation for cross-chain transactions

use crate::{CrossChainError, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomic swap state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapState {
    /// Swap initiated
    Initiated,
    /// Swap locked
    Locked,
    /// Swap confirmed
    Confirmed,
    /// Swap completed
    Completed,
    /// Swap cancelled
    Cancelled,
    /// Swap failed
    Failed,
}

impl std::fmt::Display for SwapState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwapState::Initiated => write!(f, "Initiated"),
            SwapState::Locked => write!(f, "Locked"),
            SwapState::Confirmed => write!(f, "Confirmed"),
            SwapState::Completed => write!(f, "Completed"),
            SwapState::Cancelled => write!(f, "Cancelled"),
            SwapState::Failed => write!(f, "Failed"),
        }
    }
}

/// Atomic swap details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicSwap {
    /// Swap ID
    pub id: Vec<u8>,
    /// Initiator address
    pub initiator: Vec<u8>,
    /// Participant address
    pub participant: Vec<u8>,
    /// Source chain
    pub source_chain: u32,
    /// Destination chain
    pub dest_chain: u32,
    /// Amount to swap
    pub amount: u128,
    /// Swap state
    pub state: SwapState,
    /// Lock time (Unix timestamp)
    pub lock_time: u64,
    /// Expiration time (Unix timestamp)
    pub expiration: u64,
    /// Hash lock (for HTLC)
    pub hash_lock: Vec<u8>,
    /// Secret (revealed after confirmation)
    pub secret: Option<Vec<u8>>,
    /// Initiator signature
    pub initiator_sig: Vec<u8>,
    /// Participant signature
    pub participant_sig: Vec<u8>,
}

impl AtomicSwap {
    /// Create a new atomic swap
    pub fn new(
        initiator: Vec<u8>,
        participant: Vec<u8>,
        source_chain: u32,
        dest_chain: u32,
        amount: u128,
        hash_lock: Vec<u8>,
    ) -> Result<Self> {
        if initiator.is_empty() || participant.is_empty() {
            return Err(CrossChainError::SwapFailed(
                "Initiator and participant cannot be empty".to_string(),
            ));
        }

        if amount == 0 {
            return Err(CrossChainError::SwapFailed(
                "Amount must be greater than 0".to_string(),
            ));
        }

        if source_chain == dest_chain {
            return Err(CrossChainError::SwapFailed(
                "Source and destination chains must be different".to_string(),
            ));
        }

        // Generate swap ID
        let mut hasher = blake3::Hasher::new();
        hasher.update(&initiator);
        hasher.update(&participant);
        hasher.update(&amount.to_le_bytes());
        let id = hasher.finalize().as_bytes().to_vec();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            id,
            initiator,
            participant,
            source_chain,
            dest_chain,
            amount,
            state: SwapState::Initiated,
            lock_time: now,
            expiration: now + 3600, // 1 hour expiration
            hash_lock,
            secret: None,
            initiator_sig: Vec::new(),
            participant_sig: Vec::new(),
        })
    }

    /// Lock the swap
    pub fn lock(&mut self) -> Result<()> {
        if self.state != SwapState::Initiated {
            return Err(CrossChainError::InvalidSwapState(
                format!("Cannot lock swap in state: {}", self.state),
            ));
        }

        self.state = SwapState::Locked;
        Ok(())
    }

    /// Confirm the swap
    pub fn confirm(&mut self, secret: Vec<u8>) -> Result<()> {
        if self.state != SwapState::Locked {
            return Err(CrossChainError::InvalidSwapState(
                format!("Cannot confirm swap in state: {}", self.state),
            ));
        }

        // Verify secret against hash lock
        let mut hasher = blake3::Hasher::new();
        hasher.update(&secret);
        let computed_hash = hasher.finalize().as_bytes().to_vec();

        if computed_hash != self.hash_lock {
            return Err(CrossChainError::SwapFailed(
                "Secret does not match hash lock".to_string(),
            ));
        }

        self.secret = Some(secret);
        self.state = SwapState::Confirmed;
        Ok(())
    }

    /// Complete the swap
    pub fn complete(&mut self) -> Result<()> {
        if self.state != SwapState::Confirmed {
            return Err(CrossChainError::InvalidSwapState(
                format!("Cannot complete swap in state: {}", self.state),
            ));
        }

        // Check expiration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now > self.expiration {
            return Err(CrossChainError::SwapFailed("Swap has expired".to_string()));
        }

        self.state = SwapState::Completed;
        Ok(())
    }

    /// Cancel the swap
    pub fn cancel(&mut self) -> Result<()> {
        if self.state == SwapState::Completed || self.state == SwapState::Cancelled {
            return Err(CrossChainError::InvalidSwapState(
                format!("Cannot cancel swap in state: {}", self.state),
            ));
        }

        self.state = SwapState::Cancelled;
        Ok(())
    }

    /// Check if swap has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now > self.expiration
    }

    /// Validate swap
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(CrossChainError::InvalidSwapState(
                "Swap ID is empty".to_string(),
            ));
        }

        if self.initiator.is_empty() || self.participant.is_empty() {
            return Err(CrossChainError::InvalidSwapState(
                "Initiator or participant is empty".to_string(),
            ));
        }

        if self.amount == 0 {
            return Err(CrossChainError::InvalidSwapState(
                "Amount is zero".to_string(),
            ));
        }

        if self.source_chain == self.dest_chain {
            return Err(CrossChainError::InvalidSwapState(
                "Source and destination chains are the same".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_creation() {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"secret");
        let hash_lock = hasher.finalize().as_bytes().to_vec();

        let swap = AtomicSwap::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            0,
            1,
            1000,
            hash_lock,
        );

        assert!(swap.is_ok());
        let swap = swap.unwrap();
        assert_eq!(swap.state, SwapState::Initiated);
        assert_eq!(swap.amount, 1000);
    }

    #[test]
    fn test_swap_state_transitions() {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"secret");
        let hash_lock = hasher.finalize().as_bytes().to_vec();

        let mut swap = AtomicSwap::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            0,
            1,
            1000,
            hash_lock,
        )
        .unwrap();

        assert!(swap.lock().is_ok());
        assert_eq!(swap.state, SwapState::Locked);

        assert!(swap.confirm(b"secret".to_vec()).is_ok());
        assert_eq!(swap.state, SwapState::Confirmed);

        assert!(swap.complete().is_ok());
        assert_eq!(swap.state, SwapState::Completed);
    }

    #[test]
    fn test_swap_invalid_secret() {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"secret");
        let hash_lock = hasher.finalize().as_bytes().to_vec();

        let mut swap = AtomicSwap::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            0,
            1,
            1000,
            hash_lock,
        )
        .unwrap();

        swap.lock().unwrap();
        assert!(swap.confirm(b"wrong_secret".to_vec()).is_err());
    }

    #[test]
    fn test_swap_validation() {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"secret");
        let hash_lock = hasher.finalize().as_bytes().to_vec();

        let swap = AtomicSwap::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            0,
            1,
            1000,
            hash_lock,
        )
        .unwrap();

        assert!(swap.validate().is_ok());
    }
}
