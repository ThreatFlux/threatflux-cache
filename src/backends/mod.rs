//! Storage backend implementations

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

pub mod memory;

#[cfg(feature = "filesystem-backend")]
pub mod filesystem;
