//! Cross-chain message routing

use crate::{CrossChainError, CrossChainMessage, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Message route information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub source_chain: u32,
    pub dest_chain: u32,
    pub hops: Vec<u32>,
    pub latency_ms: u64,
    pub reliability: f32,
}

impl Route {
    pub fn new(source_chain: u32, dest_chain: u32) -> Self {
        Self {
            source_chain,
            dest_chain,
            hops: vec![source_chain, dest_chain],
            latency_ms: 0,
            reliability: 1.0,
        }
    }

    pub fn with_hops(mut self, hops: Vec<u32>) -> Self {
        self.hops = hops;
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn with_reliability(mut self, reliability: f32) -> Self {
        self.reliability = (reliability.max(0.0)).min(1.0);
        self
    }
}

/// Message router for cross-chain communication
pub struct MessageRouter {
    routes: Arc<DashMap<(u32, u32), Route>>,
    pending_messages: Arc<DashMap<Vec<u8>, CrossChainMessage>>,
    processed_messages: Arc<DashMap<Vec<u8>, u64>>,
    max_pending: usize,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new(max_pending: usize) -> Self {
        info!("Creating message router with max_pending={}", max_pending);

        Self {
            routes: Arc::new(DashMap::new()),
            pending_messages: Arc::new(DashMap::new()),
            processed_messages: Arc::new(DashMap::new()),
            max_pending,
        }
    }

    /// Register a route between chains
    pub fn register_route(&self, route: Route) -> Result<()> {
        debug!(
            "Registering route from chain {} to chain {}",
            route.source_chain, route.dest_chain
        );

        self.routes.insert((route.source_chain, route.dest_chain), route);
        Ok(())
    }

    /// Get route between chains
    pub fn get_route(&self, source: u32, dest: u32) -> Option<Route> {
        self.routes.get(&(source, dest)).map(|r| r.clone())
    }

    /// Route a message
    pub async fn route_message(&self, message: CrossChainMessage) -> Result<()> {
        // Validate message
        message.validate()?;

        // Check for duplicate
        if self.processed_messages.contains_key(&message.id) {
            return Err(CrossChainError::DuplicateMessage);
        }

        // Check pending message limit
        if self.pending_messages.len() >= self.max_pending {
            return Err(CrossChainError::RoutingFailed(
                "Too many pending messages".to_string(),
            ));
        }

        // Get route
        let route = self.get_route(message.source_chain, message.dest_chain)
            .ok_or_else(|| {
                CrossChainError::RoutingFailed(
                    format!(
                        "No route from chain {} to chain {}",
                        message.source_chain, message.dest_chain
                    ),
                )
            })?;

        debug!(
            "Routing message {} via {} hops",
            hex::encode(&message.id),
            route.hops.len()
        );

        // Add to pending messages
        self.pending_messages.insert(message.id.clone(), message.clone());

        // Mark as processed
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.processed_messages.insert(message.id.clone(), now);

        info!(
            "Message {} routed successfully",
            hex::encode(&message.id)
        );

        Ok(())
    }

    /// Get pending message
    pub fn get_pending_message(&self, message_id: &[u8]) -> Option<CrossChainMessage> {
        self.pending_messages.get(message_id).map(|m| m.clone())
    }

    /// Remove pending message
    pub fn remove_pending_message(&self, message_id: &[u8]) -> Option<CrossChainMessage> {
        self.pending_messages.remove(message_id).map(|(_, m)| m)
    }

    /// Get pending message count
    pub fn pending_count(&self) -> usize {
        self.pending_messages.len()
    }

    /// Get processed message count
    pub fn processed_count(&self) -> usize {
        self.processed_messages.len()
    }

    /// Clear old processed messages
    pub fn cleanup_old_messages(&self, max_age_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut to_remove = Vec::new();
        for entry in self.processed_messages.iter() {
            if now - entry.value() > max_age_secs {
                to_remove.push(entry.key().clone());
            }
        }

        for key in to_remove {
            self.processed_messages.remove(&key);
        }

        debug!("Cleaned up old processed messages");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_creation() {
        let route = Route::new(0, 1);
        assert_eq!(route.source_chain, 0);
        assert_eq!(route.dest_chain, 1);
        assert_eq!(route.hops.len(), 2);
    }

    #[test]
    fn test_router_creation() {
        let router = MessageRouter::new(1000);
        assert_eq!(router.pending_count(), 0);
        assert_eq!(router.processed_count(), 0);
    }

    #[tokio::test]
    async fn test_route_registration() {
        let router = MessageRouter::new(1000);
        let route = Route::new(0, 1);

        assert!(router.register_route(route).is_ok());
        assert!(router.get_route(0, 1).is_some());
    }

    #[tokio::test]
    async fn test_message_routing() {
        let router = MessageRouter::new(1000);
        let route = Route::new(0, 1);
        router.register_route(route).unwrap();

        let msg = CrossChainMessage::new(
            0,
            1,
            crate::MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        )
        .unwrap();

        assert!(router.route_message(msg).await.is_ok());
        assert_eq!(router.pending_count(), 1);
    }

    #[tokio::test]
    async fn test_duplicate_message() {
        let router = MessageRouter::new(1000);
        let route = Route::new(0, 1);
        router.register_route(route).unwrap();

        let msg = CrossChainMessage::new(
            0,
            1,
            crate::MessageType::DataTransfer,
            vec![1, 2, 3],
            vec![4, 5, 6],
        )
        .unwrap();

        assert!(router.route_message(msg.clone()).await.is_ok());
        assert!(router.route_message(msg).await.is_err());
    }
}
