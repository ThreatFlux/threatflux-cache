//! Simple test example for threatflux-cache functionality
//! This example demonstrates basic cache operations without complex dependencies

use serde::{Deserialize, Serialize};
use std::error::Error;
use threatflux_cache::{AsyncCache, Cache, CacheConfig};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: u32,
    name: String,
    value: i32,
}

#[tokio::main]
#[allow(clippy::type_complexity)]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    println!("🚀 Testing ThreatFlux Cache Library");

    // Create a cache with default configuration
    let config = CacheConfig::default()
        .with_max_entries_per_key(5)
        .with_max_total_entries(100);

    let cache: Cache<String, TestData> = Cache::with_config(config).await?;

    // Test data
    let test_data = TestData {
        id: 1,
        name: "Test Item".to_string(),
        value: 42,
    };

    println!("✅ Cache created successfully");

    // Test basic operations
    println!("📝 Testing basic cache operations...");
    let key = "test_key".to_string();

    // Put operation
    cache.put(key.clone(), test_data.clone()).await?;
    println!("✅ Put operation successful");

    // Get operation
    assert_eq!(cache.get(&key).await?, Some(test_data.clone()));
    println!("✅ Get operation successful - data matches");

    // Contains operation
    assert!(cache.contains(&key).await?);
    println!("✅ Contains operation successful");

    // Cache statistics
    let len = cache.len().await?;
    println!("📊 Cache has {len} entries");

    // Remove operation
    let removed = cache.remove(&key).await?.expect("Remove operation failed");
    assert_eq!(removed, test_data);
    println!("✅ Remove operation successful");

    // Verify empty after removal
    assert!(cache.is_empty().await?);
    println!("✅ Cache is empty after removal");

    // Clear operation
    cache.clear().await?;
    println!("✅ Clear operation successful");

    println!("🎉 All tests passed! ThreatFlux Cache is working correctly.");

    Ok(())
}
