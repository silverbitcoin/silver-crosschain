//! Cross-chain bridge implementation for managing multiple chains

use crate::{
    atomic_swap::AtomicSwap, routing::MessageRouter, CrossChainError, CrossChainMessage, Result,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Bridge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub id: Vec<u8>,
    pub chains: Vec<u32>,
    pub min_confirmations: u32,
    pub max_message_size: usize,
    pub message_timeout_secs: u64,
    pub enable_swaps: bool,
}

impl BridgeConfig {
    pub fn new(chains: Vec<u32>) -> Self {
        let mut hasher = blake3::Hasher::new();
        for chain in &chains {
            hasher.update(&chain.to_le_bytes());
        }
        let id = hasher.finalize().as_bytes().to_vec();

        Self {
            id,
            chains,
            min_confirmations: 6,
            max_message_size: 1024 * 1024,
            message_timeout_secs: 3600,
            enable_swaps: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.chains.is_empty() {
            return Err(CrossChainError::Internal(
                "Bridge must support at least one chain".to_string(),
            ));
        }

        if self.min_confirmations == 0 {
            return Err(CrossChainError::Internal(
                "Minimum confirmations must be greater than 0".to_string(),
            ));
        }

        if self.max_message_size == 0 {
            return Err(CrossChainError::Internal(
                "Maximum message size must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Chain connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChainState {
    Connected,
    Disconnected,
    Syncing,
    Error,
}

impl std::fmt::Display for ChainState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainState::Connected => write!(f, "Connected"),
            ChainState::Disconnected => write!(f, "Disconnected"),
            ChainState::Syncing => write!(f, "Syncing"),
            ChainState::Error => write!(f, "Error"),
        }
    }
}

/// Chain information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: u32,
    pub state: ChainState,
    pub height: u64,
    pub last_block_hash: Vec<u8>,
    pub last_sync: u64,
    pub error_message: Option<String>,
}

impl ChainInfo {
    pub fn new(chain_id: u32) -> Self {
        Self {
            chain_id,
            state: ChainState::Disconnected,
            height: 0,
            last_block_hash: Vec::new(),
            last_sync: 0,
            error_message: None,
        }
    }

    pub fn connect(&mut self) {
        self.state = ChainState::Connected;
        self.error_message = None;
    }

    pub fn disconnect(&mut self) {
        self.state = ChainState::Disconnected;
    }

    pub fn start_sync(&mut self) {
        self.state = ChainState::Syncing;
    }

    pub fn finish_sync(&mut self, height: u64, block_hash: Vec<u8>) {
        self.state = ChainState::Connected;
        self.height = height;
        self.last_block_hash = block_hash;
        self.last_sync = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.error_message = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.state = ChainState::Error;
        self.error_message = Some(error);
    }
}


/// Cross-chain bridge
pub struct CrossChainBridge {
    config: BridgeConfig,
    chains: Arc<DashMap<u32, ChainInfo>>,
    router: Arc<MessageRouter>,
    swaps: Arc<DashMap<Vec<u8>, AtomicSwap>>,
    message_cache: Arc<DashMap<Vec<u8>, CrossChainMessage>>,
}

impl CrossChainBridge {
    pub fn new(config: BridgeConfig) -> Result<Self> {
        config.validate()?;

        info!("Creating cross-chain bridge with {} chains", config.chains.len());

        let chains = Arc::new(DashMap::new());
        for chain_id in &config.chains {
            chains.insert(*chain_id, ChainInfo::new(*chain_id));
        }

        let router = Arc::new(MessageRouter::new(10000));

        Ok(Self {
            config,
            chains,
            router,
            swaps: Arc::new(DashMap::new()),
            message_cache: Arc::new(DashMap::new()),
        })
    }

    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    pub fn get_chain_info(&self, chain_id: u32) -> Option<ChainInfo> {
        self.chains.get(&chain_id).map(|c| c.clone())
    }

    pub fn update_chain_state(&self, chain_id: u32, state: ChainState) -> Result<()> {
        if !self.config.chains.contains(&chain_id) {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Chain {} not supported by bridge",
                chain_id
            )));
        }

        if let Some(mut chain) = self.chains.get_mut(&chain_id) {
            chain.state = state;
            debug!("Updated chain {} state to {}", chain_id, state);
            Ok(())
        } else {
            Err(CrossChainError::ChainNotConnected(format!(
                "Chain {} not found",
                chain_id
            )))
        }
    }

    pub fn connect_chain(&self, chain_id: u32) -> Result<()> {
        if !self.config.chains.contains(&chain_id) {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Chain {} not supported by bridge",
                chain_id
            )));
        }

        if let Some(mut chain) = self.chains.get_mut(&chain_id) {
            chain.connect();
            info!("Connected chain {}", chain_id);
            
            // Register routes to all other connected chains
            for other_chain_id in &self.config.chains {
                if *other_chain_id != chain_id {
                    let route = crate::routing::Route::new(chain_id, *other_chain_id);
                    let _ = self.router.register_route(route);
                    
                    let reverse_route = crate::routing::Route::new(*other_chain_id, chain_id);
                    let _ = self.router.register_route(reverse_route);
                }
            }
            
            Ok(())
        } else {
            Err(CrossChainError::ChainNotConnected(format!(
                "Chain {} not found",
                chain_id
            )))
        }
    }

    pub fn disconnect_chain(&self, chain_id: u32) -> Result<()> {
        if let Some(mut chain) = self.chains.get_mut(&chain_id) {
            chain.disconnect();
            info!("Disconnected chain {}", chain_id);
            Ok(())
        } else {
            Err(CrossChainError::ChainNotConnected(format!(
                "Chain {} not found",
                chain_id
            )))
        }
    }

    pub fn get_connected_chains(&self) -> Vec<u32> {
        self.chains
            .iter()
            .filter(|entry| entry.value().state == ChainState::Connected)
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub async fn route_message(&self, message: CrossChainMessage) -> Result<()> {
        message.validate()?;

        if message.size() > self.config.max_message_size {
            return Err(CrossChainError::InvalidMessage(
                "Message exceeds maximum size".to_string(),
            ));
        }

        if !self.config.chains.contains(&message.source_chain) {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Source chain {} not supported",
                message.source_chain
            )));
        }

        if !self.config.chains.contains(&message.dest_chain) {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Destination chain {} not supported",
                message.dest_chain
            )));
        }

        let source_chain = self.get_chain_info(message.source_chain).ok_or_else(|| {
            CrossChainError::ChainNotConnected(format!(
                "Source chain {} not found",
                message.source_chain
            ))
        })?;

        if source_chain.state != ChainState::Connected {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Source chain {} is not connected",
                message.source_chain
            )));
        }

        let dest_chain = self.get_chain_info(message.dest_chain).ok_or_else(|| {
            CrossChainError::ChainNotConnected(format!(
                "Destination chain {} not found",
                message.dest_chain
            ))
        })?;

        if dest_chain.state != ChainState::Connected {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Destination chain {} is not connected",
                message.dest_chain
            )));
        }

        self.router.route_message(message.clone()).await?;
        self.message_cache.insert(message.id.clone(), message);

        Ok(())
    }

    pub fn get_message(&self, message_id: &[u8]) -> Option<CrossChainMessage> {
        self.message_cache.get(message_id).map(|m| m.clone())
    }

    pub fn create_swap(&self, swap: AtomicSwap) -> Result<()> {
        if !self.config.enable_swaps {
            return Err(CrossChainError::SwapFailed(
                "Atomic swaps are disabled".to_string(),
            ));
        }

        swap.validate()?;

        if !self.config.chains.contains(&swap.source_chain) {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Source chain {} not supported",
                swap.source_chain
            )));
        }

        if !self.config.chains.contains(&swap.dest_chain) {
            return Err(CrossChainError::ChainNotConnected(format!(
                "Destination chain {} not supported",
                swap.dest_chain
            )));
        }

        self.swaps.insert(swap.id.clone(), swap);
        info!("Created atomic swap");

        Ok(())
    }

    pub fn get_swap(&self, swap_id: &[u8]) -> Option<AtomicSwap> {
        self.swaps.get(swap_id).map(|s| s.clone())
    }

    pub fn update_swap(&self, swap: AtomicSwap) -> Result<()> {
        if !self.swaps.contains_key(&swap.id) {
            return Err(CrossChainError::SwapFailed(
                "Swap not found".to_string(),
            ));
        }

        self.swaps.insert(swap.id.clone(), swap);
        Ok(())
    }

    pub fn get_all_swaps(&self) -> Vec<AtomicSwap> {
        self.swaps
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn get_stats(&self) -> BridgeStats {
        let connected_chains = self.get_connected_chains().len();
        let total_chains = self.config.chains.len();
        let pending_messages = self.router.pending_count();
        let processed_messages = self.router.processed_count();
        let active_swaps = self.swaps.len();
        let cached_messages = self.message_cache.len();

        BridgeStats {
            connected_chains,
            total_chains,
            pending_messages,
            processed_messages,
            active_swaps,
            cached_messages,
        }
    }
}

/// Bridge statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStats {
    pub connected_chains: usize,
    pub total_chains: usize,
    pub pending_messages: usize,
    pub processed_messages: usize,
    pub active_swaps: usize,
    pub cached_messages: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MessageType;

    #[test]
    fn test_bridge_config_creation() {
        let config = BridgeConfig::new(vec![0, 1, 2]);
        assert_eq!(config.chains.len(), 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_bridge_creation() {
        let config = BridgeConfig::new(vec![0, 1]);
        let bridge = CrossChainBridge::new(config);
        assert!(bridge.is_ok());
    }

    #[test]
    fn test_chain_connection() {
        let config = BridgeConfig::new(vec![0, 1]);
        let bridge = CrossChainBridge::new(config).unwrap();

        assert!(bridge.connect_chain(0).is_ok());
        let chain_info = bridge.get_chain_info(0).unwrap();
        assert_eq!(chain_info.state, ChainState::Connected);
    }

    #[tokio::test]
    async fn test_message_routing() {
        let config = BridgeConfig::new(vec![0, 1]);
        let bridge = CrossChainBridge::new(config).unwrap();

        bridge.connect_chain(0).unwrap();
        bridge.connect_chain(1).unwrap();

        let msg = CrossChainMessage::new(
            0,
            1,
            MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        )
        .unwrap();

        assert!(bridge.route_message(msg).await.is_ok());
    }

    #[test]
    fn test_atomic_swap_creation() {
        let config = BridgeConfig::new(vec![0, 1]);
        let bridge = CrossChainBridge::new(config).unwrap();

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

        assert!(bridge.create_swap(swap).is_ok());
    }

    #[test]
    fn test_bridge_stats() {
        let config = BridgeConfig::new(vec![0, 1, 2]);
        let bridge = CrossChainBridge::new(config).unwrap();

        bridge.connect_chain(0).unwrap();
        bridge.connect_chain(1).unwrap();

        let stats = bridge.get_stats();
        assert_eq!(stats.connected_chains, 2);
        assert_eq!(stats.total_chains, 3);
    }
}
