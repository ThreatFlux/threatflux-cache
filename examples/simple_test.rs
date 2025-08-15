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

    // Put operation
    cache.put("test_key".to_string(), test_data.clone()).await?;
    println!("✅ Put operation successful");

    // Get operation
    if let Some(retrieved) = cache.get(&"test_key".to_string()).await? {
        assert_eq!(retrieved, test_data);
        println!("✅ Get operation successful - data matches");
    } else {
        return Err("Failed to retrieve data from cache".into());
    }

    // Contains operation
    if cache.contains(&"test_key".to_string()).await? {
        println!("✅ Contains operation successful");
    } else {
        return Err("Contains check failed".into());
    }

    // Cache statistics
    let len = cache.len().await?;
    println!("📊 Cache has {} entries", len);

    // Remove operation
    if let Some(removed) = cache.remove(&"test_key".to_string()).await? {
        assert_eq!(removed, test_data);
        println!("✅ Remove operation successful");
    } else {
        return Err("Remove operation failed".into());
    }

    // Verify empty after removal
    if cache.is_empty().await? {
        println!("✅ Cache is empty after removal");
    } else {
        return Err("Cache should be empty after removal".into());
    }

    // Clear operation
    cache.clear().await?;
    println!("✅ Clear operation successful");

    println!("🎉 All tests passed! ThreatFlux Cache is working correctly.");

    Ok(())
}
