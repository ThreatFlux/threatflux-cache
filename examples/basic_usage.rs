//! Basic usage example for threatflux-cache

use serde::{Deserialize, Serialize};
use threatflux_cache::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
#[allow(clippy::type_complexity)]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create a cache with default configuration
    let config = CacheConfig::default()
        .with_max_entries_per_key(10)
        .with_max_total_entries(1000);

    let cache: Cache<String, User> = Cache::with_config(config).await?;

    // Store some users
    let user1 = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    let user2 = User {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    // Simple put/get operations
    cache.put("user:1".to_string(), user1.clone()).await?;
    cache.put("user:2".to_string(), user2.clone()).await?;

    // Retrieve a user
    if let Some(user) = cache.get(&"user:1".to_string()).await? {
        println!("Found user: {:?}", user);
    }

    // Check if a key exists
    if cache.contains(&"user:2".to_string()).await? {
        println!("User 2 exists in cache");
    }

    // Get cache statistics
    let stats = cache.get_stats().await;
    println!(
        "Cache stats: {} entries, {} keys",
        stats.total_entries, stats.total_keys
    );

    // Remove a user
    if let Some(removed) = cache.remove(&"user:1".to_string()).await? {
        println!("Removed user: {:?}", removed);
    }

    // Clear the cache
    cache.clear().await?;
    println!("Cache cleared");

    Ok(())
}
