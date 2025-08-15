//! Eviction strategies for cache entries

use crate::config::EvictionPolicy;
use crate::{CacheEntry, EntryMetadata};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::hash::Hash;

/// Type alias for eviction strategy box
type EvictionStrategyBox<K, V, M> = Box<dyn EvictionStrategy<K, V, M>>;

/// Context for eviction decisions
#[derive(Debug, Clone)]
pub struct EvictionContext {
    /// Maximum total entries allowed
    pub max_total_entries: usize,
    /// Current total entries
    pub current_total_entries: usize,
}

/// Trait for eviction strategies
#[async_trait]
pub trait EvictionStrategy<K, V, M>: Send + Sync
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: EntryMetadata,
{
    /// Evict entries based on the strategy
    async fn evict(
        &self,
        entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        _context: &EvictionContext,
    );
}

/// Create an eviction strategy based on policy
#[allow(clippy::type_complexity)]
pub fn create_strategy<K, V, M>(policy: &EvictionPolicy) -> EvictionStrategyBox<K, V, M>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    M: EntryMetadata + 'static,
{
    match policy {
        EvictionPolicy::Lru => Box::new(LruEviction),
        EvictionPolicy::Lfu => Box::new(LfuEviction),
        EvictionPolicy::Fifo => Box::new(FifoEviction),
        EvictionPolicy::Ttl => Box::new(TtlEviction),
        EvictionPolicy::None => Box::new(NoEviction),
    }
}

/// Least Recently Used eviction
pub struct LruEviction;

#[async_trait]
impl<K, V, M> EvictionStrategy<K, V, M> for LruEviction
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: EntryMetadata,
{
    async fn evict(
        &self,
        entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        _context: &EvictionContext,
    ) {
        // Find the least recently accessed entry
        let mut oldest_key: Option<K> = None;
        let mut oldest_access = Utc::now();

        for (key, entry_vec) in entries.iter() {
            if let Some(entry) = entry_vec.iter().min_by_key(|e| e.last_accessed) {
                if entry.last_accessed < oldest_access {
                    oldest_access = entry.last_accessed;
                    oldest_key = Some(key.clone());
                }
            }
        }

        // Remove the oldest key's entries
        if let Some(key) = oldest_key {
            entries.remove(&key);
        }
    }
}

/// Least Frequently Used eviction
pub struct LfuEviction;

#[async_trait]
impl<K, V, M> EvictionStrategy<K, V, M> for LfuEviction
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: EntryMetadata,
{
    async fn evict(
        &self,
        entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        _context: &EvictionContext,
    ) {
        // Find the least frequently accessed entry
        let mut least_used_key: Option<K> = None;
        let mut min_access_count = u64::MAX;

        for (key, entry_vec) in entries.iter() {
            let total_access: u64 = entry_vec.iter().map(|e| e.access_count).sum();
            if total_access < min_access_count {
                min_access_count = total_access;
                least_used_key = Some(key.clone());
            }
        }

        // Remove the least used key's entries
        if let Some(key) = least_used_key {
            entries.remove(&key);
        }
    }
}

/// First In First Out eviction
pub struct FifoEviction;

#[async_trait]
impl<K, V, M> EvictionStrategy<K, V, M> for FifoEviction
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: EntryMetadata,
{
    async fn evict(
        &self,
        entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        _context: &EvictionContext,
    ) {
        // Find the oldest entry by creation timestamp
        let mut oldest_key: Option<K> = None;
        let mut oldest_timestamp = Utc::now();

        for (key, entry_vec) in entries.iter() {
            if let Some(entry) = entry_vec.iter().min_by_key(|e| e.timestamp) {
                if entry.timestamp < oldest_timestamp {
                    oldest_timestamp = entry.timestamp;
                    oldest_key = Some(key.clone());
                }
            }
        }

        // Remove the oldest key's entries
        if let Some(key) = oldest_key {
            entries.remove(&key);
        }
    }
}

/// Time To Live based eviction
pub struct TtlEviction;

#[async_trait]
impl<K, V, M> EvictionStrategy<K, V, M> for TtlEviction
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: EntryMetadata,
{
    async fn evict(
        &self,
        entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        _context: &EvictionContext,
    ) {
        // Remove all expired entries first
        let keys_to_check: Vec<K> = entries.keys().cloned().collect();

        for key in keys_to_check {
            if let Some(entry_vec) = entries.get_mut(&key) {
                // Remove expired entries from the vector
                entry_vec.retain(|entry| !entry.is_expired());

                // If all entries for this key are expired, remove the key
                if entry_vec.is_empty() {
                    entries.remove(&key);
                }
            }
        }

        // If still over capacity, fall back to FIFO
        let total_entries: usize = entries.values().map(|v| v.len()).sum();
        if total_entries > _context.max_total_entries {
            FifoEviction.evict(entries, _context).await;
        }
    }
}

/// No eviction (manual only)
pub struct NoEviction;

#[async_trait]
impl<K, V, M> EvictionStrategy<K, V, M> for NoEviction
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: EntryMetadata,
{
    async fn evict(
        &self,
        _entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
        _context: &EvictionContext,
    ) {
        // No automatic eviction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[allow(clippy::type_complexity)]
    fn create_test_entry<K: Clone + std::hash::Hash + Eq, V: Clone>(
        key: K,
        value: V,
    ) -> CacheEntry<K, V, ()> {
        CacheEntry::new(key, value)
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let mut entries = HashMap::new();
        let mut entry1 = create_test_entry("key1".to_string(), "value1".to_string());
        let mut entry2 = create_test_entry("key2".to_string(), "value2".to_string());

        // Make entry1 older in terms of access
        entry1.last_accessed = Utc::now() - Duration::hours(1);
        entry2.last_accessed = Utc::now();

        entries.insert("key1".to_string(), vec![entry1]);
        entries.insert("key2".to_string(), vec![entry2]);

        let eviction = LruEviction;
        let context = EvictionContext {
            max_total_entries: 1,
            current_total_entries: 2,
        };

        eviction.evict(&mut entries, &context).await;

        // Should have removed key1 (least recently used)
        assert!(!entries.contains_key("key1"));
        assert!(entries.contains_key("key2"));
    }

    #[tokio::test]
    async fn test_lfu_eviction() {
        let mut entries = HashMap::new();
        let mut entry1 = create_test_entry("key1".to_string(), "value1".to_string());
        let mut entry2 = create_test_entry("key2".to_string(), "value2".to_string());

        // Make entry2 more frequently used
        entry1.access_count = 1;
        entry2.access_count = 5;

        entries.insert("key1".to_string(), vec![entry1]);
        entries.insert("key2".to_string(), vec![entry2]);

        let eviction = LfuEviction;
        let context = EvictionContext {
            max_total_entries: 1,
            current_total_entries: 2,
        };

        eviction.evict(&mut entries, &context).await;

        // Should have removed key1 (least frequently used)
        assert!(!entries.contains_key("key1"));
        assert!(entries.contains_key("key2"));
    }

    #[tokio::test]
    async fn test_fifo_eviction() {
        let mut entries = HashMap::new();
        let mut entry1 = create_test_entry("key1".to_string(), "value1".to_string());
        let mut entry2 = create_test_entry("key2".to_string(), "value2".to_string());

        // Make entry1 older
        entry1.timestamp = Utc::now() - Duration::hours(1);
        entry2.timestamp = Utc::now();

        entries.insert("key1".to_string(), vec![entry1]);
        entries.insert("key2".to_string(), vec![entry2]);

        let eviction = FifoEviction;
        let context = EvictionContext {
            max_total_entries: 1,
            current_total_entries: 2,
        };

        eviction.evict(&mut entries, &context).await;

        // Should have removed key1 (first in)
        assert!(!entries.contains_key("key1"));
        assert!(entries.contains_key("key2"));
    }

    #[tokio::test]
    async fn test_ttl_eviction() {
        let mut entries = HashMap::new();
        let entry1 = create_test_entry("key1".to_string(), "value1".to_string())
            .with_ttl(Duration::hours(-1)); // Already expired
        let entry2 = create_test_entry("key2".to_string(), "value2".to_string())
            .with_ttl(Duration::hours(1)); // Not expired

        entries.insert("key1".to_string(), vec![entry1]);
        entries.insert("key2".to_string(), vec![entry2]);

        let eviction = TtlEviction;
        let context = EvictionContext {
            max_total_entries: 10,
            current_total_entries: 2,
        };

        eviction.evict(&mut entries, &context).await;

        // Should have removed key1 (expired)
        assert!(!entries.contains_key("key1"));
        assert!(entries.contains_key("key2"));
    }
}
