//! Adapter pattern for migrating a file-analysis cache to ThreatFlux Cache.
//!
//! This example requires the `filesystem-backend` feature and treats cached
//! analysis as reconstructible data rather than a system of record.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use threatflux_cache::prelude::*;
use threatflux_cache::{CacheError, PersistenceConfig};

// Replicate file-scanner's cache entry structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysisResult {
    pub file_path: String,
    pub file_hash: String,
    pub tool_name: String,
    pub tool_args: HashMap<String, serde_json::Value>,
    pub result: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub file_size: u64,
    pub execution_time_ms: u64,
}

// Custom metadata for file analysis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FileAnalysisMetadata {
    pub file_path: String,
    pub file_size: u64,
    pub tool_args: HashMap<String, serde_json::Value>,
    pub tags: Vec<String>,
}

impl EntryMetadata for FileAnalysisMetadata {
    fn size_bytes(&self) -> Option<u64> {
        Some(self.file_size)
    }

    fn category(&self) -> Option<&str> {
        Some("file_analysis")
    }
}

type Value = serde_json::Value;

pub struct FileAnalysisCacheAdapter {
    #[allow(clippy::type_complexity)]
    cache: Cache<
        String,
        Value,
        FileAnalysisMetadata,
        FilesystemBackend<String, Value, FileAnalysisMetadata>,
    >,
}

impl FileAnalysisCacheAdapter {
    pub async fn new(cache_dir: impl AsRef<Path>) -> Result<Self> {
        let config = CacheConfig::default()
            .with_max_entries_per_key(100)
            .with_max_total_entries(10000)
            .with_persistence(PersistenceConfig::enabled());
        let backend = FilesystemBackend::new(cache_dir).await?;
        let cache = Cache::new(config, backend).await?;

        Ok(Self { cache })
    }

    // File-scanner compatible API.
    pub async fn add_analysis_result(&self, result: FileAnalysisResult) -> Result<()> {
        let file_hash = result.file_hash.clone();
        let metadata = FileAnalysisMetadata {
            file_path: result.file_path.clone(),
            file_size: result.file_size,
            tool_args: result.tool_args.clone(),
            tags: vec![result.tool_name.clone()],
        };

        let value = serde_json::to_value(result)
            .map_err(|error| CacheError::Serialization(error.to_string()))?;
        let entry = CacheEntry::with_metadata(file_hash, value, metadata);

        self.cache.add_entry(entry).await
    }

    /// Persist all completed writes before a controlled shutdown.
    pub async fn flush(&self) -> Result<()> {
        self.cache.flush().await
    }

    // Search by file hash.
    pub async fn get_analysis_by_hash(&self, file_hash: &str) -> Option<serde_json::Value> {
        self.cache
            .get_latest(&file_hash.to_string())
            .await
            .map(|entry| entry.value)
    }
}

async fn run_example() -> Result<()> {
    let cache_dir = tempfile::tempdir()?;
    let adapter = FileAnalysisCacheAdapter::new(cache_dir.path()).await?;

    let analysis = FileAnalysisResult {
        file_path: "/bin/ls".to_string(),
        file_hash: "abc123def456".to_string(),
        tool_name: "calculate_hashes".to_string(),
        tool_args: {
            let mut args = HashMap::new();
            args.insert(
                "algorithm".to_string(),
                serde_json::Value::String("sha256".to_string()),
            );
            args
        },
        result: serde_json::json!({
            "sha256": "a1b2c3d4e5f6...",
            "md5": "1a2b3c4d5e6f...",
            "file_type": "ELF 64-bit LSB executable"
        }),
        timestamp: Utc::now(),
        file_size: 123456,
        execution_time_ms: 45,
    };

    let file_hash = analysis.file_hash.clone();
    adapter.add_analysis_result(analysis).await?;
    adapter.flush().await?;

    if let Some(result) = adapter.get_analysis_by_hash(&file_hash).await {
        println!("Retrieved analysis result:");
        let formatted = serde_json::to_string_pretty(&result)
            .map_err(|error| CacheError::Serialization(error.to_string()))?;
        println!("{formatted}");
    }

    Ok(())
}

#[tokio::main]
#[allow(clippy::type_complexity)]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    run_example().await?;
    Ok(())
}
