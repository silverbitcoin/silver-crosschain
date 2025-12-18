use silver_crosschain::{
    atomic_swap::AtomicSwap, bridge::BridgeConfig, CrossChainBridge, CrossChainMessage,
    MessageType,
};

#[tokio::test]
async fn test_multi_chain_bridge_setup() {
    let config = BridgeConfig::new(vec![0, 1, 2, 3]);
    let bridge = CrossChainBridge::new(config).unwrap();

    // Connect all chains
    for chain_id in 0..4 {
        assert!(bridge.connect_chain(chain_id).is_ok());
    }

    let connected = bridge.get_connected_chains();
    assert_eq!(connected.len(), 4);
}

#[tokio::test]
async fn test_cross_chain_message_flow() {
    let config = BridgeConfig::new(vec![0, 1, 2]);
    let bridge = CrossChainBridge::new(config).unwrap();

    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();
    bridge.connect_chain(2).unwrap();

    // Send message from chain 0 to chain 1
    let msg1 = CrossChainMessage::new(
        0,
        1,
        MessageType::DataTransfer,
        vec![1, 2, 3, 4, 5],
        vec![10, 11, 12],
    )
    .unwrap();

    assert!(bridge.route_message(msg1.clone()).await.is_ok());
    assert!(bridge.get_message(&msg1.id).is_some());

    // Send message from chain 1 to chain 2
    let msg2 = CrossChainMessage::new(
        1,
        2,
        MessageType::StateSync,
        vec![6, 7, 8],
        vec![13, 14, 15],
    )
    .unwrap();

    assert!(bridge.route_message(msg2.clone()).await.is_ok());
    assert!(bridge.get_message(&msg2.id).is_some());
}

#[tokio::test]
async fn test_atomic_swap_lifecycle() {
    let config = BridgeConfig::new(vec![0, 1]);
    let bridge = CrossChainBridge::new(config).unwrap();

    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();

    // Create hash lock
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"secret_key_123");
    let hash_lock = hasher.finalize().as_bytes().to_vec();

    // Create swap
    let mut swap = AtomicSwap::new(
        vec![100, 101, 102],
        vec![200, 201, 202],
        0,
        1,
        5000,
        hash_lock,
    )
    .unwrap();

    assert!(bridge.create_swap(swap.clone()).is_ok());
    assert!(bridge.get_swap(&swap.id).is_some());

    // Lock swap
    assert!(swap.lock().is_ok());
    assert!(bridge.update_swap(swap.clone()).is_ok());

    // Confirm swap with correct secret
    assert!(swap.confirm(b"secret_key_123".to_vec()).is_ok());
    assert!(bridge.update_swap(swap.clone()).is_ok());

    // Complete swap
    assert!(swap.complete().is_ok());
    assert!(bridge.update_swap(swap.clone()).is_ok());

    let final_swap = bridge.get_swap(&swap.id).unwrap();
    assert_eq!(final_swap.state, silver_crosschain::atomic_swap::SwapState::Completed);
}

#[tokio::test]
async fn test_bridge_statistics() {
    let config = BridgeConfig::new(vec![0, 1, 2, 3, 4]);
    let bridge = CrossChainBridge::new(config).unwrap();

    let stats_before = bridge.get_stats();
    assert_eq!(stats_before.connected_chains, 0);
    assert_eq!(stats_before.total_chains, 5);

    // Connect some chains
    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();
    bridge.connect_chain(2).unwrap();

    let stats_after = bridge.get_stats();
    assert_eq!(stats_after.connected_chains, 3);
    assert_eq!(stats_after.total_chains, 5);
}

#[tokio::test]
async fn test_message_routing_with_disconnected_chain() {
    let config = BridgeConfig::new(vec![0, 1]);
    let bridge = CrossChainBridge::new(config).unwrap();

    bridge.connect_chain(0).unwrap();
    // Don't connect chain 1

    let msg = CrossChainMessage::new(
        0,
        1,
        MessageType::DataTransfer,
        vec![1, 2, 3],
        vec![4, 5, 6],
    )
    .unwrap();

    // Should fail because destination chain is not connected
    assert!(bridge.route_message(msg).await.is_err());
}

#[tokio::test]
async fn test_multiple_swaps() {
    let config = BridgeConfig::new(vec![0, 1]);
    let bridge = CrossChainBridge::new(config).unwrap();

    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();

    // Create multiple swaps
    let mut swaps = Vec::new();
    for i in 0..5 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(format!("secret_{}", i).as_bytes());
        let hash_lock = hasher.finalize().as_bytes().to_vec();

        let swap = AtomicSwap::new(
            vec![100 + i as u8],
            vec![200 + i as u8],
            0,
            1,
            1000 * (i as u128 + 1),
            hash_lock,
        )
        .unwrap();

        assert!(bridge.create_swap(swap.clone()).is_ok());
        swaps.push(swap);
    }

    let all_swaps = bridge.get_all_swaps();
    assert_eq!(all_swaps.len(), 5);
}

#[tokio::test]
async fn test_chain_state_transitions() {
    let config = BridgeConfig::new(vec![0, 1]);
    let bridge = CrossChainBridge::new(config).unwrap();

    // Initially disconnected
    let chain_info = bridge.get_chain_info(0).unwrap();
    assert_eq!(
        chain_info.state,
        silver_crosschain::bridge::ChainState::Disconnected
    );

    // Connect
    bridge.connect_chain(0).unwrap();
    let chain_info = bridge.get_chain_info(0).unwrap();
    assert_eq!(
        chain_info.state,
        silver_crosschain::bridge::ChainState::Connected
    );

    // Disconnect
    bridge.disconnect_chain(0).unwrap();
    let chain_info = bridge.get_chain_info(0).unwrap();
    assert_eq!(
        chain_info.state,
        silver_crosschain::bridge::ChainState::Disconnected
    );
}

#[tokio::test]
async fn test_message_metadata() {
    let config = BridgeConfig::new(vec![0, 1]);
    let bridge = CrossChainBridge::new(config).unwrap();

    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();

    let mut msg = CrossChainMessage::new(
        0,
        1,
        MessageType::DataTransfer,
        vec![1, 2, 3],
        vec![4, 5, 6],
    )
    .unwrap();

    msg.add_metadata("priority".to_string(), vec![1]);
    msg.add_metadata("timestamp".to_string(), vec![2, 3, 4, 5]);

    assert!(bridge.route_message(msg.clone()).await.is_ok());

    let retrieved = bridge.get_message(&msg.id).unwrap();
    assert_eq!(retrieved.get_metadata("priority"), Some(&vec![1]));
    assert_eq!(
        retrieved.get_metadata("timestamp"),
        Some(&vec![2, 3, 4, 5])
    );
}

#[tokio::test]
async fn test_swap_expiration() {
    let config = BridgeConfig::new(vec![0, 1]);
    let bridge = CrossChainBridge::new(config).unwrap();

    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();

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

    assert!(!swap.is_expired());
    assert!(bridge.create_swap(swap).is_ok());
}

#[tokio::test]
async fn test_bridge_config_validation() {
    // Empty chains should fail
    let config = BridgeConfig::new(vec![]);
    assert!(config.validate().is_err());

    // Valid config
    let config = BridgeConfig::new(vec![0, 1, 2]);
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_concurrent_message_routing() {
    let config = BridgeConfig::new(vec![0, 1, 2]);
    let bridge = std::sync::Arc::new(CrossChainBridge::new(config).unwrap());

    bridge.connect_chain(0).unwrap();
    bridge.connect_chain(1).unwrap();
    bridge.connect_chain(2).unwrap();

    let mut handles = vec![];

    for i in 0..10 {
        let bridge_clone = bridge.clone();
        let handle = tokio::spawn(async move {
            let msg = CrossChainMessage::new(
                0,
                1,
                MessageType::DataTransfer,
                vec![i as u8],
                vec![100 + i as u8],
            )
            .unwrap();

            bridge_clone.route_message(msg).await
        });
        handles.push(handle);
    }

    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    let stats = bridge.get_stats();
    assert_eq!(stats.pending_messages, 10);
}
