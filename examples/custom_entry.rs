//! Custom entry metadata, version history, and search.

use serde::{Deserialize, Serialize};
use threatflux_cache::prelude::*;
use threatflux_cache::{EvictionPolicy, SearchQuery, entry::BasicMetadata};

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
type DocCache = Cache<String, Document, BasicMetadata>;

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
        .with_max_entries_per_key(5)
        .with_eviction_policy(EvictionPolicy::Lru);
    Cache::with_config(config).await.map_err(Into::into)
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
    // Pattern matching is a case-sensitive substring search over the key.
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
    let key = "doc:1".to_owned();
    if let Some(entries) = cache.get_entries(&key).await {
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
