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
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    println!("🚀 Testing ThreatFlux Cache Library");

    let cache = setup_cache().await?;
    println!("✅ Cache created successfully");

    run_basic_operations(&cache).await?;

    println!("🎉 All tests passed! ThreatFlux Cache is working correctly.");
    Ok(())
}

async fn setup_cache() -> Result<Cache<String, TestData>, Box<dyn Error>> {
    let config = CacheConfig::default()
        .with_max_entries_per_key(5)
        .with_max_total_entries(100);
    Cache::with_config(config).await.map_err(Into::into)
}

async fn run_basic_operations(cache: &Cache<String, TestData>) -> Result<(), Box<dyn Error>> {
    let test_data = TestData {
        id: 1,
        name: "Test Item".to_string(),
        value: 42,
    };

    println!("📝 Testing basic cache operations...");
    let key = "test_key".to_string();

    cache.put(key.clone(), test_data.clone()).await?;
    println!("✅ Put operation successful");

    assert_eq!(cache.get(&key).await?, Some(test_data.clone()));
    println!("✅ Get operation successful - data matches");

    assert!(cache.contains(&key).await?);
    println!("✅ Contains operation successful");

    let len = cache.len().await?;
    println!("📊 Cache has {len} entries");

    let removed = cache.remove(&key).await?.expect("Remove operation failed");
    assert_eq!(removed, test_data);
    println!("✅ Remove operation successful");

    assert!(cache.is_empty().await?);
    println!("✅ Cache is empty after removal");

    cache.clear().await?;
    println!("✅ Clear operation successful");
    Ok(())
}
