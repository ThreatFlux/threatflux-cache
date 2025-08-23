//! Storage backend implementations

use crate::EntryMetadata;
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

/// Bounds required for backend keys
pub trait BackendKey: Hash + Eq + Clone + Send + Sync {}
impl<T> BackendKey for T where T: Hash + Eq + Clone + Send + Sync {}

/// Bounds required for backend values
pub trait BackendValue: Clone + Send + Sync {}
impl<T> BackendValue for T where T: Clone + Send + Sync {}

/// Bounds required for backend metadata
pub trait BackendMeta: Clone + Send + Sync {}
impl<T> BackendMeta for T where T: Clone + Send + Sync {}

/// Combined bounds for storage keys
pub trait StorageKey: BackendKey + Serialize + DeserializeOwned + 'static {}
impl<T> StorageKey for T where T: BackendKey + Serialize + DeserializeOwned + 'static {}

/// Combined bounds for storage values
pub trait StorageValue: BackendValue + Serialize + DeserializeOwned + 'static {}
impl<T> StorageValue for T where T: BackendValue + Serialize + DeserializeOwned + 'static {}

/// Combined bounds for storage metadata
pub trait StorageMeta: BackendMeta + Serialize + DeserializeOwned + EntryMetadata {}
impl<T> StorageMeta for T where T: BackendMeta + Serialize + DeserializeOwned + EntryMetadata {}

pub mod memory;

#[cfg(feature = "filesystem-backend")]
pub mod filesystem;
