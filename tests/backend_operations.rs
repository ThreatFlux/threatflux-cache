use std::collections::HashMap;

use threatflux_cache::backends::memory::MemoryBackend;
use threatflux_cache::{CacheEntry, StorageBackend};

#[cfg(feature = "filesystem-backend")]
use tempfile::TempDir;
#[cfg(feature = "filesystem-backend")]
use threatflux_cache::backends::filesystem::FilesystemBackend;

async fn run_basic_backend_tests<B>(backend: B)
where
    B: StorageBackend<Key = String, Value = String, Metadata = ()>,
{
    // Test empty state
    let loaded = backend.load().await.unwrap();
    assert!(loaded.is_empty());

    // Test save and load
    let mut entries = HashMap::new();
    let entry = CacheEntry::new("key1".to_string(), "value1".to_string());
    entries.insert("key1".to_string(), vec![entry]);

    backend.save(&entries).await.unwrap();
    let size = backend.size_bytes().await.unwrap();
    assert!(size > 0);

    let loaded = backend.load().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(loaded.contains_key("key1"));

    // Test contains
    assert!(backend.contains(&"key1".to_string()).await.unwrap());
    assert!(!backend.contains(&"key2".to_string()).await.unwrap());

    // Test remove
    backend.remove(&"key1".to_string()).await.unwrap();
    assert!(!backend.contains(&"key1".to_string()).await.unwrap());

    // Test clear and compaction
    backend.save(&entries).await.unwrap();
    backend.compact().await.unwrap();
    backend.clear().await.unwrap();
    let loaded = backend.load().await.unwrap();
    assert!(loaded.is_empty());
    let size_after_clear = backend.size_bytes().await.unwrap();
    assert!(size_after_clear <= size);
}

#[tokio::test]
async fn memory_backend_operations() {
    let backend: MemoryBackend<String, String> = MemoryBackend::new();
    run_basic_backend_tests(backend).await;
}

#[cfg(feature = "filesystem-backend")]
#[tokio::test]
async fn filesystem_backend_operations() {
    let temp_dir = TempDir::new().unwrap();
    let backend: FilesystemBackend<String, String> =
        FilesystemBackend::new(temp_dir.path()).await.unwrap();
    run_basic_backend_tests(backend).await;
}
