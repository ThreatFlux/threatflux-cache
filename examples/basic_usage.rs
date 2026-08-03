//! Core cache operations using the default in-memory backend.

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
    let config = CacheConfig::default()
        .with_max_entries_per_key(10)
        .with_max_total_entries(1000);

    let cache: Cache<String, User> = Cache::with_config(config).await?;

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

    let user1_key = "user:1".to_owned();
    let user2_key = "user:2".to_owned();
    cache.put(user1_key.clone(), user1.clone()).await?;
    cache.put(user2_key.clone(), user2.clone()).await?;

    let user = cache.get(&user1_key).await?.expect("user not found");
    println!("Found user: {user:?}");

    assert!(cache.contains(&user2_key).await?);
    println!("User 2 exists in cache");

    let stats = cache.get_stats().await;
    println!(
        "Cache stats: {} entries, {} keys",
        stats.total_entries, stats.total_keys
    );

    let removed = cache.remove(&user1_key).await?.expect("user not found");
    println!("Removed user: {removed:?}");

    cache.clear().await?;
    println!("Cache cleared");

    Ok(())
}
