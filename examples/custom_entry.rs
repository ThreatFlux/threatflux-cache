//! Example showing custom entry metadata and search functionality

use serde::{Deserialize, Serialize};
use threatflux_cache::prelude::*;
use threatflux_cache::{entry::BasicMetadata, EvictionPolicy, PersistenceConfig, SearchQuery};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    id: String,
    title: String,
    content: String,
}

fn make_entry(
    id: &str,
    title: &str,
    content: &str,
    category: &str,
    tags: &[&str],
    exec_time: u64,
) -> CacheEntry<String, Document, BasicMetadata> {
    let doc = Document {
        id: format!("doc{id}"),
        title: title.to_string(),
        content: content.to_string(),
    };
    let metadata = BasicMetadata {
        execution_time_ms: Some(exec_time),
        size_bytes: Some(doc.content.len() as u64),
        category: Some(category.to_string()),
        tags: tags.iter().map(|t| (*t).to_string()).collect(),
    };
    CacheEntry::with_metadata(format!("doc:{id}"), doc, metadata)
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[cfg(feature = "filesystem-backend")]
type Backend = FilesystemBackend<String, Document, BasicMetadata>;
#[cfg(not(feature = "filesystem-backend"))]
type Backend = MemoryBackend<String, Document, BasicMetadata>;

type DocCache = Cache<String, Document, BasicMetadata, Backend>;

#[tokio::main]
async fn main() -> Result<()> {
    let cache = build_cache().await?;
    populate_cache(&cache).await?;
    search_and_display(&cache).await;
    show_entries(&cache).await?;
    Ok(())
}

async fn build_cache() -> Result<DocCache> {
    let config = CacheConfig::default()
        .with_persistence(PersistenceConfig::with_path("/tmp/document-cache"))
        .with_eviction_policy(EvictionPolicy::Lru);

    #[cfg(feature = "filesystem-backend")]
    let backend = FilesystemBackend::new("/tmp/document-cache").await?;
    #[cfg(not(feature = "filesystem-backend"))]
    let backend = MemoryBackend::new();

    Cache::new(config, backend).await.map_err(Into::into)
}

async fn populate_cache(cache: &DocCache) -> Result<()> {
    let docs = [
        (
            "1",
            "Introduction to Rust",
            "Rust is a systems programming language...",
            "tutorial",
            &["rust", "programming"][..],
            45,
        ),
        (
            "2",
            "Advanced Rust Patterns",
            "This document covers advanced patterns...",
            "advanced",
            &["rust", "patterns"][..],
            30,
        ),
    ];
    for (id, title, content, category, tags, exec) in docs {
        cache
            .add_entry(make_entry(id, title, content, category, tags, exec))
            .await?;
    }
    Ok(())
}

async fn search_and_display(cache: &DocCache) {
    let query = SearchQuery::new()
        .with_pattern("doc")
        .with_category("tutorial");
    let results = cache.search(&query).await;
    println!("Found {} documents matching query", results.len());
    for result in results {
        println!(
            "- {} (category: {:?})",
            result.value.title,
            result.metadata.category()
        );
    }
}

async fn show_entries(cache: &DocCache) -> Result<()> {
    if let Some(entries) = cache.get_entries(&"doc:1".to_string()).await {
        for entry in entries {
            println!(
                "Entry: {} - Access count: {}, Age: {:?}",
                entry.value.title,
                entry.access_count,
                entry.age()
            );
        }
    }
    Ok(())
}
