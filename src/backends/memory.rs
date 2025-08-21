//! In-memory storage backend

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backends::{BackendKey, BackendMeta, BackendValue};
use crate::{CacheEntry, EntryMetadata, Result, StorageBackend};

/// In-memory storage backend
#[allow(clippy::type_complexity)]
pub struct MemoryBackend<K: BackendKey, V: BackendValue, M: BackendMeta = ()> {
    data: Arc<RwLock<HashMap<K, Vec<CacheEntry<K, V, M>>>>>,
}

impl<K: BackendKey, V: BackendValue, M: BackendMeta> MemoryBackend<K, V, M> {
    /// Create a new memory backend
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<K: BackendKey, V: BackendValue, M: BackendMeta> Default for MemoryBackend<K, V, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: BackendKey, V: BackendValue, M: BackendMeta> Clone for MemoryBackend<K, V, M> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

#[async_trait]
impl<K, V, M> StorageBackend for MemoryBackend<K, V, M>
where
    K: BackendKey + Serialize + DeserializeOwned + 'static,
    V: BackendValue + Serialize + DeserializeOwned + 'static,
    M: BackendMeta + Serialize + DeserializeOwned + EntryMetadata,
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
