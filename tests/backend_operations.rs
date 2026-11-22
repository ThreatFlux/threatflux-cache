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

#[cfg(feature = "filesystem-backend")]
#[tokio::test]
async fn filesystem_backend_preserves_chrono_snapshot_timestamps() {
    use threatflux_cache::{SignedDuration, Timestamp};

    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("cache.json");
    let snapshot = serde_json::json!({
        "version": 1,
        "entries": [["key", [{
            "key": "key",
            "value": "value",
            "metadata": null,
            "timestamp": "2024-02-29T12:34:56.123456789Z",
            "expiry": "2024-02-29T12:35:56.123456789Z",
            "access_count": 1,
            "last_accessed": "2024-02-29T12:35:00Z"
        }, {
            "key": "key",
            "value": "legacy",
            "metadata": null,
            "timestamp": "2016-12-31T23:59:60.123Z",
            "expiry": null,
            "access_count": 1,
            "last_accessed": "+10000-01-01T00:00:00.123456Z"
        }]]]
    });
    tokio::fs::write(&path, serde_json::to_vec(&snapshot).unwrap())
        .await
        .unwrap();

    let backend = FilesystemBackend::<String, String>::new(temp_dir.path())
        .await
        .unwrap();
    let entries = backend.load().await.unwrap();
    let entry = &entries["key"][0];
    let timestamp: Timestamp = "2024-02-29T12:34:56.123456789Z".parse().unwrap();
    assert_eq!(entry.timestamp, timestamp);
    assert_eq!(
        entry.expiry,
        Some(timestamp.saturating_add(SignedDuration::minutes(1)))
    );

    let legacy_entry = &entries["key"][1];
    assert!(legacy_entry.timestamp.is_leap_second());
    assert_eq!(legacy_entry.timestamp.nanosecond(), 123_000_000);
    assert_eq!(
        legacy_entry
            .timestamp
            .saturating_add(SignedDuration::milliseconds(877)),
        "2017-01-01T00:00:00Z".parse().unwrap()
    );

    backend.save(&entries).await.unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
    assert_eq!(saved, snapshot);
}
