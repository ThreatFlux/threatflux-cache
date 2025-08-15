//! Storage backend trait and utilities

use crate::entry::CacheEntry;
use crate::error::Result;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::hash::Hash;

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
    async fn save(
        &self,
        entries: &HashMap<Self::Key, Vec<CacheEntry<Self::Key, Self::Value, Self::Metadata>>>,
    ) -> Result<()>;

    /// Load entries from storage
    async fn load(
        &self,
    ) -> Result<HashMap<Self::Key, Vec<CacheEntry<Self::Key, Self::Value, Self::Metadata>>>>;

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
    async fn size_bytes(&self) -> Result<u64> {
        Ok(0) // Default implementation returns 0
    }

    /// Compact storage (optional operation for backends that support it)
    async fn compact(&self) -> Result<()> {
        Ok(()) // Default is no-op
    }
}

/// Serialization format for storage backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
    /// JSON format
    #[cfg(feature = "json-serialization")]
    Json,
    /// Bincode format
    #[cfg(feature = "bincode-serialization")]
    Bincode,
}

impl SerializationFormat {
    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            #[cfg(feature = "json-serialization")]
            SerializationFormat::Json => "json",
            #[cfg(feature = "bincode-serialization")]
            SerializationFormat::Bincode => "bin",
            #[cfg(not(any(feature = "json-serialization", feature = "bincode-serialization")))]
            _ => "data",
        }
    }

    /// Serialize data to bytes
    #[allow(unused_variables)]
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        match self {
            #[cfg(feature = "json-serialization")]
            SerializationFormat::Json => serde_json::to_vec_pretty(value).map_err(Into::into),
            #[cfg(feature = "bincode-serialization")]
            SerializationFormat::Bincode => bincode::serialize(value).map_err(Into::into),
            #[cfg(not(any(feature = "json-serialization", feature = "bincode-serialization")))]
            _ => Err(crate::error::CacheError::Serialization(
                "No serialization features enabled".to_string(),
            )),
        }
    }

    /// Deserialize data from bytes
    #[allow(unused_variables)]
    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        match self {
            #[cfg(feature = "json-serialization")]
            SerializationFormat::Json => serde_json::from_slice(data).map_err(Into::into),
            #[cfg(feature = "bincode-serialization")]
            SerializationFormat::Bincode => bincode::deserialize(data).map_err(Into::into),
            #[cfg(not(any(feature = "json-serialization", feature = "bincode-serialization")))]
            _ => Err(crate::error::CacheError::Serialization(
                "No serialization features enabled".to_string(),
            )),
        }
    }
}

/// Storage statistics
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    /// Total number of keys
    pub total_keys: usize,
    /// Total number of entries
    pub total_entries: usize,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Average entries per key
    pub avg_entries_per_key: f64,
}

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "json-serialization", feature = "bincode-serialization"))]
    use super::*;

    #[test]
    fn test_serialization_format_extension() {
        #[cfg(feature = "json-serialization")]
        assert_eq!(SerializationFormat::Json.extension(), "json");

        #[cfg(feature = "bincode-serialization")]
        assert_eq!(SerializationFormat::Bincode.extension(), "bin");
    }

    #[cfg(feature = "json-serialization")]
    #[test]
    fn test_json_serialization() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestData {
            value: String,
        }

        let data = TestData {
            value: "test".to_string(),
        };
        let format = SerializationFormat::Json;

        let serialized = format.serialize(&data).unwrap();
        let deserialized: TestData = format.deserialize(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }
}
