//! Example showing how to migrate from file-scanner's cache to threatflux-cache
//!
//! This example requires the json-serialization feature.

#[cfg(feature = "json-serialization")]
mod with_json {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use threatflux_cache::prelude::*;
    use threatflux_cache::{PersistenceConfig, SearchQuery};

    // Replicate file-scanner's cache entry structure
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

    // Custom metadata for file analysis
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

    // Type aliases for compatibility
    type Value = serde_json::Value;

    // Adapter functions to maintain API compatibility
    pub struct FileAnalysisCacheAdapter {
        #[cfg(feature = "filesystem-backend")]
        #[allow(clippy::type_complexity)]
        cache: Cache<
            String,
            Value,
            FileAnalysisMetadata,
            FilesystemBackend<String, Value, FileAnalysisMetadata>,
        >,
        #[cfg(not(feature = "filesystem-backend"))]
        cache: Cache<
            String,
            Value,
            FileAnalysisMetadata,
            MemoryBackend<String, Value, FileAnalysisMetadata>,
        >,
    }

    impl FileAnalysisCacheAdapter {
        pub async fn new(cache_dir: &str) -> Result<Self> {
            let config = CacheConfig::default()
                .with_persistence(PersistenceConfig::with_path(cache_dir))
                .with_max_entries_per_key(100)
                .with_max_total_entries(10000);

            #[cfg(feature = "filesystem-backend")]
            let backend = FilesystemBackend::new(cache_dir).await?;
            #[cfg(not(feature = "filesystem-backend"))]
            let backend = MemoryBackend::new();

            let cache = Cache::new(config, backend).await?;

            Ok(Self { cache })
        }

        // File-scanner compatible API
        pub async fn add_analysis_result(&self, result: FileAnalysisResult) -> Result<()> {
            let metadata = FileAnalysisMetadata {
                file_path: result.file_path.clone(),
                file_size: result.file_size,
                tool_args: result.tool_args.clone(),
                tags: vec![result.tool_name.clone()],
            };

            let entry = CacheEntry::with_metadata(
                result.file_hash.clone(),
                serde_json::to_value(result)?,
                metadata,
            );

            self.cache.add_entry(entry).await
        }

        // Search by file hash
        pub async fn get_analysis_by_hash(&self, file_hash: &str) -> Option<serde_json::Value> {
            self.cache
                .get_latest(&file_hash.to_string())
                .await
                .map(|entry| entry.value)
        }

        // Search analyses by file path pattern
        pub async fn search_by_path(&self, path_pattern: &str) -> Vec<serde_json::Value> {
            let query = SearchQuery::new().with_pattern(path_pattern);
            self.cache
                .search(&query)
                .await
                .into_iter()
                .map(|entry| entry.value)
                .collect()
        }
    }

    pub async fn run_example() -> Result<()> {
        // Create adapter with file-scanner compatible API
        let adapter = FileAnalysisCacheAdapter::new("/tmp/file-scanner-cache").await?;

        // Add an analysis result (mimicking file-scanner usage)
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
            timestamp: chrono::Utc::now(),
            file_size: 123456,
            execution_time_ms: 45,
        };

        adapter.add_analysis_result(analysis).await?;

        // Retrieve analysis
        if let Some(result) = adapter.get_analysis_by_hash("abc123def456").await {
            println!("Retrieved analysis result:");
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        // Search by file path
        let results = adapter.search_by_path("/bin").await;
        println!("Found {} results for /bin pattern", results.len());

        Ok(())
    }
}

#[cfg(feature = "json-serialization")]
#[tokio::main]
#[allow(clippy::type_complexity)]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    with_json::run_example().await?;
    Ok(())
}

#[cfg(not(feature = "json-serialization"))]
fn main() {
    println!("This example requires the 'json-serialization' feature to be enabled.");
    println!("Run with: cargo run --example file_scanner_migration --features json-serialization");
}
