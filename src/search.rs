//! Search and query functionality for cache entries

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Trait for searchable cache entries
pub trait Searchable {
    /// Query type for searching
    type Query;

    /// Check if this entry matches the query
    fn matches(&self, query: &Self::Query) -> bool;
}

/// Basic search query for cache entries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchQuery {
    /// Pattern to match in string representation
    pub pattern: Option<String>,
    /// Minimum timestamp
    pub min_timestamp: Option<DateTime<Utc>>,
    /// Maximum timestamp
    pub max_timestamp: Option<DateTime<Utc>>,
    /// Minimum access count
    pub min_access_count: Option<u64>,
    /// Maximum access count
    pub max_access_count: Option<u64>,
    /// Include expired entries
    pub include_expired: bool,
    /// Category filter
    pub category: Option<String>,
}

impl SearchQuery {
    /// Create a new empty search query
    pub fn new() -> Self {
        Self::default()
    }

    /// Set pattern to search for
    pub fn with_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// Set timestamp range
    pub fn with_timestamp_range(
        mut self,
        min: Option<DateTime<Utc>>,
        max: Option<DateTime<Utc>>,
    ) -> Self {
        self.min_timestamp = min;
        self.max_timestamp = max;
        self
    }

    /// Set access count range
    pub fn with_access_count_range(mut self, min: Option<u64>, max: Option<u64>) -> Self {
        self.min_access_count = min;
        self.max_access_count = max;
        self
    }

    /// Set whether to include expired entries
    pub fn include_expired(mut self, include: bool) -> Self {
        self.include_expired = include;
        self
    }

    /// Set category filter
    pub fn with_category<S: Into<String>>(mut self, category: S) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// Implement Searchable for common types
impl<K, V, M> Searchable for crate::CacheEntry<K, V, M>
where
    K: Clone + std::hash::Hash + Eq + std::fmt::Display,
    V: Clone + std::fmt::Debug,
    M: Clone + crate::EntryMetadata,
{
    type Query = SearchQuery;

    fn matches(&self, query: &Self::Query) -> bool {
        let key_str = self.key.to_string();
        (query.include_expired || !self.is_expired())
            && query.pattern.as_ref().is_none_or(|p| key_str.contains(p))
            && query.min_timestamp.is_none_or(|min| self.timestamp >= min)
            && query.max_timestamp.is_none_or(|max| self.timestamp <= max)
            && query
                .min_access_count
                .is_none_or(|min| self.access_count >= min)
            && query
                .max_access_count
                .is_none_or(|max| self.access_count <= max)
            && query
                .category
                .as_ref()
                .is_none_or(|category| self.metadata.category().is_some_and(|c| c == category))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheEntry;

    #[test]
    fn test_search_query_builder() {
        let query = SearchQuery::new()
            .with_pattern("test")
            .with_access_count_range(Some(5), Some(10))
            .include_expired(true);

        assert_eq!(query.pattern, Some("test".to_string()));
        assert_eq!(query.min_access_count, Some(5));
        assert_eq!(query.max_access_count, Some(10));
        assert!(query.include_expired);
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn test_cache_entry_search() {
        let mut entry: CacheEntry<String, String, ()> =
            CacheEntry::new("test_key".to_string(), "test_value".to_string());
        entry.access_count = 7;

        // Test pattern matching
        let query1 = SearchQuery::new().with_pattern("test");
        assert!(entry.matches(&query1));

        let query2 = SearchQuery::new().with_pattern("notfound");
        assert!(!entry.matches(&query2));

        // Test access count range
        let query3 = SearchQuery::new().with_access_count_range(Some(5), Some(10));
        assert!(entry.matches(&query3));

        let query4 = SearchQuery::new().with_access_count_range(Some(10), None);
        assert!(!entry.matches(&query4));
    }

    #[test]
    fn test_search_query_timestamp_category() {
        let now = Utc::now();
        let query = SearchQuery::new()
            .with_timestamp_range(
                Some(now - chrono::Duration::seconds(1)),
                Some(now + chrono::Duration::seconds(1)),
            )
            .with_category("api");
        assert!(query.min_timestamp.is_some());
        assert_eq!(query.category, Some("api".to_string()));
    }

    #[test]
    fn test_cache_entry_search_branches() {
        use crate::entry::BasicMetadata;
        let metadata = BasicMetadata {
            category: Some("cat".to_string()),
            ..Default::default()
        };
        let mut entry = CacheEntry::with_metadata("k".to_string(), "v".to_string(), metadata);

        let past = entry.timestamp - chrono::Duration::seconds(10);
        let future = entry.timestamp + chrono::Duration::seconds(10);

        // timestamp range not matching
        let q = SearchQuery::new().with_timestamp_range(Some(future), None);
        assert!(!entry.matches(&q));

        // timestamp range matching
        let q2 = SearchQuery::new().with_timestamp_range(Some(past), Some(future));
        assert!(entry.matches(&q2));

        // category matching
        let q3 = SearchQuery::new().with_category("cat");
        assert!(entry.matches(&q3));

        // category mismatch
        let q4 = SearchQuery::new().with_category("other");
        assert!(!entry.matches(&q4));

        // expired handling
        entry.expiry = Some(entry.timestamp - chrono::Duration::seconds(1));
        let q5 = SearchQuery::new();
        assert!(!entry.matches(&q5));
        let q6 = SearchQuery::new().include_expired(true);
        assert!(entry.matches(&q6));
    }
}
