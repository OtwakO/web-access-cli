// Provider-neutral search results and degradation diagnostics.

use wa_core::types::SearchResult;

/// Search results plus evidence that upstream search engines were unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub warning: Option<SearchDegradationWarning>,
}

/// Upstream failures that prevented the selected provider from returning results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDegradationWarning {
    pub upstreams: Vec<UnavailableUpstream>,
}

/// One upstream search engine that did not answer successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableUpstream {
    pub name: String,
    pub reason: String,
}
