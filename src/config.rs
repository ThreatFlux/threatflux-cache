//! Configuration types for the cache

use serde::{Deserialize, Serialize};
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
    /// Default TTL for entries (if not specified per-entry)
    pub default_ttl: Option<Duration>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries_per_key: 100,
            max_total_entries: 10_000,
            eviction_policy: EvictionPolicy::Lru,
            persistence: PersistenceConfig::default(),
            default_ttl: None,
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
    /// Sync to disk after every N operations
    pub sync_interval: usize,
    /// Load existing cache on startup
    pub load_on_startup: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sync_interval: 100,
            load_on_startup: true,
        }
    }
}

impl PersistenceConfig {
    /// Create an enabled persistence configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
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
        let persistence = PersistenceConfig::enabled();
        assert!(persistence.enabled);
        assert_eq!(persistence.sync_interval, 100);
        assert!(persistence.load_on_startup);
    }

    #[test]
    fn test_persistence_config_disabled() {
        let persistence = PersistenceConfig::disabled();
        assert!(!persistence.enabled);
    }

    #[test]
    fn test_with_persistence_builder() {
        let p = PersistenceConfig::enabled();
        let config = CacheConfig::new().with_persistence(p.clone());
        assert!(config.persistence.enabled);
    }
}
