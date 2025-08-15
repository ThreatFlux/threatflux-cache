//! In-memory storage backend

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{CacheEntry, EntryMetadata, Result, StorageBackend};

/// In-memory storage backend
#[allow(clippy::type_complexity)]
pub struct MemoryBackend<K, V, M = ()>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: Clone + Send + Sync,
{
    data: Arc<RwLock<HashMap<K, Vec<CacheEntry<K, V, M>>>>>,
}

impl<K, V, M> MemoryBackend<K, V, M>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: Clone + Send + Sync,
{
    /// Create a new memory backend
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<K, V, M> Default for MemoryBackend<K, V, M>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: Clone + Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, M> Clone for MemoryBackend<K, V, M>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: Clone + Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

#[async_trait]
impl<K, V, M> StorageBackend for MemoryBackend<K, V, M>
where
    K: Serialize + DeserializeOwned + Hash + Eq + Clone + Send + Sync + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    M: Serialize + DeserializeOwned + Clone + Send + Sync + EntryMetadata,
{
    type Key = K;
    type Value = V;
    type Metadata = M;

    async fn save(&self, entries: &HashMap<K, Vec<CacheEntry<K, V, M>>>) -> Result<()> {
        let mut data = self.data.write().await;
        *data = entries.clone();
        Ok(())
    }

    async fn load(&self) -> Result<HashMap<K, Vec<CacheEntry<K, V, M>>>> {
        let data = self.data.read().await;
        Ok(data.clone())
    }

    async fn remove(&self, key: &K) -> Result<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }

    async fn contains(&self, key: &K) -> Result<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }

    async fn size_bytes(&self) -> Result<u64> {
        let data = self.data.read().await;

        // Estimate size based on number of entries
        let total_entries: usize = data.values().map(|v| v.len()).sum();
        let estimated_size = total_entries * std::mem::size_of::<CacheEntry<K, V, M>>();

        Ok(estimated_size as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend_operations() {
        let backend: MemoryBackend<String, String> = MemoryBackend::new();

        // Test empty state
        let loaded = backend.load().await.unwrap();
        assert!(loaded.is_empty());

        // Test save and load
        let mut entries = HashMap::new();
        let entry = CacheEntry::new("key1".to_string(), "value1".to_string());
        entries.insert("key1".to_string(), vec![entry]);

        backend.save(&entries).await.unwrap();
        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("key1"));

        // Test contains
        assert!(backend.contains(&"key1".to_string()).await.unwrap());
        assert!(!backend.contains(&"key2".to_string()).await.unwrap());

        // Test remove
        backend.remove(&"key1".to_string()).await.unwrap();
        assert!(!backend.contains(&"key1".to_string()).await.unwrap());

        // Test clear
        backend.save(&entries).await.unwrap();
        backend.clear().await.unwrap();
        let loaded = backend.load().await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_memory_backend_clone() {
        let backend1: MemoryBackend<String, String> = MemoryBackend::new();
        let backend2 = backend1.clone();

        // Changes in one should be reflected in the other
        let mut entries = HashMap::new();
        let entry = CacheEntry::new("key1".to_string(), "value1".to_string());
        entries.insert("key1".to_string(), vec![entry]);

        backend1.save(&entries).await.unwrap();
        assert!(backend2.contains(&"key1".to_string()).await.unwrap());
    }
}
