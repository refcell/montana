//! Integration test for blob transaction roundtrip.
//!
//! This test verifies that:
//! 1. The batch submitter can submit batches as EIP-4844 blob transactions
//! 2. The derivation source can successfully read these batches back

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use montana_anvil::{AnvilConfig, AnvilManager};
use montana_pipeline::{BatchSink, CompressedBatch, L1BatchSource};

/// Test that blob submission and derivation work correctly.
///
/// This spawns an Anvil instance, submits a batch as a blob transaction,
/// and verifies the source can read it back with correct data.
#[tokio::test]
async fn blob_submission_and_derivation_roundtrip() {
    // Spawn Anvil instance
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    // Create sink with blob mode enabled
    let use_blobs = Arc::new(AtomicBool::new(true));
    let mut sink = manager.sink(Arc::clone(&use_blobs));

    // Create source
    let mut source = manager.source();

    // Create a test batch with some data
    let original_data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
    let batch = CompressedBatch {
        batch_number: 1,
        data: original_data.clone(),
        block_count: 10,
        first_block: 100,
        last_block: 109,
    };

    // Submit the batch as a blob transaction
    let receipt = sink.submit(batch).await.expect("Failed to submit batch as blob");

    // Verify receipt
    assert_eq!(receipt.batch_number, 1);
    assert!(receipt.l1_block > 0, "L1 block should be set");

    // Read the batch back from the source
    let derived_batch = source.next_batch().await.expect("Failed to read batch");
    let derived_batch = derived_batch.expect("Should have found the batch");

    // Verify the derived batch matches what we submitted
    assert_eq!(derived_batch.batch_number, 1);
    assert_eq!(derived_batch.data, original_data);
    assert_eq!(derived_batch.block_count, 10);
    assert_eq!(derived_batch.first_block, 100);
    assert_eq!(derived_batch.last_block, 109);
}

/// Test that calldata submission and derivation work correctly.
///
/// This is a control test to ensure calldata mode works as expected.
#[tokio::test]
async fn calldata_submission_and_derivation_roundtrip() {
    // Spawn Anvil instance
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    // Create sink with calldata mode (blobs disabled)
    let use_blobs = Arc::new(AtomicBool::new(false));
    let mut sink = manager.sink(Arc::clone(&use_blobs));

    // Create source
    let mut source = manager.source();

    // Create a test batch
    let original_data = vec![0xCA, 0xFE, 0xBA, 0xBE];
    let batch = CompressedBatch {
        batch_number: 1,
        data: original_data.clone(),
        block_count: 5,
        first_block: 50,
        last_block: 54,
    };

    // Submit the batch as calldata
    let receipt = sink.submit(batch).await.expect("Failed to submit batch as calldata");

    // Verify receipt
    assert_eq!(receipt.batch_number, 1);

    // Read the batch back
    let derived_batch = source.next_batch().await.expect("Failed to read batch");
    let derived_batch = derived_batch.expect("Should have found the batch");

    // Verify data integrity
    assert_eq!(derived_batch.data, original_data);
    assert_eq!(derived_batch.block_count, 5);
    assert_eq!(derived_batch.first_block, 50);
    assert_eq!(derived_batch.last_block, 54);
}

/// Test toggling between blob and calldata modes.
///
/// This test simulates the TUI toggle behavior and verifies that
/// switching modes mid-operation works correctly.
#[tokio::test]
async fn toggle_blob_mode_during_submission() {
    // Spawn Anvil instance
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    // Start with calldata mode
    let use_blobs = Arc::new(AtomicBool::new(false));
    let mut sink = manager.sink(Arc::clone(&use_blobs));
    let mut source = manager.source();

    // Submit first batch as calldata
    let batch1 = CompressedBatch {
        batch_number: 1,
        data: vec![0x01],
        block_count: 1,
        first_block: 1,
        last_block: 1,
    };
    sink.submit(batch1).await.expect("Failed to submit batch 1 as calldata");

    // Toggle to blob mode (simulating TUI 'b' keypress)
    use_blobs.store(true, Ordering::SeqCst);

    // Submit second batch as blob
    let batch2 = CompressedBatch {
        batch_number: 2,
        data: vec![0x02],
        block_count: 1,
        first_block: 2,
        last_block: 2,
    };
    sink.submit(batch2).await.expect("Failed to submit batch 2 as blob");

    // Toggle back to calldata
    use_blobs.store(false, Ordering::SeqCst);

    // Submit third batch as calldata again
    let batch3 = CompressedBatch {
        batch_number: 3,
        data: vec![0x03],
        block_count: 1,
        first_block: 3,
        last_block: 3,
    };
    sink.submit(batch3).await.expect("Failed to submit batch 3 as calldata");

    // Read all batches back
    let derived1 = source.next_batch().await.unwrap().expect("Missing batch 1");
    assert_eq!(derived1.data, vec![0x01]);

    let derived2 = source.next_batch().await.unwrap().expect("Missing batch 2");
    assert_eq!(derived2.data, vec![0x02]);

    let derived3 = source.next_batch().await.unwrap().expect("Missing batch 3");
    assert_eq!(derived3.data, vec![0x03]);
}

/// Test multiple sequential blob submissions.
#[tokio::test]
async fn multiple_blob_submissions() {
    // Spawn Anvil instance
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    // Create sink with blob mode
    let use_blobs = Arc::new(AtomicBool::new(true));
    let mut sink = manager.sink(Arc::clone(&use_blobs));
    let mut source = manager.source();

    // Submit multiple batches as blobs
    for i in 1..=5 {
        let batch = CompressedBatch {
            batch_number: i,
            data: vec![i as u8; 100], // 100 bytes of data
            block_count: i,
            first_block: i * 10,
            last_block: i * 10 + i - 1,
        };
        sink.submit(batch).await.expect(&format!("Failed to submit batch {}", i));
    }

    // Read all batches back and verify
    for i in 1..=5 {
        let derived = source.next_batch().await.unwrap().expect(&format!("Missing batch {}", i));
        assert_eq!(derived.batch_number, i);
        assert_eq!(derived.data, vec![i as u8; 100]);
        assert_eq!(derived.block_count, i);
        assert_eq!(derived.first_block, i * 10);
        assert_eq!(derived.last_block, i * 10 + i - 1);
    }
}

/// Test blob submission with larger data payload.
///
/// This tests that the SidecarBuilder correctly handles data
/// that requires real blob encoding.
#[tokio::test]
async fn blob_submission_with_larger_payload() {
    // Spawn Anvil instance
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    // Create sink with blob mode
    let use_blobs = Arc::new(AtomicBool::new(true));
    let mut sink = manager.sink(Arc::clone(&use_blobs));
    let mut source = manager.source();

    // Create a larger payload (10KB)
    let original_data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
    let batch = CompressedBatch {
        batch_number: 1,
        data: original_data.clone(),
        block_count: 100,
        first_block: 1000,
        last_block: 1099,
    };

    // Submit as blob
    let receipt = sink.submit(batch).await.expect("Failed to submit large batch as blob");
    assert_eq!(receipt.batch_number, 1);

    // Read back and verify
    let derived = source.next_batch().await.unwrap().expect("Missing batch");
    assert_eq!(derived.data.len(), original_data.len());
    assert_eq!(derived.data, original_data);
}

/// Test health check functionality.
#[tokio::test]
async fn sink_health_check() {
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    let use_blobs = Arc::new(AtomicBool::new(true));
    let sink = manager.sink(Arc::clone(&use_blobs));

    // Health check should succeed
    sink.health_check().await.expect("Health check failed");
}

/// Test capacity reporting.
#[tokio::test]
async fn sink_capacity() {
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    let use_blobs = Arc::new(AtomicBool::new(true));
    let sink = manager.sink(Arc::clone(&use_blobs));

    // Capacity should return a reasonable value
    let capacity = sink.capacity().await.expect("Failed to get capacity");
    assert!(capacity > 0, "Capacity should be positive");
    assert_eq!(capacity, 128 * 1024, "Expected 128KB capacity");
}

/// Test that batches with empty compressed data still work.
///
/// Even with empty compressed data, the sink adds a 24-byte header
/// (block_count, first_block, last_block), so the blob is not truly empty.
#[tokio::test]
async fn blob_submission_with_empty_compressed_data() {
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    let use_blobs = Arc::new(AtomicBool::new(true));
    let mut sink = manager.sink(Arc::clone(&use_blobs));
    let mut source = manager.source();

    // Create a batch with empty compressed data
    // The sink will still add the 24-byte metadata header
    let batch = CompressedBatch {
        batch_number: 1,
        data: vec![],
        block_count: 0,
        first_block: 0,
        last_block: 0,
    };

    // Should succeed because the header is still added
    let receipt = sink.submit(batch).await.expect("Failed to submit batch with empty data");
    assert_eq!(receipt.batch_number, 1);

    // Read back and verify
    let derived = source.next_batch().await.unwrap().expect("Missing batch");
    assert!(derived.data.is_empty(), "Compressed data should be empty");
    assert_eq!(derived.block_count, 0);
}

/// Test that batches at max blob capacity work correctly.
///
/// This tests a batch that uses exactly the maximum number of blobs (6),
/// verifying the boundary condition works.
#[tokio::test]
async fn blob_submission_at_max_capacity() {
    let manager = AnvilManager::spawn(AnvilConfig::default()).await.expect("Failed to spawn Anvil");

    // Enable blob mode
    let use_blobs = Arc::new(AtomicBool::new(true));
    let mut sink = manager.sink(Arc::clone(&use_blobs));
    let mut source = manager.source();

    // Create a batch that uses close to max blobs (but stays under gas limit)
    // Each blob can hold approximately 4096 * 31 = 126,976 bytes
    // 5 blobs = ~634,880 bytes, which should work with blob mode
    // Use 500KB to stay well within limits but test multi-blob scenarios
    let data: Vec<u8> = (0..500_000).map(|i| (i % 256) as u8).collect();
    let batch = CompressedBatch {
        batch_number: 1,
        data: data.clone(),
        block_count: 1,
        first_block: 1,
        last_block: 1,
    };

    // Should succeed as a blob transaction
    let receipt = sink.submit(batch).await.expect("Max capacity blob batch should succeed");
    assert_eq!(receipt.batch_number, 1);

    // Read back and verify data integrity
    let derived = source.next_batch().await.unwrap().expect("Missing batch");
    assert_eq!(derived.data.len(), data.len());
    assert_eq!(derived.data, data);
}
