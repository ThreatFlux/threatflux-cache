//! Error types for the cache library

use std::io;
use thiserror::Error;

/// Main error type for cache operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CacheError {
    /// I/O error occurred during cache operations
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Cache capacity exceeded
    #[error("Cache capacity exceeded: {message}")]
    CapacityExceeded {
        /// Error message
        message: String,
    },

    /// Storage backend error
    #[error("Storage backend error: {0}")]
    StorageBackend(String),

    /// Entry not found
    #[error("Entry not found for key")]
    NotFound,

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// A persistence snapshot exceeded the configured byte limit
    #[error("Persistence snapshot is too large ({actual_bytes} bytes; limit is {max_bytes})")]
    SnapshotTooLarge {
        /// Actual snapshot size in bytes
        actual_bytes: u64,
        /// Configured maximum size in bytes
        max_bytes: u64,
    },

    /// The persisted data uses an unsupported layout or format version
    #[error("Unsupported persistence format: {0}")]
    UnsupportedPersistenceFormat(String),

    /// Custom error for extensions
    #[error("Custom error: {0}")]
    Custom(String),
}

/// Result type alias for cache operations
pub type Result<T> = std::result::Result<T, CacheError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_error_variants() {
        let io_err: CacheError = io::Error::other("oops").into();
        assert!(matches!(io_err, CacheError::Io(_)));

        let ser_err = CacheError::Serialization("ser".into());
        assert_eq!(format!("{ser_err}"), "Serialization error: ser");

        let des_err = CacheError::Deserialization("de".into());
        assert_eq!(format!("{des_err}"), "Deserialization error: de");

        let cap_err = CacheError::CapacityExceeded {
            message: "full".into(),
        };
        assert!(matches!(cap_err, CacheError::CapacityExceeded { .. }));

        let backend_err = CacheError::StorageBackend("be".into());
        assert!(matches!(backend_err, CacheError::StorageBackend(_)));

        let not_found = CacheError::NotFound;
        assert_eq!(format!("{not_found}"), "Entry not found for key");

        let custom = CacheError::Custom("c".into());
        assert_eq!(format!("{custom}"), "Custom error: c");
    }
}
