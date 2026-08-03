//! Core cache implementation

use async_trait::async_trait;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

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

macro_rules! impl_cache_common {
    ($(#[$meta:meta])? $trait:path, $($body:tt)*) => {
        $(#[$meta])?
        impl<K, V, M, B> $trait for Cache<K, V, M, B>
        where
            K: CacheKey,
            V: CacheValue,
            M: EntryMetadata + Default,
            B: StorageBackend<Key = K, Value = V, Metadata = M>,
        {
            $($body)*
        }
    };
}

/// Common bounds for cache keys
pub trait CacheKey: Hash + Eq + Clone + Send + Sync + 'static {}
impl<T> CacheKey for T where T: Hash + Eq + Clone + Send + Sync + 'static {}

/// Common bounds for cache values
pub trait CacheValue: Clone + Send + Sync + 'static {}
impl<T> CacheValue for T where T: Clone + Send + Sync + 'static {}

/// Async cache trait defining the core cache operations
#[async_trait]
pub trait AsyncCache<K, V>: Send + Sync
where
    K: CacheKey,
    V: CacheValue,
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
    K: CacheKey,
    V: CacheValue,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    entries: CacheStorage<K, V, M>,
    config: CacheConfig,
    backend: Arc<B>,
    operation_lock: Arc<Mutex<()>>,
    operation_count: Arc<RwLock<usize>>,
    eviction_strategy: EvictionStrategyBox<K, V, M>,
}

impl<K, V, M, B> Cache<K, V, M, B>
where
    K: CacheKey,
    V: CacheValue,
    M: EntryMetadata + Default,
    B: StorageBackend<Key = K, Value = V, Metadata = M>,
{
    /// Create a new cache with the given configuration and backend
    pub async fn new(config: CacheConfig, backend: B) -> Result<Self> {
        Self::validate_config(&config)?;
        let eviction_strategy = crate::eviction::create_strategy(&config.eviction_policy);

        let cache = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
            backend: Arc::new(backend),
            operation_lock: Arc::new(Mutex::new(())),
            operation_count: Arc::new(RwLock::new(0)),
            eviction_strategy,
        };

        // Load existing cache if configured
        if cache.config.persistence.enabled && cache.config.persistence.load_on_startup {
            cache.load_from_storage().await?;
        }

        Ok(cache)
    }

    fn validate_config(config: &CacheConfig) -> Result<()> {
        if config.max_entries_per_key == 0 {
            return Err(CacheError::InvalidConfiguration(
                "max_entries_per_key must be greater than zero".to_string(),
            ));
        }
        if config.max_total_entries == 0 {
            return Err(CacheError::InvalidConfiguration(
                "max_total_entries must be greater than zero".to_string(),
            ));
        }
        if config.persistence.enabled && config.persistence.sync_interval == 0 {
            return Err(CacheError::InvalidConfiguration(
                "persistence.sync_interval must be greater than zero".to_string(),
            ));
        }
        if let Some(ttl) = config.default_ttl {
            chrono::Duration::from_std(ttl).map_err(|_| {
                CacheError::InvalidConfiguration(
                    "default_ttl exceeds the supported timestamp range".to_string(),
                )
            })?;
        }
        Ok(())
    }

    fn apply_default_ttl(&self, mut entry: Entry<K, V, M>) -> Result<Entry<K, V, M>> {
        if entry.expiry.is_none()
            && let Some(ttl) = self.config.default_ttl
        {
            let ttl = chrono::Duration::from_std(ttl).map_err(|_| {
                CacheError::InvalidConfiguration(
                    "default_ttl exceeds the supported timestamp range".to_string(),
                )
            })?;
            entry = entry.with_ttl(ttl);
        }
        Ok(entry)
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
        let _operation = self.operation_lock.lock().await;
        let entry = self.apply_default_ttl(entry)?;
        {
            let mut entries = self.entries.write().await;
            self.insert_entry(&mut entries, entry).await?;
        }

        // Increment operation count and check if we need to sync
        self.increment_and_maybe_sync().await?;

        Ok(())
    }

    async fn insert_entry(
        &self,
        entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        entry: Entry<K, V, M>,
    ) -> Result<()> {
        entries.retain(|_, key_entries| {
            key_entries.retain(|entry| !entry.is_expired());
            !key_entries.is_empty()
        });

        let total_entries = entries.values().try_fold(0usize, |total, key_entries| {
            total
                .checked_add(key_entries.len())
                .ok_or_else(|| CacheError::CapacityExceeded {
                    message: "cache entry count overflowed usize".to_string(),
                })
        })?;
        let grows = entries
            .get(&entry.key)
            .is_none_or(|key_entries| key_entries.len() < self.config.max_entries_per_key);
        if grows
            && total_entries >= self.config.max_total_entries
            && self.config.eviction_policy == crate::EvictionPolicy::None
        {
            return Err(CacheError::CapacityExceeded {
                message: format!(
                    "max_total_entries ({}) has been reached and eviction is disabled",
                    self.config.max_total_entries
                ),
            });
        }

        let key_entries = entries.entry(entry.key.clone()).or_default();
        key_entries.push(entry);
        key_entries.sort_by_key(|entry| entry.timestamp);

        // Limit entries per key while retaining the newest timestamps.
        if key_entries.len() > self.config.max_entries_per_key {
            let excess = key_entries.len() - self.config.max_entries_per_key;
            key_entries.drain(..excess);
        }

        // Check if we need to evict
        let total_entries = entries.values().try_fold(0usize, |total, key_entries| {
            total
                .checked_add(key_entries.len())
                .ok_or_else(|| CacheError::CapacityExceeded {
                    message: "cache entry count overflowed usize".to_string(),
                })
        })?;
        if total_entries > self.config.max_total_entries {
            let context = EvictionContext {
                max_total_entries: self.config.max_total_entries,
                current_total_entries: total_entries,
            };
            self.eviction_strategy.evict(entries, &context).await;
        }
        Ok(())
    }

    /// Get all entries for a key
    pub async fn get_entries(&self, key: &K) -> Option<Vec<CacheEntry<K, V, M>>> {
        let mut entries = self.entries.write().await;
        let result = entries.get_mut(key).and_then(|key_entries| {
            key_entries.retain(|entry| !entry.is_expired());
            if key_entries.is_empty() {
                None
            } else {
                for entry in key_entries.iter_mut() {
                    entry.record_access();
                }
                Some(key_entries.clone())
            }
        });
        if result.is_none() {
            entries.remove(key);
        }
        result
    }

    /// Get the latest entry for a key
    pub async fn get_latest(&self, key: &K) -> Option<CacheEntry<K, V, M>> {
        let mut entries = self.entries.write().await;
        let result = entries.get_mut(key).and_then(|key_entries| {
            key_entries.retain(|entry| !entry.is_expired());
            key_entries
                .iter_mut()
                .max_by_key(|entry| entry.timestamp)
                .map(|entry| {
                    entry.record_access();
                    entry.clone()
                })
        });
        if result.is_none() {
            entries.remove(key);
        }
        result
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

    /// Aggregate statistics for a slice of cache entries
    fn entry_vec_stats(entry_vec: &[CacheEntry<K, V, M>]) -> (usize, u64, usize) {
        entry_vec
            .iter()
            .fold((0, 0, 0), |(count, access, expired), entry| {
                (
                    count.saturating_add(1),
                    access.saturating_add(entry.access_count),
                    expired.saturating_add(usize::from(entry.is_expired())),
                )
            })
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let entries = self.entries.read().await;
        let total_keys = entries.len();

        let (total_entries, total_access_count, expired_count) =
            entries
                .values()
                .fold((0usize, 0u64, 0usize), |acc, entry_vec| {
                    let (e, a, exp) = Self::entry_vec_stats(entry_vec);
                    (
                        acc.0.saturating_add(e),
                        acc.1.saturating_add(a),
                        acc.2.saturating_add(exp),
                    )
                });

        CacheStats {
            total_entries,
            total_keys,
            total_access_count,
            expired_count,
        }
    }

    /// Save cache to storage backend
    async fn save_to_storage(&self) -> Result<()> {
        if !self.config.persistence.enabled {
            return Ok(());
        }

        let snapshot = self.entries.read().await.clone();
        self.backend.save(&snapshot).await
    }

    /// Persist the current cache state and wait for the backend to finish.
    ///
    /// This is a no-op when persistence is disabled. Call this before shutdown
    /// when durability of recent operations is required.
    pub async fn flush(&self) -> Result<()> {
        let _operation = self.operation_lock.lock().await;
        self.save_to_storage().await?;
        *self.operation_count.write().await = 0;
        Ok(())
    }

    /// Load cache from storage backend
    async fn load_from_storage(&self) -> Result<()> {
        if !self.config.persistence.enabled {
            return Ok(());
        }

        let mut loaded_entries = self.backend.load().await?;
        for (key, key_entries) in &mut loaded_entries {
            if key_entries.iter().any(|entry| &entry.key != key) {
                return Err(CacheError::Deserialization(
                    "persisted entry key does not match its containing key".to_string(),
                ));
            }
            key_entries.retain(|entry| !entry.is_expired());
            key_entries.sort_by_key(|entry| entry.timestamp);
            if key_entries.len() > self.config.max_entries_per_key {
                let excess = key_entries.len() - self.config.max_entries_per_key;
                key_entries.drain(..excess);
            }
        }
        loaded_entries.retain(|_, key_entries| !key_entries.is_empty());

        let total_entries = loaded_entries
            .values()
            .try_fold(0usize, |total, key_entries| {
                total
                    .checked_add(key_entries.len())
                    .ok_or_else(|| CacheError::CapacityExceeded {
                        message: "persisted entry count overflowed usize".to_string(),
                    })
            })?;
        if total_entries > self.config.max_total_entries {
            let mut flattened = Vec::new();
            flattened
                .try_reserve(total_entries)
                .map_err(|error| CacheError::CapacityExceeded {
                    message: format!("could not allocate persisted entries: {error}"),
                })?;
            for (_, key_entries) in loaded_entries.drain() {
                flattened.extend(
                    key_entries
                        .into_iter()
                        .map(|entry| (entry.key.clone(), entry)),
                );
            }
            flattened.sort_unstable_by_key(|(_, entry)| entry.timestamp);
            flattened.drain(..total_entries - self.config.max_total_entries);

            loaded_entries
                .try_reserve(flattened.len())
                .map_err(|error| CacheError::CapacityExceeded {
                    message: format!("could not allocate persisted cache index: {error}"),
                })?;
            for (key, entry) in flattened {
                loaded_entries.entry(key).or_default().push(entry);
            }
        }
        let mut entries = self.entries.write().await;
        *entries = loaded_entries;
        Ok(())
    }

    /// Force the next mutation to retry a full persistence sync.
    async fn mark_sync_pending(&self) {
        if self.config.persistence.enabled {
            *self.operation_count.write().await = self.config.persistence.sync_interval;
        }
    }

    /// Increment operation count and sync if needed
    async fn increment_and_maybe_sync(&self) -> Result<()> {
        if !self.config.persistence.enabled {
            return Ok(());
        }
        let mut count = self.operation_count.write().await;
        *count = count.saturating_add(1);

        if *count >= self.config.persistence.sync_interval {
            *count = 0;
            drop(count);
            if let Err(error) = self.save_to_storage().await {
                self.mark_sync_pending().await;
                return Err(error);
            }
        }

        Ok(())
    }
}

impl_cache_common!(
    Clone,
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            config: self.config.clone(),
            backend: Arc::clone(&self.backend),
            operation_lock: Arc::clone(&self.operation_lock),
            operation_count: Arc::clone(&self.operation_count),
            eviction_strategy: crate::eviction::create_strategy(&self.config.eviction_policy),
        }
    }
);

impl_cache_common!(#[async_trait] AsyncCache<K, V>,
    type Error = CacheError;

    async fn get(&self, key: &K) -> std::result::Result<Option<V>, Self::Error> {
        Ok(self.get_latest(key).await.map(|entry| entry.value))
    }

    async fn put(&self, key: K, value: V) -> std::result::Result<(), Self::Error> {
        let _operation = self.operation_lock.lock().await;
        let entry = self.apply_default_ttl(CacheEntry::new(key.clone(), value))?;
        {
            let mut entries = self.entries.write().await;
            entries.remove(&key);
            self.insert_entry(&mut entries, entry).await?;
        }

        // Increment operation count and check if we need to sync
        self.increment_and_maybe_sync().await?;
        Ok(())
    }

    async fn remove(&self, key: &K) -> std::result::Result<Option<V>, Self::Error> {
        let _operation = self.operation_lock.lock().await;
        let removed = self.entries.write().await.remove(key);

        if removed.is_some() {
            if self.config.persistence.enabled
                && let Err(error) = self.backend.remove(key).await
            {
                self.mark_sync_pending().await;
                return Err(error);
            }
            self.increment_and_maybe_sync().await?;
        }

        Ok(removed.and_then(|entries| {
            entries
                .into_iter()
                .max_by_key(|entry| entry.timestamp)
                .map(|entry| entry.value)
        }))
    }

    async fn clear(&self) -> std::result::Result<(), Self::Error> {
        let _operation = self.operation_lock.lock().await;
        self.entries.write().await.clear();

        if self.config.persistence.enabled
            && let Err(error) = self.backend.clear().await
        {
            self.mark_sync_pending().await;
            return Err(error);
        }

        *self.operation_count.write().await = 0;
        Ok(())
    }

    async fn contains(&self, key: &K) -> std::result::Result<bool, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries
            .get(key)
            .is_some_and(|key_entries| key_entries.iter().any(|entry| !entry.is_expired())))
    }

    async fn len(&self) -> std::result::Result<usize, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries
            .values()
            .flat_map(|key_entries| key_entries.iter())
            .filter(|entry| !entry.is_expired())
            .count())
    }
);

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SearchQuery;
    use crate::backends::memory::MemoryBackend;

    async fn create_cache() -> Cache<String, String> {
        let config = CacheConfig::default();
        let backend = MemoryBackend::new();
        Cache::new(config, backend).await.unwrap()
    }

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = create_cache().await;

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
        let cache = create_cache().await;

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

    #[tokio::test]
    async fn test_entry_limits_and_eviction() {
        let config = CacheConfig {
            max_entries_per_key: 2,
            max_total_entries: 3,
            ..CacheConfig::default()
        };
        let backend: MemoryBackend<String, String> = MemoryBackend::new();
        let cache: Cache<String, String> = Cache::new(config, backend).await.unwrap();

        cache
            .add_entry(CacheEntry::new("k1".to_string(), "v1".to_string()))
            .await
            .unwrap();
        cache
            .add_entry(CacheEntry::new("k1".to_string(), "v2".to_string()))
            .await
            .unwrap();
        cache
            .add_entry(CacheEntry::new("k1".to_string(), "v3".to_string()))
            .await
            .unwrap();

        let k1_entries = cache.get_entries(&"k1".to_string()).await.unwrap();
        assert_eq!(k1_entries.len(), 2);
        assert_eq!(k1_entries[0].value, "v2");
        assert_eq!(k1_entries[1].value, "v3");

        cache
            .add_entry(CacheEntry::new("k2".to_string(), "v".to_string()))
            .await
            .unwrap();
        cache
            .add_entry(CacheEntry::new("k3".to_string(), "v".to_string()))
            .await
            .unwrap();

        assert!(cache.len().await.unwrap() <= 3);
    }

    #[tokio::test]
    async fn test_cache_entries_search_stats() {
        let cache = create_cache().await;

        let mut first = CacheEntry::new("key".to_string(), "v1".to_string());
        first.timestamp = chrono::Utc::now() - chrono::Duration::seconds(1);
        first.last_accessed = first.timestamp;
        cache.add_entry(first).await.unwrap();
        cache
            .add_entry(CacheEntry::new("key".to_string(), "v2".to_string()))
            .await
            .unwrap();

        let entries = cache.get_entries(&"key".to_string()).await.unwrap();
        assert_eq!(entries.len(), 2);
        let latest = cache.get_latest(&"key".to_string()).await.unwrap();
        assert_eq!(latest.value, "v2");

        let results = cache.search(&SearchQuery::new().with_pattern("key")).await;
        assert_eq!(results.len(), 2);

        // Add expired entry for stats
        let expired = CacheEntry::new("expired".to_string(), "v".to_string())
            .with_ttl(chrono::Duration::seconds(-1));
        cache.add_entry(expired).await.unwrap();

        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.expired_count, 1);
        assert_eq!(stats.total_access_count, 3); // accesses from get_entries/get_latest
    }

    #[tokio::test]
    async fn test_empty_cache_stats() {
        let cache = create_cache().await;
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_keys, 0);
        assert_eq!(stats.total_access_count, 0);
        assert_eq!(stats.expired_count, 0);
    }

    #[tokio::test]
    async fn test_cache_persistence() {
        use crate::test_utils::TestBackend;

        let backend = TestBackend::default();
        // Preload backend
        backend
            .save(&HashMap::from([(
                "loaded".to_string(),
                vec![CacheEntry::new("loaded".to_string(), "v".to_string())],
            )]))
            .await
            .unwrap();

        let mut config = CacheConfig::default();
        config.persistence.enabled = true;
        config.persistence.load_on_startup = true;
        config.persistence.sync_interval = 1;

        let cache: Cache<String, String, (), TestBackend> =
            Cache::new(config, backend.clone()).await.unwrap();
        // Loaded entry should be present
        assert!(cache.contains(&"loaded".to_string()).await.unwrap());
        assert_eq!(*backend.load_calls.read().await, 1);

        // Put new entry triggers save due to sync_interval=1
        cache.put("k".to_string(), "v".to_string()).await.unwrap();
        assert!(*backend.save_calls.read().await >= 1);
        assert!(backend.entries.read().await.contains_key("k"));
    }

    #[tokio::test]
    async fn invalid_capacity_and_sync_configuration_is_rejected() {
        for config in [
            CacheConfig {
                max_entries_per_key: 0,
                ..CacheConfig::default()
            },
            CacheConfig {
                max_total_entries: 0,
                ..CacheConfig::default()
            },
            CacheConfig {
                persistence: crate::PersistenceConfig {
                    enabled: true,
                    sync_interval: 0,
                    load_on_startup: false,
                },
                ..CacheConfig::default()
            },
        ] {
            let result: Result<Cache<String, String>> =
                Cache::new(config, MemoryBackend::new()).await;
            assert!(matches!(result, Err(CacheError::InvalidConfiguration(_))));
        }
    }

    #[tokio::test]
    async fn default_ttl_is_applied_and_expired_entries_are_hidden() {
        let config = CacheConfig::default().with_default_ttl(std::time::Duration::ZERO);
        let cache: Cache<String, String> = Cache::new(config, MemoryBackend::new()).await.unwrap();
        let key = "expired".to_string();
        cache.put(key.clone(), "value".to_string()).await.unwrap();

        assert_eq!(cache.get(&key).await.unwrap(), None);
        assert!(!cache.contains(&key).await.unwrap());
        assert_eq!(cache.len().await.unwrap(), 0);
        assert!(cache.get_entries(&key).await.is_none());
    }

    #[tokio::test]
    async fn explicit_entry_ttl_overrides_default_ttl() {
        let config = CacheConfig::default().with_default_ttl(std::time::Duration::ZERO);
        let cache: Cache<String, String> = Cache::new(config, MemoryBackend::new()).await.unwrap();
        cache
            .add_entry(
                CacheEntry::new("key".to_string(), "value".to_string())
                    .with_ttl(chrono::Duration::hours(1)),
            )
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"key".to_string()).await.unwrap(),
            Some("value".to_string())
        );
    }

    #[tokio::test]
    async fn expired_entries_are_reclaimed_before_capacity_checks() {
        let config = CacheConfig {
            max_total_entries: 1,
            eviction_policy: crate::EvictionPolicy::None,
            ..CacheConfig::default()
        };
        let cache: Cache<String, String> = Cache::new(config, MemoryBackend::new()).await.unwrap();
        cache
            .add_entry(
                CacheEntry::new("expired".to_string(), "old".to_string())
                    .with_ttl(chrono::Duration::seconds(-1)),
            )
            .await
            .unwrap();

        assert_eq!(cache.len().await.unwrap(), 0);
        cache
            .put("live".to_string(), "new".to_string())
            .await
            .unwrap();

        assert_eq!(cache.len().await.unwrap(), 1);
        assert_eq!(
            cache.get(&"live".to_string()).await.unwrap(),
            Some("new".to_string())
        );
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.expired_count, 0);
    }

    #[tokio::test]
    async fn add_entry_orders_history_and_retains_newest_timestamps() {
        let config = CacheConfig {
            max_entries_per_key: 2,
            ..CacheConfig::default()
        };
        let cache: Cache<String, String> = Cache::new(config, MemoryBackend::new()).await.unwrap();
        let key = "history".to_string();
        let now = chrono::Utc::now();

        let mut newest = CacheEntry::new(key.clone(), "newest".to_string());
        newest.timestamp = now;
        newest.last_accessed = now;
        let mut oldest = CacheEntry::new(key.clone(), "oldest".to_string());
        oldest.timestamp = now - chrono::Duration::hours(2);
        oldest.last_accessed = oldest.timestamp;
        let mut middle = CacheEntry::new(key.clone(), "middle".to_string());
        middle.timestamp = now - chrono::Duration::hours(1);
        middle.last_accessed = middle.timestamp;

        cache.add_entry(newest).await.unwrap();
        cache.add_entry(oldest).await.unwrap();
        cache.add_entry(middle).await.unwrap();

        let history = cache.get_entries(&key).await.unwrap();
        assert_eq!(
            history
                .iter()
                .map(|entry| entry.value.as_str())
                .collect::<Vec<_>>(),
            vec!["middle", "newest"]
        );
        assert!(
            history
                .windows(2)
                .all(|pair| pair[0].timestamp <= pair[1].timestamp)
        );
        assert_eq!(cache.get_latest(&key).await.unwrap().value, "newest");
    }

    #[tokio::test]
    async fn no_eviction_policy_returns_capacity_error_without_growing() {
        let config = CacheConfig {
            max_total_entries: 1,
            eviction_policy: crate::EvictionPolicy::None,
            ..CacheConfig::default()
        };
        let cache: Cache<String, String> = Cache::new(config, MemoryBackend::new()).await.unwrap();
        cache.put("one".to_string(), "1".to_string()).await.unwrap();
        let result = cache.put("two".to_string(), "2".to_string()).await;
        assert!(matches!(result, Err(CacheError::CapacityExceeded { .. })));
        assert_eq!(cache.len().await.unwrap(), 1);
        assert_eq!(
            cache.get(&"one".to_string()).await.unwrap(),
            Some("1".to_string())
        );
    }

    #[tokio::test]
    async fn startup_rejects_mismatched_embedded_keys() {
        use crate::test_utils::TestBackend;

        let backend = TestBackend::default();
        backend
            .save(&HashMap::from([(
                "outer".to_string(),
                vec![CacheEntry::new("inner".to_string(), "value".to_string())],
            )]))
            .await
            .unwrap();
        let config = CacheConfig {
            persistence: crate::PersistenceConfig::enabled(),
            ..CacheConfig::default()
        };
        let result: Result<Cache<String, String, (), TestBackend>> =
            Cache::new(config, backend).await;
        assert!(matches!(result, Err(CacheError::Deserialization(_))));
    }

    #[tokio::test]
    async fn flush_waits_for_persistence() {
        use crate::test_utils::TestBackend;

        let backend = TestBackend::default();
        let config = CacheConfig {
            persistence: crate::PersistenceConfig {
                enabled: true,
                sync_interval: 100,
                load_on_startup: false,
            },
            ..CacheConfig::default()
        };
        let cache: Cache<String, String, (), TestBackend> =
            Cache::new(config, backend.clone()).await.unwrap();
        cache
            .put("key".to_string(), "value".to_string())
            .await
            .unwrap();
        assert_eq!(*backend.save_calls.read().await, 0);
        cache.flush().await.unwrap();
        assert_eq!(*backend.save_calls.read().await, 1);
        assert!(backend.entries.read().await.contains_key("key"));
    }

    #[tokio::test]
    async fn remove_error_keeps_memory_mutation_and_next_mutation_reconciles() {
        use crate::test_utils::TestBackend;

        let backend = TestBackend::default();
        let removed_key = "removed".to_string();
        backend
            .save(&HashMap::from([(
                removed_key.clone(),
                vec![CacheEntry::new(removed_key.clone(), "value".to_string())],
            )]))
            .await
            .unwrap();
        let config = CacheConfig {
            persistence: crate::PersistenceConfig {
                enabled: true,
                sync_interval: 100,
                load_on_startup: true,
            },
            ..CacheConfig::default()
        };
        let cache: Cache<String, String, (), TestBackend> =
            Cache::new(config, backend.clone()).await.unwrap();
        *backend.remove_error_after_mutation.write().await = true;

        let error = cache.remove(&removed_key).await.unwrap_err();
        assert!(matches!(error, CacheError::StorageBackend(_)));
        assert!(!cache.contains(&removed_key).await.unwrap());
        assert_eq!(
            *cache.operation_count.read().await,
            cache.config.persistence.sync_interval
        );

        *backend.remove_error_after_mutation.write().await = false;
        cache
            .put("new".to_string(), "value".to_string())
            .await
            .unwrap();
        let persisted = backend.entries.read().await;
        assert!(!persisted.contains_key(&removed_key));
        assert!(persisted.contains_key("new"));
        assert_eq!(*cache.operation_count.read().await, 0);
    }

    #[tokio::test]
    async fn clear_error_keeps_memory_mutation_and_flush_reconciles() {
        use crate::test_utils::TestBackend;

        let backend = TestBackend::default();
        backend
            .save(&HashMap::from([(
                "key".to_string(),
                vec![CacheEntry::new("key".to_string(), "value".to_string())],
            )]))
            .await
            .unwrap();
        let config = CacheConfig {
            persistence: crate::PersistenceConfig {
                enabled: true,
                sync_interval: 100,
                load_on_startup: true,
            },
            ..CacheConfig::default()
        };
        let cache: Cache<String, String, (), TestBackend> =
            Cache::new(config, backend.clone()).await.unwrap();
        *backend.clear_error_after_mutation.write().await = true;

        let error = cache.clear().await.unwrap_err();
        assert!(matches!(error, CacheError::StorageBackend(_)));
        assert!(cache.is_empty().await.unwrap());
        assert_eq!(
            *cache.operation_count.read().await,
            cache.config.persistence.sync_interval
        );

        *backend.clear_error_after_mutation.write().await = false;
        cache.flush().await.unwrap();
        assert!(backend.entries.read().await.is_empty());
        assert_eq!(*cache.operation_count.read().await, 0);
    }

    #[tokio::test]
    async fn successful_clear_resets_a_pending_sync() {
        use crate::test_utils::TestBackend;

        let backend = TestBackend::default();
        let first_key = "first".to_string();
        backend
            .save(&HashMap::from([
                (
                    first_key.clone(),
                    vec![CacheEntry::new(first_key.clone(), "1".to_string())],
                ),
                (
                    "second".to_string(),
                    vec![CacheEntry::new("second".to_string(), "2".to_string())],
                ),
            ]))
            .await
            .unwrap();
        let config = CacheConfig {
            persistence: crate::PersistenceConfig {
                enabled: true,
                sync_interval: 2,
                load_on_startup: true,
            },
            ..CacheConfig::default()
        };
        let cache: Cache<String, String, (), TestBackend> =
            Cache::new(config, backend.clone()).await.unwrap();
        *backend.remove_error_after_mutation.write().await = true;
        assert!(cache.remove(&first_key).await.is_err());
        *backend.remove_error_after_mutation.write().await = false;

        cache.clear().await.unwrap();
        assert_eq!(*cache.operation_count.read().await, 0);
        let save_calls = *backend.save_calls.read().await;
        cache
            .put("new".to_string(), "value".to_string())
            .await
            .unwrap();
        assert_eq!(*backend.save_calls.read().await, save_calls);
    }

    #[cfg(feature = "filesystem-backend")]
    #[tokio::test]
    async fn startup_propagates_corrupt_filesystem_snapshot() {
        use crate::FilesystemBackend;

        let directory = tempfile::TempDir::new().unwrap();
        tokio::fs::write(directory.path().join("cache.json"), b"not json")
            .await
            .unwrap();
        let backend: FilesystemBackend<String, String> =
            FilesystemBackend::new(directory.path()).await.unwrap();
        let config = CacheConfig {
            persistence: crate::PersistenceConfig::enabled(),
            ..CacheConfig::default()
        };
        let result: Result<Cache<String, String, (), _>> = Cache::new(config, backend).await;
        assert!(matches!(result, Err(CacheError::Deserialization(_))));
    }
}
