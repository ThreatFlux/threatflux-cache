#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use tokio::sync::RwLock;

#[cfg(test)]
use crate::{CacheEntry, Result, StorageBackend};

/// Test backend storing entries in memory and tracking save/load calls.
#[cfg(test)]
#[derive(Clone, Default)]
#[allow(clippy::type_complexity)]
pub(crate) struct TestBackend {
    pub entries: Arc<RwLock<HashMap<String, Vec<CacheEntry<String, String, ()>>>>>,
    pub save_calls: Arc<RwLock<usize>>,
    pub load_calls: Arc<RwLock<usize>>,
}

#[cfg(test)]
#[async_trait::async_trait]
impl StorageBackend for TestBackend {
    type Key = String;
    type Value = String;
    type Metadata = ();

    async fn save(
        &self,
        entries: &HashMap<Self::Key, Vec<CacheEntry<Self::Key, Self::Value, Self::Metadata>>>,
    ) -> Result<()> {
        *self.save_calls.write().await += 1;
        *self.entries.write().await = entries.clone();
        Ok(())
    }

    async fn load(
        &self,
    ) -> Result<HashMap<Self::Key, Vec<CacheEntry<Self::Key, Self::Value, Self::Metadata>>>> {
        *self.load_calls.write().await += 1;
        Ok(self.entries.read().await.clone())
    }

    async fn remove(&self, key: &Self::Key) -> Result<()> {
        self.entries.write().await.remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.entries.write().await.clear();
        Ok(())
    }
}
