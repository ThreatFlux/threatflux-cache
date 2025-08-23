//! Filesystem storage backend

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::backends::{StorageKey, StorageMeta, StorageValue};
use crate::{
    storage::{EntryMap, SerializationFormat},
    CacheEntry, EntryMetadata, Result, StorageBackend,
};

/// Type alias for complex phantom data type
type PhantomTypes<K, V, M> = std::marker::PhantomData<(K, V, M)>;

/// Filesystem storage backend
#[allow(clippy::type_complexity)]
pub struct FilesystemBackend<K, V, M = ()>
where
    K: StorageKey + std::fmt::Display,
    V: StorageValue,
    M: StorageMeta,
{
    base_path: PathBuf,
    format: SerializationFormat,
    _phantom: PhantomTypes<K, V, M>,
}

impl<K, V, M> FilesystemBackend<K, V, M>
where
    K: StorageKey + std::fmt::Display,
    V: StorageValue,
    M: StorageMeta,
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

    async fn write_data<P: AsRef<Path>>(&self, path: P, data: &[u8]) -> Result<()> {
        let mut file = File::create(path).await?;
        file.write_all(data).await?;
        file.flush().await?;
        Ok(())
    }

    fn is_cache_file_path(&self, path: &Path) -> bool {
        path.extension().and_then(|s| s.to_str()) == Some(self.format.extension())
            && path.file_stem().and_then(|s| s.to_str()) != Some("metadata")
    }

    async fn cache_file_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        let mut dir_entries = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            if self.is_cache_file_path(&path) {
                paths.push(path);
            }
        }
        Ok(paths)
    }

    async fn load_entry_from_path(&self, path: &Path) -> Option<(K, Vec<CacheEntry<K, V, M>>)>
    where
        K: Serialize + DeserializeOwned + std::fmt::Display,
        V: Serialize + DeserializeOwned,
        M: Serialize + DeserializeOwned + EntryMetadata,
    {
        let data = match fs::read(path).await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to read cache file {path:?}: {e}");
                return None;
            }
        };
        let entry_vec: Vec<CacheEntry<K, V, M>> = match self.format.deserialize(&data) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to deserialize cache file {path:?}: {e}");
                return None;
            }
        };
        let key = match entry_vec.first() {
            Some(first) => first.key.clone(),
            None => return None,
        };
        Some((key, entry_vec))
    }
}

#[async_trait]
impl<K, V, M> StorageBackend for FilesystemBackend<K, V, M>
where
    K: StorageKey + std::fmt::Display,
    V: StorageValue,
    M: StorageMeta,
{
    // Type associations for this backend
    type Value = V;
    type Key = K;
    type Metadata = M;

    async fn save(&self, entries: &EntryMap<K, V, M>) -> Result<()> {
        for (key, entry_vec) in entries {
            let file_path = self.get_cache_file_path(&key.to_string());
            let data = self.format.serialize(entry_vec)?;
            self.write_data(file_path, &data).await?;
        }

        let metadata = CacheMetadata {
            total_keys: entries.len(),
            last_updated: chrono::Utc::now(),
        };
        let data = self.format.serialize(&metadata)?;
        self.write_data(self.get_metadata_path(), &data).await
    }

    async fn load(&self) -> Result<EntryMap<K, V, M>> {
        let mut entries: EntryMap<K, V, M> = HashMap::new();
        for path in self.cache_file_paths().await? {
            if let Some((key, entry_vec)) = self.load_entry_from_path(&path).await {
                entries.insert(key, entry_vec);
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
        for path in self.cache_file_paths().await? {
            fs::remove_file(&path).await?;
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

    async fn new_backend() -> (TempDir, FilesystemBackend<String, String>) {
        let temp_dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(temp_dir.path()).await.unwrap();
        (temp_dir, backend)
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
        let (_temp_dir, backend) = new_backend().await;

        // Save some data
        let mut entries = HashMap::new();
        for i in 0..5 {
            let entry = CacheEntry::new(format!("key{i}"), format!("value{i}"));
            entries.insert(format!("key{i}"), vec![entry]);
        }

        backend.save(&entries).await.unwrap();

        // Check size is non-zero
        let size = backend.size_bytes().await.unwrap();
        assert!(size > 0);
    }

    #[tokio::test]
    async fn test_load_skips_corrupted_files() {
        let (_temp_dir, backend) = new_backend().await;

        // Create a corrupt cache file
        let bad_path = backend.get_cache_file_path("bad");
        let mut file = File::create(&bad_path).await.unwrap();
        file.write_all(b"not valid").await.unwrap();
        file.flush().await.unwrap();

        // Save a valid entry
        let mut entries = HashMap::new();
        entries.insert(
            "good".to_string(),
            vec![CacheEntry::new("good".to_string(), "value".to_string())],
        );
        backend.save(&entries).await.unwrap();

        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("good"));

        // Ensure corrupt file was not loaded
        assert!(!loaded.contains_key("bad"));
    }

    #[tokio::test]
    async fn test_path_traversal_protection() {
        let (_temp_dir, backend) = new_backend().await;

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
                "Malicious key '{malicious_key}' resulted in path outside base directory: {path:?}"
            );

            // Ensure the filename doesn't contain path separators
            let filename = path.file_name().unwrap().to_str().unwrap();
            assert!(
                !filename.contains('/') && !filename.contains('\\'),
                "Filename '{filename}' contains path separators for key '{malicious_key}'"
            );
        }
    }

    #[test]
    fn test_filename_sanitization() {
        let cases = [
            ("../etc/passwd", "_._etc_passwd"),
            ("file\\name", "file_name"),
            ("file:name", "file_name"),
            ("file*name", "file_name"),
            ("file?name", "file_name"),
            ("file\"name", "file_name"),
            ("file<name>", "file_name_"),
            ("file|name", "file_name"),
            (".hidden", "_hidden"),
            ("...", "_"),
            ("", ""),
            ("   ", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(
                FilesystemBackend::<String, String>::sanitize_filename(input),
                expected
            );
        }

        let result = FilesystemBackend::<String, String>::sanitize_filename("../etc/passwd");
        assert!(!result.contains('/'));
        assert!(!result.contains('\\'));
        assert!(!result.starts_with('.'));
    }
}
