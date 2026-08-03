//! Filesystem storage backend.
//!
//! The backend stores one versioned snapshot per cache directory. Writes use a
//! temporary file followed by a rename so readers never observe a partially
//! written JSON document. A backend instance serializes its own I/O; callers
//! must not use multiple instances as concurrent writers for the same path.

use async_trait::async_trait;
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::backends::{StorageKey, StorageMeta, StorageValue};
use crate::storage::EntryMap;
use crate::{CacheEntry, CacheError, Result, StorageBackend};

const SNAPSHOT_FILE_NAME: &str = "cache.json";
const TEMP_FILE_PREFIX: &str = ".cache.json.tmp-";
const SNAPSHOT_FORMAT_VERSION: u32 = 1;
const MAX_SNAPSHOT_KEYS: usize = 100_000;
const MAX_SNAPSHOT_ENTRIES: usize = 1_000_000;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Default upper bound for an on-disk cache snapshot (64 MiB).
pub const DEFAULT_MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

/// Type alias for complex phantom data type.
type PhantomTypes<K, V, M> = std::marker::PhantomData<(K, V, M)>;
type SnapshotEntriesRef<'a, K, V, M> = Vec<(&'a K, &'a Vec<CacheEntry<K, V, M>>)>;

struct BoundedBuffer {
    bytes: Vec<u8>,
    max_bytes: usize,
    attempted_bytes: u64,
    exceeded: bool,
}

impl BoundedBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
            attempted_bytes: 0,
            exceeded: false,
        }
    }
}

impl std::io::Write for BoundedBuffer {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let attempted = self.bytes.len().saturating_add(buffer.len());
        self.attempted_bytes = u64::try_from(attempted).unwrap_or(u64::MAX);
        if attempted > self.max_bytes {
            self.exceeded = true;
            return Err(std::io::Error::other("snapshot byte limit exceeded"));
        }
        self.bytes
            .try_reserve(buffer.len())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize)]
struct SnapshotRef<'a, K, V, M>
where
    K: Clone + std::hash::Hash + Eq,
    V: Clone,
    M: Clone,
{
    version: u32,
    entries: SnapshotEntriesRef<'a, K, V, M>,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone
"))]
struct Snapshot<K, V, M>
where
    K: Clone + std::hash::Hash + Eq,
    V: Clone,
    M: Clone,
{
    version: u32,
    #[serde(deserialize_with = "deserialize_snapshot_entries")]
    entries: EntryMap<K, V, M>,
}

struct RejectAdditionalElement(&'static str);

impl<'de> DeserializeSeed<'de> for RejectAdditionalElement {
    type Value = ();

    fn deserialize<D>(self, _deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom(self.0))
    }
}

struct HistorySeed<K, V, M> {
    max_entries: usize,
    marker: PhantomTypes<K, V, M>,
}

impl<K, V, M> HistorySeed<K, V, M> {
    fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            marker: std::marker::PhantomData,
        }
    }
}

impl<'de, K, V, M> DeserializeSeed<'de> for HistorySeed<K, V, M>
where
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone,
{
    type Value = Vec<CacheEntry<K, V, M>>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(HistoryVisitor {
            max_entries: self.max_entries,
            marker: self.marker,
        })
    }
}

struct HistoryVisitor<K, V, M> {
    max_entries: usize,
    marker: PhantomTypes<K, V, M>,
}

impl<'de, K, V, M> Visitor<'de> for HistoryVisitor<K, V, M>
where
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone,
{
    type Value = Vec<CacheEntry<K, V, M>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a cache entry history within the configured snapshot limit")
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence.size_hint().unwrap_or(0).min(self.max_entries);
        let mut entries = Vec::new();
        entries.try_reserve(capacity).map_err(de::Error::custom)?;

        loop {
            if entries.len() == self.max_entries {
                let additional = sequence.next_element_seed(RejectAdditionalElement(
                    "snapshot contains more than the maximum number of entries",
                ))?;
                debug_assert!(additional.is_none());
                break;
            }
            match sequence.next_element()? {
                Some(entry) => entries.push(entry),
                None => break,
            }
        }

        Ok(entries)
    }
}

struct SnapshotPairSeed<K, V, M> {
    remaining_entries: usize,
    marker: PhantomTypes<K, V, M>,
}

impl<K, V, M> SnapshotPairSeed<K, V, M> {
    fn new(remaining_entries: usize) -> Self {
        Self {
            remaining_entries,
            marker: std::marker::PhantomData,
        }
    }
}

impl<'de, K, V, M> DeserializeSeed<'de> for SnapshotPairSeed<K, V, M>
where
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone,
{
    type Value = (K, Vec<CacheEntry<K, V, M>>);

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(
            2,
            SnapshotPairVisitor {
                remaining_entries: self.remaining_entries,
                marker: self.marker,
            },
        )
    }
}

struct SnapshotPairVisitor<K, V, M> {
    remaining_entries: usize,
    marker: PhantomTypes<K, V, M>,
}

impl<'de, K, V, M> Visitor<'de> for SnapshotPairVisitor<K, V, M>
where
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone,
{
    type Value = (K, Vec<CacheEntry<K, V, M>>);

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a two-element [key, entry_history] pair")
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let key = sequence
            .next_element()?
            .ok_or_else(|| de::Error::custom("snapshot entry pair is missing its key"))?;
        let history = sequence
            .next_element_seed(HistorySeed::new(self.remaining_entries))?
            .ok_or_else(|| de::Error::custom("snapshot entry pair is missing its history"))?;
        let additional = sequence.next_element_seed(RejectAdditionalElement(
            "snapshot entry pair must contain exactly two elements",
        ))?;
        debug_assert!(additional.is_none());
        Ok((key, history))
    }
}

struct SnapshotEntriesVisitor<K, V, M> {
    max_keys: usize,
    max_entries: usize,
    marker: PhantomTypes<K, V, M>,
}

impl<'de, K, V, M> Visitor<'de> for SnapshotEntriesVisitor<K, V, M>
where
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone,
{
    type Value = EntryMap<K, V, M>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded array of cache entry pairs")
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence.size_hint().unwrap_or(0).min(self.max_keys);
        let mut entries = HashMap::new();
        entries.try_reserve(capacity).map_err(de::Error::custom)?;
        let mut total_entries = 0usize;

        loop {
            if entries.len() == self.max_keys {
                let additional = sequence.next_element_seed(RejectAdditionalElement(
                    "snapshot contains more than the maximum number of keys",
                ))?;
                debug_assert!(additional.is_none());
                break;
            }

            let remaining_entries = self.max_entries - total_entries;
            let Some((key, key_entries)) =
                sequence.next_element_seed(SnapshotPairSeed::new(remaining_entries))?
            else {
                break;
            };
            total_entries = total_entries
                .checked_add(key_entries.len())
                .ok_or_else(|| de::Error::custom("snapshot entry count overflowed usize"))?;
            if key_entries.iter().any(|entry| entry.key != key) {
                return Err(de::Error::custom(
                    "snapshot contains an entry whose embedded key does not match",
                ));
            }
            if entries.insert(key, key_entries).is_some() {
                return Err(de::Error::custom("snapshot contains a duplicate key"));
            }
        }

        Ok(entries)
    }
}

fn deserialize_snapshot_entries<'de, D, K, V, M>(
    deserializer: D,
) -> std::result::Result<EntryMap<K, V, M>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Clone + std::hash::Hash + Eq,
    V: Deserialize<'de> + Clone,
    M: Deserialize<'de> + Clone,
{
    deserializer.deserialize_seq(SnapshotEntriesVisitor {
        max_keys: MAX_SNAPSHOT_KEYS,
        max_entries: MAX_SNAPSHOT_ENTRIES,
        marker: std::marker::PhantomData,
    })
}

/// Filesystem storage backend.
///
/// Each backend instance is safe for concurrent use. Multiple backend
/// instances or processes must not write to the same directory concurrently.
#[allow(clippy::type_complexity)]
pub struct FilesystemBackend<K, V, M = ()>
where
    K: StorageKey,
    V: StorageValue,
    M: StorageMeta,
{
    base_path: PathBuf,
    max_snapshot_bytes: u64,
    io_lock: Arc<Mutex<()>>,
    _phantom: PhantomTypes<K, V, M>,
}

impl<K, V, M> FilesystemBackend<K, V, M>
where
    K: StorageKey,
    V: StorageValue,
    M: StorageMeta,
{
    /// Create a new filesystem backend with the given base path.
    pub async fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path).await?;
        let metadata = fs::symlink_metadata(&base_path).await?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CacheError::InvalidConfiguration(
                "filesystem cache path must be a real directory, not a symlink".to_string(),
            ));
        }

        Ok(Self {
            base_path,
            max_snapshot_bytes: DEFAULT_MAX_SNAPSHOT_BYTES,
            io_lock: Arc::new(Mutex::new(())),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Set the maximum accepted and generated snapshot size in bytes.
    ///
    /// A zero-byte limit is permitted but makes every snapshot write fail.
    pub fn with_max_snapshot_bytes(mut self, max_snapshot_bytes: u64) -> Self {
        self.max_snapshot_bytes = max_snapshot_bytes;
        self
    }

    fn snapshot_path(&self) -> PathBuf {
        self.base_path.join(SNAPSHOT_FILE_NAME)
    }

    fn temporary_path(&self) -> PathBuf {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        self.base_path.join(format!(
            "{TEMP_FILE_PREFIX}{}-{sequence}",
            std::process::id()
        ))
    }

    async fn has_legacy_layout(&self) -> Result<bool> {
        let mut directory = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = directory.next_entry().await? {
            let file_type = entry.file_type().await?;
            let path = entry.path();
            let is_legacy_extension = matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("json" | "bin")
            );
            let is_snapshot =
                path.file_name().and_then(|name| name.to_str()) == Some(SNAPSHOT_FILE_NAME);
            if file_type.is_symlink() && is_legacy_extension {
                return Err(CacheError::StorageBackend(
                    "cache directory contains a symlink with a persistence-file extension"
                        .to_string(),
                ));
            }
            if file_type.is_file() && is_legacy_extension && !is_snapshot {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn reject_legacy_layout(&self) -> Result<()> {
        if self.has_legacy_layout().await? {
            Err(CacheError::UnsupportedPersistenceFormat(
                "legacy per-key cache files were found; clear or migrate the cache directory"
                    .to_string(),
            ))
        } else {
            Ok(())
        }
    }

    async fn read_snapshot_bytes(&self) -> Result<Option<Vec<u8>>> {
        self.reject_legacy_layout().await?;
        let path = self.snapshot_path();
        let metadata = match fs::symlink_metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CacheError::StorageBackend(
                "cache snapshot must be a regular file".to_string(),
            ));
        }
        if metadata.len() > self.max_snapshot_bytes {
            return Err(CacheError::SnapshotTooLarge {
                actual_bytes: metadata.len(),
                max_bytes: self.max_snapshot_bytes,
            });
        }

        let mut bytes = Vec::new();
        let read_limit = self.max_snapshot_bytes.saturating_add(1);
        File::open(&path)
            .await?
            .take(read_limit)
            .read_to_end(&mut bytes)
            .await?;
        let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_bytes > self.max_snapshot_bytes {
            return Err(CacheError::SnapshotTooLarge {
                actual_bytes,
                max_bytes: self.max_snapshot_bytes,
            });
        }
        Ok(Some(bytes))
    }

    async fn load_unlocked(&self) -> Result<EntryMap<K, V, M>> {
        let Some(bytes) = self.read_snapshot_bytes().await? else {
            return Ok(HashMap::new());
        };
        let snapshot: Snapshot<K, V, M> = serde_json::from_slice(&bytes)
            .map_err(|error| CacheError::Deserialization(error.to_string()))?;
        if snapshot.version != SNAPSHOT_FORMAT_VERSION {
            return Err(CacheError::UnsupportedPersistenceFormat(format!(
                "snapshot version {} is not supported (expected {SNAPSHOT_FORMAT_VERSION})",
                snapshot.version
            )));
        }
        Ok(snapshot.entries)
    }

    async fn replace_snapshot(&self, bytes: &[u8]) -> Result<()> {
        let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_bytes > self.max_snapshot_bytes {
            return Err(CacheError::SnapshotTooLarge {
                actual_bytes,
                max_bytes: self.max_snapshot_bytes,
            });
        }

        let temporary_path = self.temporary_path();
        let write_result = async {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                options.mode(0o600);
            }
            let mut file = options.open(&temporary_path).await?;
            file.write_all(bytes).await?;
            file.flush().await?;
            file.sync_all().await?;
            drop(file);

            #[cfg(not(windows))]
            fs::rename(&temporary_path, self.snapshot_path()).await?;

            // `rename` does not replace an existing file on Windows. This
            // fallback preserves functionality there, though the replacement
            // window cannot be fully atomic with the standard library alone.
            #[cfg(windows)]
            {
                let snapshot_path = self.snapshot_path();
                match fs::rename(&temporary_path, &snapshot_path).await {
                    Ok(()) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::AlreadyExists
                                | std::io::ErrorKind::PermissionDenied
                        ) =>
                    {
                        fs::remove_file(&snapshot_path).await?;
                        fs::rename(&temporary_path, &snapshot_path).await?;
                    }
                    Err(error) => return Err(error),
                }
            }

            #[cfg(unix)]
            File::open(&self.base_path).await?.sync_all().await?;
            Ok::<(), std::io::Error>(())
        }
        .await;

        if write_result.is_err() {
            let _ = fs::remove_file(&temporary_path).await;
        }
        write_result.map_err(Into::into)
    }

    async fn save_unlocked(&self, entries: &EntryMap<K, V, M>) -> Result<()> {
        self.reject_legacy_layout().await?;
        if entries.len() > MAX_SNAPSHOT_KEYS {
            return Err(CacheError::CapacityExceeded {
                message: format!(
                    "snapshot contains {} keys; limit is {MAX_SNAPSHOT_KEYS}",
                    entries.len()
                ),
            });
        }
        let total_entries = entries.values().try_fold(0usize, |total, key_entries| {
            total
                .checked_add(key_entries.len())
                .ok_or_else(|| CacheError::CapacityExceeded {
                    message: "snapshot entry count overflowed usize".to_string(),
                })
        })?;
        if total_entries > MAX_SNAPSHOT_ENTRIES {
            return Err(CacheError::CapacityExceeded {
                message: format!("snapshot contains more than {MAX_SNAPSHOT_ENTRIES} entries"),
            });
        }
        for (key, key_entries) in entries {
            if key_entries.iter().any(|entry| &entry.key != key) {
                return Err(CacheError::Serialization(
                    "cannot persist an entry whose embedded key does not match".to_string(),
                ));
            }
        }
        let snapshot = SnapshotRef {
            version: SNAPSHOT_FORMAT_VERSION,
            entries: entries.iter().collect(),
        };
        let max_bytes = usize::try_from(self.max_snapshot_bytes).unwrap_or(usize::MAX);
        let mut writer = BoundedBuffer::new(max_bytes);
        if let Err(error) = serde_json::to_writer(&mut writer, &snapshot) {
            if writer.exceeded {
                return Err(CacheError::SnapshotTooLarge {
                    actual_bytes: writer.attempted_bytes,
                    max_bytes: self.max_snapshot_bytes,
                });
            }
            return Err(CacheError::Serialization(error.to_string()));
        }
        self.replace_snapshot(&writer.bytes).await
    }
}

#[async_trait]
impl<K, V, M> StorageBackend for FilesystemBackend<K, V, M>
where
    K: StorageKey,
    V: StorageValue,
    M: StorageMeta,
{
    type Value = V;
    type Key = K;
    type Metadata = M;

    async fn save(&self, entries: &EntryMap<K, V, M>) -> Result<()> {
        let _guard = self.io_lock.lock().await;
        self.save_unlocked(entries).await
    }

    async fn load(&self) -> Result<EntryMap<K, V, M>> {
        let _guard = self.io_lock.lock().await;
        self.load_unlocked().await
    }

    async fn remove(&self, key: &K) -> Result<()> {
        let _guard = self.io_lock.lock().await;
        let mut entries = self.load_unlocked().await?;
        if entries.remove(key).is_some() {
            self.save_unlocked(&entries).await?;
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let _guard = self.io_lock.lock().await;
        self.reject_legacy_layout().await?;
        let snapshot_path = self.snapshot_path();
        let metadata = match fs::symlink_metadata(&snapshot_path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CacheError::StorageBackend(
                "cache snapshot must be a regular file".to_string(),
            ));
        }
        fs::remove_file(snapshot_path).await?;
        #[cfg(unix)]
        File::open(&self.base_path).await?.sync_all().await?;
        Ok(())
    }

    async fn contains(&self, key: &K) -> Result<bool> {
        let _guard = self.io_lock.lock().await;
        Ok(self.load_unlocked().await?.contains_key(key))
    }

    async fn size_bytes(&self) -> Result<u64> {
        let _guard = self.io_lock.lock().await;
        self.reject_legacy_layout().await?;
        match fs::symlink_metadata(self.snapshot_path()).await {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                Ok(metadata.len())
            }
            Ok(_) => Err(CacheError::StorageBackend(
                "cache snapshot must be a regular file".to_string(),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.into()),
        }
    }

    async fn compact(&self) -> Result<()> {
        let _guard = self.io_lock.lock().await;
        let entries = self.load_unlocked().await?;
        self.save_unlocked(&entries).await
    }
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

    fn entries(values: &[(&str, &str)]) -> EntryMap<String, String, ()> {
        values
            .iter()
            .map(|(key, value)| {
                (
                    (*key).to_string(),
                    vec![CacheEntry::new((*key).to_string(), (*value).to_string())],
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn persists_and_atomically_replaces_snapshot() {
        let (temp_dir, backend) = new_backend().await;
        backend.save(&entries(&[("one", "1")])).await.unwrap();
        backend.save(&entries(&[("two", "2")])).await.unwrap();

        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["two"][0].value, "2");
        assert!(!loaded.contains_key("one"));

        let file_names: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            file_names,
            vec![std::ffi::OsString::from(SNAPSHOT_FILE_NAME)]
        );
    }

    #[tokio::test]
    async fn formerly_colliding_and_traversal_keys_round_trip() {
        let (_temp_dir, backend) = new_backend().await;
        let values = entries(&[("a/b", "slash"), ("a\\b", "backslash"), ("../x", "dot")]);
        backend.save(&values).await.unwrap();
        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded["a/b"][0].value, "slash");
        assert_eq!(loaded["a\\b"][0].value, "backslash");
        assert_eq!(loaded["../x"][0].value, "dot");
    }

    #[tokio::test]
    async fn corrupted_snapshot_is_an_error() {
        let (temp_dir, backend) = new_backend().await;
        fs::write(temp_dir.path().join(SNAPSHOT_FILE_NAME), b"not json")
            .await
            .unwrap();
        assert!(matches!(
            backend.load().await,
            Err(CacheError::Deserialization(_))
        ));
    }

    #[tokio::test]
    async fn oversized_snapshot_is_rejected_before_deserialization() {
        let (temp_dir, backend) = new_backend().await;
        fs::write(temp_dir.path().join(SNAPSHOT_FILE_NAME), b"123456789")
            .await
            .unwrap();
        let backend = backend.with_max_snapshot_bytes(8);
        assert!(matches!(
            backend.load().await,
            Err(CacheError::SnapshotTooLarge {
                actual_bytes: 9,
                max_bytes: 8
            })
        ));
    }

    #[tokio::test]
    async fn oversized_save_does_not_replace_the_previous_snapshot() {
        let (temp_dir, backend) = new_backend().await;
        backend
            .save(&entries(&[("stable", "value")]))
            .await
            .unwrap();

        let constrained: FilesystemBackend<String, String> =
            FilesystemBackend::new(temp_dir.path())
                .await
                .unwrap()
                .with_max_snapshot_bytes(32);
        let result = constrained
            .save(&entries(&[("large", &"x".repeat(1_024))]))
            .await;
        assert!(matches!(result, Err(CacheError::SnapshotTooLarge { .. })));

        let loaded = backend.load().await.unwrap();
        assert!(loaded.contains_key("stable"));
        assert!(!loaded.contains_key("large"));
    }

    #[test]
    fn snapshot_deserialization_enforces_limits_while_streaming() {
        let too_many_keys = serde_json::to_string(&vec![
            (
                "one".to_string(),
                vec![CacheEntry::<String, String>::new(
                    "one".to_string(),
                    "1".to_string(),
                )],
            ),
            ("two".to_string(), Vec::new()),
        ])
        .unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&too_many_keys);
        let key_result =
            deserializer.deserialize_seq(SnapshotEntriesVisitor::<String, String, ()> {
                max_keys: 1,
                max_entries: 10,
                marker: std::marker::PhantomData,
            });
        assert!(key_result.is_err());

        let too_many_entries = serde_json::to_string(&vec![(
            "one".to_string(),
            vec![
                CacheEntry::<String, String>::new("one".to_string(), "1".to_string()),
                CacheEntry::<String, String>::new("one".to_string(), "2".to_string()),
            ],
        )])
        .unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&too_many_entries);
        let entry_result =
            deserializer.deserialize_seq(SnapshotEntriesVisitor::<String, String, ()> {
                max_keys: 10,
                max_entries: 1,
                marker: std::marker::PhantomData,
            });
        assert!(entry_result.is_err());
    }

    #[tokio::test]
    async fn legacy_layout_is_reported_explicitly() {
        let (temp_dir, backend) = new_backend().await;
        fs::write(temp_dir.path().join("metadata.json"), b"{}")
            .await
            .unwrap();
        assert!(matches!(
            backend.load().await,
            Err(CacheError::UnsupportedPersistenceFormat(_))
        ));
    }

    #[tokio::test]
    async fn legacy_bincode_layout_is_reported_explicitly() {
        let (temp_dir, backend) = new_backend().await;
        fs::write(temp_dir.path().join("entry.bin"), b"legacy")
            .await
            .unwrap();
        assert!(matches!(
            backend.load().await,
            Err(CacheError::UnsupportedPersistenceFormat(_))
        ));
    }

    #[tokio::test]
    async fn clear_rejects_legacy_files_before_removing_the_snapshot() {
        let (temp_dir, backend) = new_backend().await;
        backend
            .save(&entries(&[("stable", "value")]))
            .await
            .unwrap();
        let legacy_path = temp_dir.path().join("legacy.json");
        fs::write(&legacy_path, b"{}").await.unwrap();

        assert!(matches!(
            backend.clear().await,
            Err(CacheError::UnsupportedPersistenceFormat(_))
        ));
        assert!(
            fs::try_exists(temp_dir.path().join(SNAPSHOT_FILE_NAME))
                .await
                .unwrap()
        );
        assert!(fs::try_exists(legacy_path).await.unwrap());
    }

    #[tokio::test]
    async fn unknown_snapshot_version_is_rejected() {
        let (temp_dir, backend) = new_backend().await;
        fs::write(
            temp_dir.path().join(SNAPSHOT_FILE_NAME),
            br#"{"version":99,"entries":[]}"#,
        )
        .await
        .unwrap();
        assert!(matches!(
            backend.load().await,
            Err(CacheError::UnsupportedPersistenceFormat(_))
        ));
    }

    #[tokio::test]
    async fn filesystem_backend_size_tracks_snapshot() {
        let (_temp_dir, backend) = new_backend().await;
        assert_eq!(backend.size_bytes().await.unwrap(), 0);
        backend.save(&entries(&[("key", "value")])).await.unwrap();
        assert!(backend.size_bytes().await.unwrap() > 0);
        backend.clear().await.unwrap();
        assert_eq!(backend.size_bytes().await.unwrap(), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_symlink_cache_directory() {
        use std::os::unix::fs::symlink;

        let target = TempDir::new().unwrap();
        let parent = TempDir::new().unwrap();
        let link = parent.path().join("cache-link");
        symlink(target.path(), &link).unwrap();
        let result = FilesystemBackend::<String, String>::new(&link).await;
        assert!(matches!(result, Err(CacheError::InvalidConfiguration(_))));
    }
}
