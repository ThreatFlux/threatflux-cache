//! Configuration types for the cache

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of entries per key
    pub max_entries_per_key: usize,
    /// Maximum total number of entries
    pub max_total_entries: usize,
    /// Eviction policy to use
    pub eviction_policy: EvictionPolicy,
    /// Persistence configuration
    pub persistence: PersistenceConfig,
    /// Enable compression for stored values
    #[cfg(feature = "compression")]
    pub compression: Option<CompressionConfig>,
    /// Default TTL for entries (if not specified per-entry)
    pub default_ttl: Option<Duration>,
    /// Enable metrics collection
    #[cfg(feature = "metrics")]
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries_per_key: 100,
            max_total_entries: 10_000,
            eviction_policy: EvictionPolicy::Lru,
            persistence: PersistenceConfig::default(),
            #[cfg(feature = "compression")]
            compression: None,
            default_ttl: None,
            #[cfg(feature = "metrics")]
            enable_metrics: false,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum entries per key
    pub fn with_max_entries_per_key(mut self, max: usize) -> Self {
        self.max_entries_per_key = max;
        self
    }

    /// Set maximum total entries
    pub fn with_max_total_entries(mut self, max: usize) -> Self {
        self.max_total_entries = max;
        self
    }

    /// Set eviction policy
    pub fn with_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// Set persistence configuration
    pub fn with_persistence(mut self, persistence: PersistenceConfig) -> Self {
        self.persistence = persistence;
        self
    }

    /// Set default TTL for entries
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Enable compression with given configuration
    #[cfg(feature = "compression")]
    pub fn with_compression(mut self, compression: CompressionConfig) -> Self {
        self.compression = Some(compression);
        self
    }

    /// Enable metrics collection
    #[cfg(feature = "metrics")]
    pub fn with_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }
}

/// Eviction policy for cache entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used
    Lfu,
    /// First In First Out
    Fifo,
    /// Time To Live based
    Ttl,
    /// No eviction (manual only)
    None,
}

/// Persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Enable persistence
    pub enabled: bool,
    /// Path to store cache data
    pub path: Option<PathBuf>,
    /// Sync to disk after every N operations
    pub sync_interval: usize,
    /// Automatically save on drop
    pub save_on_drop: bool,
    /// Load existing cache on startup
    pub load_on_startup: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            sync_interval: 100,
            save_on_drop: true,
            load_on_startup: true,
        }
    }
}

impl PersistenceConfig {
    /// Create persistence config with path
    pub fn with_path<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            enabled: true,
            path: Some(path.into()),
            ..Default::default()
        }
    }

    /// Disable persistence
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// Compression configuration
#[cfg(feature = "compression")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Compression algorithm to use
    pub algorithm: CompressionAlgorithm,
    /// Compression level (1-9, higher = better compression, slower)
    pub level: u32,
    /// Minimum size in bytes before compression is applied
    pub min_size: usize,
}

#[cfg(feature = "compression")]
impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Gzip,
            level: 6,
            min_size: 1024, // 1KB
        }
    }
}

/// Supported compression algorithms
#[cfg(feature = "compression")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// Gzip compression
    Gzip,
    /// Zlib compression
    Zlib,
    /// Raw DEFLATE compression
    Deflate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CacheConfig::default();
        assert_eq!(config.max_entries_per_key, 100);
        assert_eq!(config.max_total_entries, 10_000);
        assert_eq!(config.eviction_policy, EvictionPolicy::Lru);
        assert!(!config.persistence.enabled);
    }

    #[test]
    fn test_config_builder() {
        let config = CacheConfig::new()
            .with_max_entries_per_key(50)
            .with_max_total_entries(5000)
            .with_eviction_policy(EvictionPolicy::Lfu)
            .with_default_ttl(Duration::from_secs(300));

        assert_eq!(config.max_entries_per_key, 50);
        assert_eq!(config.max_total_entries, 5000);
        assert_eq!(config.eviction_policy, EvictionPolicy::Lfu);
        assert_eq!(config.default_ttl, Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_persistence_config() {
        let persistence = PersistenceConfig::with_path("/tmp/cache");
        assert!(persistence.enabled);
        assert_eq!(persistence.path, Some(PathBuf::from("/tmp/cache")));
        assert_eq!(persistence.sync_interval, 100);
        assert!(persistence.save_on_drop);
        assert!(persistence.load_on_startup);
    }

    #[test]
    fn test_persistence_config_disabled() {
        let persistence = PersistenceConfig::disabled();
        assert!(!persistence.enabled);
        assert_eq!(persistence.path, None);
    }

    #[test]
    fn test_with_persistence_builder() {
        let p = PersistenceConfig::with_path("/tmp/data");
        let config = CacheConfig::new().with_persistence(p.clone());
        assert!(config.persistence.enabled);
        assert_eq!(config.persistence.path, p.path);
    }

    #[cfg(feature = "compression")]
    #[test]
    fn test_compression_config() {
        let compression = CompressionConfig::default();
        assert_eq!(compression.algorithm, CompressionAlgorithm::Gzip);
        assert_eq!(compression.level, 6);
        assert_eq!(compression.min_size, 1024);
    }
}
