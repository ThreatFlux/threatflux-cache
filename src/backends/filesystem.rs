//! Filesystem storage backend

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::{storage::SerializationFormat, CacheEntry, EntryMetadata, Result, StorageBackend};

/// Filesystem storage backend
pub struct FilesystemBackend<K, V, M = ()>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: Clone + Send + Sync,
{
    base_path: PathBuf,
    format: SerializationFormat,
    _phantom: std::marker::PhantomData<(K, V, M)>,
}

impl<K, V, M> FilesystemBackend<K, V, M>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    M: Clone + Send + Sync,
{
    /// Create a new filesystem backend with the given base path
    pub async fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path).await?;

        Ok(Self {
            base_path,
            #[cfg(feature = "json-serialization")]
            format: SerializationFormat::Json,
            #[cfg(all(not(feature = "json-serialization"), feature = "bincode-serialization"))]
            format: SerializationFormat::Bincode,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Set the serialization format
    pub fn with_format(mut self, format: SerializationFormat) -> Self {
        self.format = format;
        self
    }

    /// Sanitize a filename by removing or replacing dangerous characters
    fn sanitize_filename(filename: &str) -> String {
        // Replace path separators and other dangerous characters with safe alternatives
        let mut result = filename
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if c.is_control() => '_', // Replace control characters
                c => c,
            })
            .collect::<String>();

        // Replace leading dots to prevent hidden files
        if result.starts_with('.') {
            result = result.replacen('.', "_", 1);
        }

        // Clean up trailing dots and whitespace
        result.trim_matches('.').trim().to_string()
    }

    /// Get the path for a cache file
    fn get_cache_file_path(&self, key: &str) -> PathBuf {
        let sanitized_key = Self::sanitize_filename(key);
        // Ensure the filename isn't empty after sanitization
        let safe_key = if sanitized_key.is_empty() {
            "cache_entry".to_string()
        } else {
            sanitized_key
        };

        self.base_path
            .join(format!("{}.{}", safe_key, self.format.extension()))
    }

    /// Get the metadata file path
    fn get_metadata_path(&self) -> PathBuf {
        self.base_path
            .join(format!("metadata.{}", self.format.extension()))
    }
}

#[async_trait]
impl<K, V, M> StorageBackend for FilesystemBackend<K, V, M>
where
    K: Serialize + DeserializeOwned + Hash + Eq + Clone + Send + Sync + std::fmt::Display + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    M: Serialize + DeserializeOwned + Clone + Send + Sync + EntryMetadata,
{
    type Key = K;
    type Value = V;
    type Metadata = M;

    async fn save(&self, entries: &HashMap<K, Vec<CacheEntry<K, V, M>>>) -> Result<()> {
        // Save each key's entries to a separate file
        for (key, entry_vec) in entries {
            let file_path = self.get_cache_file_path(&key.to_string());
            let data = self.format.serialize(entry_vec)?;

            let mut file = File::create(&file_path).await?;
            file.write_all(&data).await?;
            file.flush().await?;
        }

        // Save metadata about the cache
        let metadata = CacheMetadata {
            total_keys: entries.len(),
            last_updated: chrono::Utc::now(),
        };

        let metadata_path = self.get_metadata_path();
        let data = self.format.serialize(&metadata)?;

        let mut file = File::create(&metadata_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        Ok(())
    }

    async fn load(&self) -> Result<HashMap<K, Vec<CacheEntry<K, V, M>>>> {
        let mut entries = HashMap::new();

        // Read all cache files
        let mut dir_entries = fs::read_dir(&self.base_path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();

            // Skip non-cache files
            if path.extension().and_then(|s| s.to_str()) != Some(self.format.extension()) {
                continue;
            }

            // Skip metadata file
            if path.file_stem().and_then(|s| s.to_str()) == Some("metadata") {
                continue;
            }

            // Read and deserialize the file
            match fs::read(&path).await {
                Ok(data) => {
                    match self.format.deserialize::<Vec<CacheEntry<K, V, M>>>(&data) {
                        Ok(entry_vec) => {
                            if let Some(first_entry) = entry_vec.first() {
                                entries.insert(first_entry.key.clone(), entry_vec);
                            }
                        }
                        Err(e) => {
                            // Log error but continue loading other files
                            eprintln!("Failed to deserialize cache file {:?}: {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    // Log error but continue loading other files
                    eprintln!("Failed to read cache file {:?}: {}", path, e);
                }
            }
        }

        Ok(entries)
    }

    async fn remove(&self, key: &K) -> Result<()> {
        let file_path = self.get_cache_file_path(&key.to_string());
        if file_path.exists() {
            fs::remove_file(&file_path).await?;
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut dir_entries = fs::read_dir(&self.base_path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();

            // Only remove cache files
            if path.extension().and_then(|s| s.to_str()) == Some(self.format.extension()) {
                fs::remove_file(&path).await?;
            }
        }

        Ok(())
    }

    async fn contains(&self, key: &K) -> Result<bool> {
        let file_path = self.get_cache_file_path(&key.to_string());
        Ok(file_path.exists())
    }

    async fn size_bytes(&self) -> Result<u64> {
        let mut total_size = 0u64;
        let mut dir_entries = fs::read_dir(&self.base_path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                total_size += metadata.len();
            }
        }

        Ok(total_size)
    }

    async fn compact(&self) -> Result<()> {
        // For filesystem backend, compaction could involve:
        // - Removing expired entries
        // - Consolidating small files
        // - Rewriting files with compression
        // For now, just a no-op
        Ok(())
    }
}

/// Metadata about the cache stored on filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheMetadata {
    total_keys: usize,
    last_updated: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_filesystem_backend_operations() {
        let temp_dir = TempDir::new().unwrap();
        let backend: FilesystemBackend<String, String> =
            FilesystemBackend::new(temp_dir.path()).await.unwrap();

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
    async fn test_filesystem_backend_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Save data with one backend instance
        {
            let backend: FilesystemBackend<String, String> =
                FilesystemBackend::new(&path).await.unwrap();

            let mut entries = HashMap::new();
            let entry =
                CacheEntry::new("persistent_key".to_string(), "persistent_value".to_string());
            entries.insert("persistent_key".to_string(), vec![entry]);

            backend.save(&entries).await.unwrap();
        }

        // Load data with a new backend instance
        {
            let backend: FilesystemBackend<String, String> =
                FilesystemBackend::new(&path).await.unwrap();

            let loaded = backend.load().await.unwrap();
            assert_eq!(loaded.len(), 1);
            assert!(loaded.contains_key("persistent_key"));

            let entries = &loaded["persistent_key"];
            assert_eq!(entries[0].value, "persistent_value");
        }
    }

    #[tokio::test]
    async fn test_filesystem_backend_size() {
        let temp_dir = TempDir::new().unwrap();
        let backend: FilesystemBackend<String, String> =
            FilesystemBackend::new(temp_dir.path()).await.unwrap();

        // Save some data
        let mut entries = HashMap::new();
        for i in 0..5 {
            let entry = CacheEntry::new(format!("key{}", i), format!("value{}", i));
            entries.insert(format!("key{}", i), vec![entry]);
        }

        backend.save(&entries).await.unwrap();

        // Check size is non-zero
        let size = backend.size_bytes().await.unwrap();
        assert!(size > 0);
    }

    #[tokio::test]
    async fn test_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let backend: FilesystemBackend<String, String> =
            FilesystemBackend::new(temp_dir.path()).await.unwrap();

        // Test malicious keys that could attempt path traversal
        let malicious_keys = vec![
            "../etc/passwd",
            "..\\windows\\system32\\config\\sam",
            "/etc/shadow",
            "C:\\Windows\\System32\\config\\SAM",
            "../../sensitive_file",
            "./../../../etc/hosts",
            "../",
            "..",
            "test/../../../etc/passwd",
            "normal_file/../../../etc/passwd",
        ];

        for malicious_key in malicious_keys {
            let path = backend.get_cache_file_path(malicious_key);

            // Ensure the path is within the base directory
            assert!(
                path.starts_with(&backend.base_path),
                "Malicious key '{}' resulted in path outside base directory: {:?}",
                malicious_key,
                path
            );

            // Ensure the filename doesn't contain path separators
            let filename = path.file_name().unwrap().to_str().unwrap();
            assert!(
                !filename.contains('/') && !filename.contains('\\'),
                "Filename '{}' contains path separators for key '{}'",
                filename,
                malicious_key
            );
        }
    }

    #[test]
    fn test_filename_sanitization() {
        // Test various dangerous characters
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("../etc/passwd"),
            "_._etc_passwd"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file\\name"),
            "file_name"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file:name"),
            "file_name"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file*name"),
            "file_name"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file?name"),
            "file_name"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file\"name"),
            "file_name"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file<name>"),
            "file_name_"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("file|name"),
            "file_name"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename(".hidden"),
            "_hidden"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("..."),
            "_"
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename(""),
            ""
        );
        assert_eq!(
            FilesystemBackend::<String, String>::sanitize_filename("   "),
            ""
        );

        // Test the most important security aspect: no path traversal
        let result = FilesystemBackend::<String, String>::sanitize_filename("../etc/passwd");
        assert!(!result.contains('/'));
        assert!(!result.contains('\\'));
        assert!(!result.starts_with('.'));
    }
}
