//! Storage backend trait and utilities

use crate::entry::CacheEntry;
use crate::error::Result;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::hash::Hash;

/// Convenience alias for the internal storage map
pub type EntryMap<K, V, M> = HashMap<K, Vec<CacheEntry<K, V, M>>>;

/// Trait for cache storage backends
#[async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    /// Key type for the storage
    type Key: Serialize + DeserializeOwned + Hash + Eq + Clone + Send + Sync;
    /// Value type for the storage
    type Value: Serialize + DeserializeOwned + Clone + Send + Sync;
    /// Metadata type for entries
    type Metadata: Serialize + DeserializeOwned + Clone + Send + Sync;

    /// Save entries to storage
    async fn save(&self, entries: &EntryMap<Self::Key, Self::Value, Self::Metadata>) -> Result<()>;

    /// Load entries from storage
    async fn load(&self) -> Result<EntryMap<Self::Key, Self::Value, Self::Metadata>>;

    /// Remove entries for a specific key
    async fn remove(&self, key: &Self::Key) -> Result<()>;

    /// Clear all entries from storage
    async fn clear(&self) -> Result<()>;

    /// Check if storage contains a key
    async fn contains(&self, key: &Self::Key) -> Result<bool> {
        let entries = self.load().await?;
        Ok(entries.contains_key(key))
    }

    /// Get approximate size of storage in bytes
    async fn size_bytes(&self) -> Result<u64>;

    /// Compact storage (optional operation for backends that support it)
    async fn compact(&self) -> Result<()> {
        Ok(()) // Default is no-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_default_storage_methods() {
        use crate::test_utils::TestBackend;
        use std::collections::HashMap;

        let backend = TestBackend::default();
        let mut map = HashMap::new();
        map.insert(
            "a".to_string(),
            vec![CacheEntry::new("a".to_string(), "v".to_string())],
        );
        backend.save(&map).await.unwrap();

        assert!(backend.contains(&"a".to_string()).await.unwrap());
        assert!(!backend.contains(&"b".to_string()).await.unwrap());
        assert!(backend.size_bytes().await.unwrap() > 0);
        backend.compact().await.unwrap();
    }
}
