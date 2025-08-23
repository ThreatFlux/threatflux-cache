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

fn remove_key_by<K, V, M, F, T>(entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>, metric: F)
where
    K: Hash + Eq + Clone,
    V: Clone,
    M: EntryMetadata,
    F: Fn(&[CacheEntry<K, V, M>]) -> T,
    T: Ord,
{
    if let Some(key) = entries
        .iter()
        .min_by_key(|(_, v)| metric(v))
        .map(|(k, _)| k.clone())
    {
        entries.remove(&key);
    }
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

macro_rules! impl_eviction_strategy {
    ($name:ident, $ctx:ident, $entries:ident, $body:block) => {
        #[async_trait]
        impl<K, V, M> EvictionStrategy<K, V, M> for $name
        where
            K: Hash + Eq + Clone + Send + Sync,
            V: Clone + Send + Sync,
            M: EntryMetadata,
        {
            async fn evict(
                &self,
                $entries: &mut HashMap<K, Vec<CacheEntry<K, V, M>>>,
                $ctx: &EvictionContext,
            ) $body
        }
    };
}

macro_rules! simple_eviction {
    ($(#[$meta:meta])* $name:ident, $metric:expr) => {
        $(#[$meta])*
        pub struct $name;

        impl_eviction_strategy!($name, _context, entries, {
            remove_key_by(entries, $metric);
        });
    };
}

simple_eviction!(
    /// Least Recently Used eviction
    LruEviction,
    |v: &[CacheEntry<K, V, M>]| {
        v.iter()
            .min_by_key(|e| e.last_accessed)
            .map(|e| e.last_accessed)
            .unwrap_or_else(Utc::now)
    }
);

simple_eviction!(
    /// Least Frequently Used eviction
    LfuEviction,
    |v: &[CacheEntry<K, V, M>]| v.iter().map(|e| e.access_count).sum::<u64>()
);

simple_eviction!(
    /// First In First Out eviction
    FifoEviction,
    |v: &[CacheEntry<K, V, M>]| {
        v.iter()
            .min_by_key(|e| e.timestamp)
            .map(|e| e.timestamp)
            .unwrap_or_else(Utc::now)
    }
);

/// Time To Live based eviction
pub struct TtlEviction;

impl_eviction_strategy!(TtlEviction, context, entries, {
    for key in entries.keys().cloned().collect::<Vec<_>>() {
        if let Some(vec) = entries.get_mut(&key) {
            vec.retain(|e| !e.is_expired());
            if vec.is_empty() {
                entries.remove(&key);
            }
        }
    }
    let total_entries: usize = entries.values().map(|v| v.len()).sum();
    if total_entries > context.max_total_entries {
        FifoEviction.evict(entries, context).await;
    }
});

/// No eviction (manual only)
pub struct NoEviction;

impl_eviction_strategy!(NoEviction, _context, _entries, {
    // No automatic eviction
});

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn small_context() -> EvictionContext {
        EvictionContext {
            max_total_entries: 1,
            current_total_entries: 2,
        }
    }

    #[allow(clippy::type_complexity)]
    fn create_test_entry<K: Clone + std::hash::Hash + Eq, V: Clone>(
        key: K,
        value: V,
    ) -> CacheEntry<K, V, ()> {
        CacheEntry::new(key, value)
    }

    fn setup_entries<F>(modifier: F) -> HashMap<String, Vec<CacheEntry<String, String, ()>>>
    where
        F: FnOnce(&mut CacheEntry<String, String, ()>, &mut CacheEntry<String, String, ()>),
    {
        let mut entry1 = create_test_entry("key1".to_string(), "value1".to_string());
        let mut entry2 = create_test_entry("key2".to_string(), "value2".to_string());
        modifier(&mut entry1, &mut entry2);

        let mut entries = HashMap::new();
        entries.insert("key1".to_string(), vec![entry1]);
        entries.insert("key2".to_string(), vec![entry2]);
        entries
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let mut entries = setup_entries(|e1, e2| {
            e1.last_accessed = Utc::now() - Duration::hours(1);
            e2.last_accessed = Utc::now();
        });

        let eviction = LruEviction;
        let context = small_context();

        eviction.evict(&mut entries, &context).await;

        // Should have removed key1 (least recently used)
        assert!(!entries.contains_key("key1"));
        assert!(entries.contains_key("key2"));
    }

    #[tokio::test]
    async fn test_lfu_eviction() {
        let mut entries = setup_entries(|e1, e2| {
            e1.access_count = 1;
            e2.access_count = 5;
        });

        let eviction = LfuEviction;
        let context = small_context();

        eviction.evict(&mut entries, &context).await;

        // Should have removed key1 (least frequently used)
        assert!(!entries.contains_key("key1"));
        assert!(entries.contains_key("key2"));
    }

    #[tokio::test]
    async fn test_fifo_eviction() {
        let mut entries = setup_entries(|e1, e2| {
            e1.timestamp = Utc::now() - Duration::hours(1);
            e2.timestamp = Utc::now();
        });

        let eviction = FifoEviction;
        let context = small_context();

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
