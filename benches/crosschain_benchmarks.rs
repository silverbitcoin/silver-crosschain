use criterion::{black_box, criterion_group, criterion_main, Criterion};
use silver_crosschain::{
    atomic_swap::AtomicSwap, bridge::BridgeConfig, routing::MessageRouter, CrossChainBridge,
    CrossChainMessage, MessageType,
};

fn benchmark_message_creation(c: &mut Criterion) {
    c.bench_function("create_message", |b| {
        b.iter(|| {
            CrossChainMessage::new(
                black_box(0),
                black_box(1),
                black_box(MessageType::DataTransfer),
                black_box(vec![1, 2, 3, 4, 5]),
                black_box(vec![6, 7, 8, 9, 10]),
            )
        })
    });
}

fn benchmark_message_validation(c: &mut Criterion) {
    let msg = CrossChainMessage::new(
        0,
        1,
        MessageType::DataTransfer,
        vec![1, 2, 3],
        vec![4, 5, 6],
    )
    .unwrap();

    c.bench_function("validate_message", |b| {
        b.iter(|| msg.validate())
    });
}

fn benchmark_atomic_swap_creation(c: &mut Criterion) {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"secret");
    let hash_lock = hasher.finalize().as_bytes().to_vec();

    c.bench_function("create_atomic_swap", |b| {
        b.iter(|| {
            AtomicSwap::new(
                black_box(vec![1, 2, 3]),
                black_box(vec![4, 5, 6]),
                black_box(0),
                black_box(1),
                black_box(1000),
                black_box(hash_lock.clone()),
            )
        })
    });
}

fn benchmark_bridge_creation(c: &mut Criterion) {
    c.bench_function("create_bridge", |b| {
        b.iter(|| {
            let config = BridgeConfig::new(black_box(vec![0, 1, 2, 3]));
            CrossChainBridge::new(config)
        })
    });
}

fn benchmark_message_router(c: &mut Criterion) {
    c.bench_function("create_router", |b| {
        b.iter(|| MessageRouter::new(black_box(10000)))
    });
}

criterion_group!(
    benches,
    benchmark_message_creation,
    benchmark_message_validation,
    benchmark_atomic_swap_creation,
    benchmark_bridge_creation,
    benchmark_message_router
);
criterion_main!(benches);
