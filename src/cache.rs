//! Core cache implementation

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

use crate::{
    CacheConfig, CacheEntry, CacheError, EntryMetadata, Result, StorageBackend,
    eviction::{EvictionContext, EvictionStrategy},
    search::Searchable,
};

/// Type alias for cache entries storage
type CacheStorage<K, V, M> = Arc<RwLock<HashMap<K, Vec<CacheEntry<K, V, M>>>>>;

/// Type alias for eviction strategy
type EvictionStrategyBox<K, V, M> = Box<dyn EvictionStrategy<K, V, M>>;

/// Type alias for cache entry
type Entry<K, V, M> = CacheEntry<K, V, M>;

/// Async cache trait defining the core cache operations
#[async_trait]
pub trait AsyncCache<K, V>: Send + Sync
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Error type for cache operations
    type Error;

    /// Get a value from the cache
    async fn get(&self, key: &K) -> std::result::Result<Option<V>, Self::Error>;

    /// Put a value into the cache
    async fn put(&self, key: K, value: V) -> std::result::Result<(), Self::Error>;

    /// Remove a value from the cache
    async fn remove(&self, key: &K) -> std::result::Result<Option<V>, Self::Error>;

    /// Clear all entries from the cache
    async fn clear(&self) -> std::result::Result<(), Self::Error>;

    /// Check if the cache contains a key
    async fn contains(&self, key: &K) -> std::result::Result<bool, Self::Error>;

    /// Get the number of entries in the cache
    async fn len(&self) -> std::result::Result<usize, Self::Error>;

    /// Check if the cache is empty
    async fn is_empty(&self) -> std::result::Result<bool, Self::Error> {
        Ok(self.len().await? == 0)
    }
}

/// Main cache implementation
#[allow(clippy::type_complexity)]
pub struct Cache<K, V, M = (), B = crate::backends::memory::MemoryBackend<K, V, M>>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    entries: CacheStorage<K, V, M>,
    config: CacheConfig,
    backend: Arc<B>,
    save_semaphore: Arc<Semaphore>,
    operation_count: Arc<RwLock<usize>>,
    eviction_strategy: EvictionStrategyBox<K, V, M>,
}

impl<K, V, M, B> Cache<K, V, M, B>
where
    K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    /// Create a new cache with the given configuration and backend
    pub async fn new(config: CacheConfig, backend: B) -> Result<Self> {
        let eviction_strategy = crate::eviction::create_strategy(&config.eviction_policy);

        let cache = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
            backend: Arc::new(backend),
            save_semaphore: Arc::new(Semaphore::new(1)),
            operation_count: Arc::new(RwLock::new(0)),
            eviction_strategy,
        };

        // Load existing cache if configured
        if cache.config.persistence.enabled && cache.config.persistence.load_on_startup {
            let _ = cache.load_from_storage().await;
        }

        Ok(cache)
    }

    /// Create a new cache with default memory backend
    pub async fn with_config(config: CacheConfig) -> Result<Self>
    where
        B: Default,
    {
        Self::new(config, B::default()).await
    }

    /// Add an entry to the cache
    #[allow(clippy::type_complexity)]
    pub async fn add_entry(&self, entry: Entry<K, V, M>) -> Result<()> {
        let key = entry.key.clone();

        {
            let mut entries = self.entries.write().await;
            let key_entries = entries.entry(key).or_insert_with(Vec::new);
            key_entries.push(entry);

            // Limit entries per key
            if key_entries.len() > self.config.max_entries_per_key {
                key_entries.remove(0);
            }

            // Check if we need to evict
            let total_entries: usize = entries.values().map(|v| v.len()).sum();
            if total_entries > self.config.max_total_entries {
                let context = EvictionContext {
                    max_total_entries: self.config.max_total_entries,
                    current_total_entries: total_entries,
                };
                self.eviction_strategy.evict(&mut entries, &context).await;
            }
        }

        // Increment operation count and check if we need to sync
        self.increment_and_maybe_sync().await?;

        Ok(())
    }

    /// Get all entries for a key
    pub async fn get_entries(&self, key: &K) -> Option<Vec<CacheEntry<K, V, M>>> {
        let mut entries = self.entries.write().await;
        entries.get_mut(key).map(|entries| {
            // Update access statistics
            for entry in entries.iter_mut() {
                entry.record_access();
            }
            entries.clone()
        })
    }

    /// Get the latest entry for a key
    pub async fn get_latest(&self, key: &K) -> Option<CacheEntry<K, V, M>> {
        let mut entries = self.entries.write().await;
        entries.get_mut(key).and_then(|entries| {
            entries.iter_mut().max_by_key(|e| e.timestamp).map(|e| {
                e.record_access();
                e.clone()
            })
        })
    }

    /// Search entries based on a query
    pub async fn search<Q>(&self, query: &Q) -> Vec<CacheEntry<K, V, M>>
    where
        CacheEntry<K, V, M>: Searchable<Query = Q>,
    {
        let entries = self.entries.read().await;
        entries
            .values()
            .flat_map(|v| v.iter())
            .filter(|entry| entry.matches(query))
            .cloned()
            .collect()
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let entries = self.entries.read().await;

        let total_entries: usize = entries.values().map(|v| v.len()).sum();
        let total_keys = entries.len();
        let mut total_access_count = 0u64;
        let mut expired_count = 0usize;

        for entry_vec in entries.values() {
            for entry in entry_vec {
                total_access_count += entry.access_count;
                if entry.is_expired() {
                    expired_count += 1;
                }
            }
        }

        CacheStats {
            total_entries,
            total_keys,
            total_access_count,
            expired_count,
            memory_usage_bytes: 0, // Would need size estimation
        }
    }

    /// Save cache to storage backend
    async fn save_to_storage(&self) -> Result<()> {
        if !self.config.persistence.enabled {
            return Ok(());
        }

        let _permit = self.save_semaphore.acquire().await.unwrap();
        let entries = self.entries.read().await;
        self.backend.save(&entries).await
    }

    /// Load cache from storage backend
    async fn load_from_storage(&self) -> Result<()> {
        if !self.config.persistence.enabled {
            return Ok(());
        }

        let loaded_entries = self.backend.load().await?;
        let mut entries = self.entries.write().await;
        *entries = loaded_entries;
        Ok(())
    }

    /// Increment operation count and sync if needed
    async fn increment_and_maybe_sync(&self) -> Result<()> {
        let mut count = self.operation_count.write().await;
        *count += 1;

        if *count >= self.config.persistence.sync_interval {
            *count = 0;
            drop(count); // Release the lock before saving

            // Spawn background save
            let cache = self.clone();
            tokio::spawn(async move {
                let _ = cache.save_to_storage().await;
            });
        }

        Ok(())
    }
}

// Implement Clone for Cache
impl<K, V, M, B> Clone for Cache<K, V, M, B>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            config: self.config.clone(),
            backend: Arc::clone(&self.backend),
            save_semaphore: Arc::clone(&self.save_semaphore),
            operation_count: Arc::clone(&self.operation_count),
            eviction_strategy: crate::eviction::create_strategy(&self.config.eviction_policy),
        }
    }
}

// Implement AsyncCache trait
#[async_trait]
impl<K, V, M, B> AsyncCache<K, V> for Cache<K, V, M, B>
where
    K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    type Error = CacheError;

    async fn get(&self, key: &K) -> std::result::Result<Option<V>, Self::Error> {
        Ok(self.get_latest(key).await.map(|entry| entry.value))
    }

    async fn put(&self, key: K, value: V) -> std::result::Result<(), Self::Error> {
        {
            let mut entries = self.entries.write().await;
            let key_entries = entries.entry(key.clone()).or_insert_with(Vec::new);

            // For AsyncCache trait, replace existing entries rather than add
            key_entries.clear();
            key_entries.push(CacheEntry::new(key, value));
        }

        // Increment operation count and check if we need to sync
        self.increment_and_maybe_sync().await?;
        Ok(())
    }

    async fn remove(&self, key: &K) -> std::result::Result<Option<V>, Self::Error> {
        let mut entries = self.entries.write().await;
        let removed = entries.remove(key);

        if removed.is_some() {
            // Remove from backend
            self.backend.remove(key).await?;
            self.increment_and_maybe_sync().await?;
        }

        Ok(removed.and_then(|entries| entries.into_iter().next_back().map(|e| e.value)))
    }

    async fn clear(&self) -> std::result::Result<(), Self::Error> {
        let mut entries = self.entries.write().await;
        entries.clear();

        self.backend.clear().await?;

        Ok(())
    }

    async fn contains(&self, key: &K) -> std::result::Result<bool, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries.contains_key(key))
    }

    async fn len(&self) -> std::result::Result<usize, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries.values().map(|v| v.len()).sum())
    }
}

// Implement Drop to save on shutdown
impl<K, V, M, B> Drop for Cache<K, V, M, B>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    fn drop(&mut self) {
        if self.config.persistence.enabled && self.config.persistence.save_on_drop {
            // Try to save synchronously in drop
            let entries = self.entries.clone();
            let backend = self.backend.clone();

            // We can't use async in drop, so we try to block on the runtime if available
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let _ = handle.block_on(async move {
                    let entries = entries.read().await;
                    backend.save(&entries).await
                });
            }
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Total number of unique keys
    pub total_keys: usize,
    /// Total access count across all entries
    pub total_access_count: u64,
    /// Number of expired entries
    pub expired_count: usize,
    /// Approximate memory usage in bytes
    pub memory_usage_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::memory::MemoryBackend;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let config = CacheConfig::default();
        let backend = MemoryBackend::new();
        let cache: Cache<String, String> = Cache::new(config, backend).await.unwrap();

        // Test put and get
        cache
            .put("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        let value = cache.get(&"key1".to_string()).await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Test contains
        assert!(cache.contains(&"key1".to_string()).await.unwrap());
        assert!(!cache.contains(&"key2".to_string()).await.unwrap());

        // Test len
        assert_eq!(cache.len().await.unwrap(), 1);

        // Test remove
        let removed = cache.remove(&"key1".to_string()).await.unwrap();
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(cache.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let config = CacheConfig::default();
        let backend = MemoryBackend::new();
        let cache: Cache<String, String> = Cache::new(config, backend).await.unwrap();

        cache
            .put("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .put("key2".to_string(), "value2".to_string())
            .await
            .unwrap();

        assert_eq!(cache.len().await.unwrap(), 2);

        cache.clear().await.unwrap();
        assert_eq!(cache.len().await.unwrap(), 0);
        assert!(!cache.contains(&"key1".to_string()).await.unwrap());
    }
}
