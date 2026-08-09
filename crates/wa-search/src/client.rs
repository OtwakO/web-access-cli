// Provider-neutral search interface and shared result policy.

use crate::model::{SearchDegradationWarning, SearchResponse, UnavailableUpstream};
use crate::providers::{DegoogProvider, Provider, ProviderSearchResponse, SearxngProvider};
use wa_core::error::WaError;
use wa_core::types::SearchResult;

/// Configuration for one concrete search provider adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchProviderConfig {
    Searxng {
        url: String,
    },
    Degoog {
        url: String,
        api_key: Option<String>,
    },
}

/// Provider-neutral web search client.
pub struct SearchClient {
    provider: Provider,
}

impl SearchClient {
    pub fn new(config: SearchProviderConfig) -> Self {
        let provider = match config {
            SearchProviderConfig::Searxng { url } => Provider::Searxng(SearxngProvider::new(url)),
            SearchProviderConfig::Degoog { url, api_key } => {
                Provider::Degoog(DegoogProvider::new(url, api_key))
            }
        };
        Self { provider }
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, WaError> {
        Ok(self.search_with_diagnostics(query, limit).await?.results)
    }

    /// Search once, retrying an empty response with upstream failures exactly once.
    pub async fn search_with_diagnostics(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchResponse, WaError> {
        validate_query(query)?;

        let first = self.provider.search_once(query).await?;
        if !first.results.is_empty() || first.upstream_failures.is_empty() {
            return Ok(normalize(first, limit, false));
        }

        let retry = self.provider.search_once(query).await?;
        let degraded = retry.results.is_empty() && !retry.upstream_failures.is_empty();
        Ok(normalize(retry, limit, degraded))
    }

    /// Return one untouched native response body from the selected provider.
    pub async fn search_raw(&self, query: &str) -> Result<String, WaError> {
        validate_query(query)?;
        self.provider.search_raw(query).await
    }
}

fn validate_query(query: &str) -> Result<(), WaError> {
    if query.trim().is_empty() {
        Err(WaError::Search("empty query".into()))
    } else {
        Ok(())
    }
}

fn normalize(response: ProviderSearchResponse, limit: usize, degraded: bool) -> SearchResponse {
    let mut seen = std::collections::HashSet::new();
    let results = response
        .results
        .into_iter()
        .filter(|result| seen.insert(result.url.clone()))
        .take(limit)
        .collect();
    let warning = degraded.then(|| SearchDegradationWarning {
        upstreams: response
            .upstream_failures
            .into_iter()
            .map(|failure| UnavailableUpstream {
                name: failure.name,
                reason: failure.reason,
            })
            .collect(),
    });

    SearchResponse { results, warning }
}
