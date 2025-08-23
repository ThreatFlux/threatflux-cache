//! Cache entry types and metadata traits

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;

/// A cache entry containing a key-value pair with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<K, V, M = ()>
where
    K: Clone + Hash + Eq,
    V: Clone,
    M: Clone,
{
    /// The cache key
    pub key: K,
    /// The cached value
    pub value: V,
    /// Optional metadata associated with the entry
    pub metadata: M,
    /// Timestamp when the entry was created
    pub timestamp: DateTime<Utc>,
    /// Optional expiry time for TTL-based eviction
    pub expiry: Option<DateTime<Utc>>,
    /// Number of times this entry has been accessed
    pub access_count: u64,
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
}

impl<K, V, M> CacheEntry<K, V, M>
where
    K: Clone + Hash + Eq,
    V: Clone,
    M: Clone + Default,
{
    /// Create a new cache entry with default metadata
    pub fn new(key: K, value: V) -> Self {
        Self::init(key, value, M::default())
    }
}

impl<K, V, M> CacheEntry<K, V, M>
where
    K: Clone + Hash + Eq,
    V: Clone,
    M: Clone,
{
    /// Internal constructor used by `new` and `with_metadata`
    fn init(key: K, value: V, metadata: M) -> Self {
        let now = Utc::now();
        Self {
            key,
            value,
            metadata,
            timestamp: now,
            expiry: None,
            access_count: 0,
            last_accessed: now,
        }
    }

    /// Create a new cache entry with metadata
    pub fn with_metadata(key: K, value: V, metadata: M) -> Self {
        Self::init(key, value, metadata)
    }

    /// Set expiry time for the entry
    pub fn with_ttl(mut self, ttl: chrono::Duration) -> Self {
        self.expiry = Some(self.timestamp + ttl);
        self
    }

    /// Check if the entry has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expiry {
            Utc::now() > expiry
        } else {
            false
        }
    }

    /// Update access statistics
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
    }

    /// Get the age of the entry
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.timestamp
    }
}

/// Trait for cache entry metadata
pub trait EntryMetadata:
    Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static
{
    /// Get execution time in milliseconds if applicable
    fn execution_time_ms(&self) -> Option<u64> {
        None
    }

    /// Get the size of the cached data if applicable
    fn size_bytes(&self) -> Option<u64> {
        None
    }

    /// Get a category or type identifier
    fn category(&self) -> Option<&str> {
        None
    }
}

/// Empty metadata implementation
impl EntryMetadata for () {}

/// Simple metadata implementation with common fields
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BasicMetadata {
    /// Execution time in milliseconds
    pub execution_time_ms: Option<u64>,
    /// Size in bytes
    pub size_bytes: Option<u64>,
    /// Category or type
    pub category: Option<String>,
    /// Additional tags
    pub tags: Vec<String>,
}

impl EntryMetadata for BasicMetadata {
    fn execution_time_ms(&self) -> Option<u64> {
        self.execution_time_ms
    }

    fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }
}

/// Statistics for a group of cache entries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntryStatistics {
    /// Total number of entries
    pub total_count: usize,
    /// Total size in bytes
    pub total_size_bytes: u64,
    /// Average execution time
    pub avg_execution_time_ms: f64,
    /// Average age of entries
    pub avg_age_seconds: f64,
    /// Number of expired entries
    pub expired_count: usize,
    /// Total access count
    pub total_access_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry() -> CacheEntry<String, String, ()> {
        CacheEntry::new("key1".to_string(), "value1".to_string())
    }

    #[test]
    fn test_cache_entry_creation() {
        let entry = sample_entry();
        assert_eq!(entry.key, "key1");
        assert_eq!(entry.value, "value1");
        assert_eq!(entry.access_count, 0);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_ttl() {
        let entry = sample_entry().with_ttl(chrono::Duration::seconds(60));

        assert!(entry.expiry.is_some());
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_metadata() {
        let metadata = BasicMetadata {
            execution_time_ms: Some(100),
            size_bytes: Some(1024),
            category: Some("test".to_string()),
            tags: vec!["tag1".to_string()],
        };

        let entry = CacheEntry::with_metadata("key1".to_string(), "value1".to_string(), metadata);
        assert_eq!(entry.metadata.execution_time_ms(), Some(100));
        assert_eq!(entry.metadata.size_bytes(), Some(1024));
        assert_eq!(entry.metadata.category(), Some("test"));
    }

    #[test]
    fn test_entry_access_tracking() {
        let mut entry = sample_entry();
        let initial_time = entry.last_accessed;

        // Sleep a tiny bit to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        entry.record_access();
        assert_eq!(entry.access_count, 1);
        assert!(entry.last_accessed > initial_time);

        entry.record_access();
        assert_eq!(entry.access_count, 2);
    }

    #[test]
    fn test_entry_age() {
        let entry = sample_entry();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(entry.age() > chrono::Duration::zero());
    }
}
