//! Cross-chain message types and handling

use crate::{CrossChainError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cross-chain message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// State synchronization message
    StateSync,
    /// Atomic swap initiation
    SwapInit,
    /// Atomic swap confirmation
    SwapConfirm,
    /// Atomic swap cancellation
    SwapCancel,
    /// Generic data transfer
    DataTransfer,
    /// Merkle proof verification
    ProofVerification,
    /// Acknowledgment
    Ack,
}

impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageType::StateSync => write!(f, "StateSync"),
            MessageType::SwapInit => write!(f, "SwapInit"),
            MessageType::SwapConfirm => write!(f, "SwapConfirm"),
            MessageType::SwapCancel => write!(f, "SwapCancel"),
            MessageType::DataTransfer => write!(f, "DataTransfer"),
            MessageType::ProofVerification => write!(f, "ProofVerification"),
            MessageType::Ack => write!(f, "Ack"),
        }
    }
}

/// Cross-chain message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainMessage {
    /// Unique message ID
    pub id: Vec<u8>,
    /// Source chain ID
    pub source_chain: u32,
    /// Destination chain ID
    pub dest_chain: u32,
    /// Message type
    pub message_type: MessageType,
    /// Message payload
    pub payload: Vec<u8>,
    /// Timestamp
    pub timestamp: u64,
    /// Nonce for replay protection
    pub nonce: u64,
    /// Sender address
    pub sender: Vec<u8>,
    /// Signature
    pub signature: Vec<u8>,
    /// Metadata
    pub metadata: HashMap<String, Vec<u8>>,
}

impl CrossChainMessage {
    /// Create a new cross-chain message
    pub fn new(
        source_chain: u32,
        dest_chain: u32,
        message_type: MessageType,
        payload: Vec<u8>,
        sender: Vec<u8>,
    ) -> Result<Self> {
        if payload.is_empty() {
            return Err(CrossChainError::InvalidMessage(
                "Payload cannot be empty".to_string(),
            ));
        }

        if sender.is_empty() {
            return Err(CrossChainError::InvalidMessage(
                "Sender cannot be empty".to_string(),
            ));
        }

        if source_chain == dest_chain {
            return Err(CrossChainError::InvalidMessage(
                "Source and destination chains cannot be the same".to_string(),
            ));
        }

        // Generate message ID using blake3
        let mut hasher = blake3::Hasher::new();
        hasher.update(&source_chain.to_le_bytes());
        hasher.update(&dest_chain.to_le_bytes());
        hasher.update(&payload);
        let id = hasher.finalize().as_bytes().to_vec();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            id,
            source_chain,
            dest_chain,
            message_type,
            payload,
            timestamp: now,
            nonce: 0,
            sender,
            signature: Vec::new(),
            metadata: HashMap::new(),
        })
    }

    /// Validate message structure
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(CrossChainError::InvalidMessage("Message ID is empty".to_string()));
        }

        if self.payload.is_empty() {
            return Err(CrossChainError::InvalidMessage(
                "Payload is empty".to_string(),
            ));
        }

        if self.sender.is_empty() {
            return Err(CrossChainError::InvalidMessage(
                "Sender is empty".to_string(),
            ));
        }

        if self.source_chain == self.dest_chain {
            return Err(CrossChainError::InvalidMessage(
                "Source and destination chains cannot be the same".to_string(),
            ));
        }

        Ok(())
    }

    /// Get message size in bytes
    pub fn size(&self) -> usize {
        self.id.len()
            + self.payload.len()
            + self.sender.len()
            + self.signature.len()
            + 32 // metadata overhead
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: Vec<u8>) {
        self.metadata.insert(key, value);
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&Vec<u8>> {
        self.metadata.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = CrossChainMessage::new(
            0,
            1,
            MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        );

        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.source_chain, 0);
        assert_eq!(msg.dest_chain, 1);
        assert_eq!(msg.message_type, MessageType::DataTransfer);
    }

    #[test]
    fn test_message_validation() {
        let msg = CrossChainMessage::new(
            0,
            1,
            MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        )
        .unwrap();

        assert!(msg.validate().is_ok());
    }

    #[test]
    fn test_message_invalid_same_chain() {
        let msg = CrossChainMessage::new(
            0,
            0,
            MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        );

        assert!(msg.is_err());
    }

    #[test]
    fn test_message_metadata() {
        let mut msg = CrossChainMessage::new(
            0,
            1,
            MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        )
        .unwrap();

        msg.add_metadata("key".to_string(), vec![7, 8, 9]);
        assert_eq!(msg.get_metadata("key"), Some(&vec![7, 8, 9]));
    }
}
