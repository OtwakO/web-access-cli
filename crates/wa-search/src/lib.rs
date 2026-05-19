//! SearXNG web search client.
//!
//! Queries a self-hosted SearXNG instance and returns deduplicated,
//! ranked search results.

use wa_core::error::WaError;
use wa_core::types::SearchResult;

/// Client for a SearXNG instance.
pub struct SearXNGClient {
    /// Base URL of the SearXNG instance (e.g. `http://localhost:8080`)
    instance_url: String,
    /// HTTP client
    client: reqwest::Client,
}

impl SearXNGClient {
    /// Create a new SearXNG client.
    ///
    /// The `instance_url` should be the root URL of the SearXNG instance,
    /// without a trailing slash.
    pub fn new(instance_url: String) -> Self {
        Self {
            instance_url,
            client: reqwest::Client::builder()
                .user_agent("wa/0.1 searxng-client")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    /// Build the search URL for a given query.
    fn build_search_url(&self, query: &str) -> String {
        format!(
            "{}/search?q={}&format=json&categories=general&safesearch=0",
            self.instance_url,
            urlencoding(query)
        )
    }

    /// Execute a search and return deduplicated results.
    ///
    /// SearXNG may return the same URL under different categories;
    /// only the first occurrence is kept.
    ///
    /// Returns `WaError::Search` on HTTP failures or bad JSON.
    /// Returns `WaError::RateLimit` on HTTP 429.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, WaError> {
        if query.trim().is_empty() {
            return Err(WaError::Search("empty query".into()));
        }

        let url = self.build_search_url(query);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                WaError::Search("request timed out".into())
            } else if e.is_connect() {
                WaError::Search(format!("connection refused: {}", e))
            } else {
                WaError::Search(format!("request failed: {}", e))
            }
        })?;

        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(WaError::RateLimit(
                "SearXNG rate limited".into(),
            ));
        }

        if !status.is_success() {
            return Err(WaError::Search(format!(
                "HTTP {} from SearXNG",
                status.as_u16()
            )));
        }

        let body = resp.text().await.map_err(|e| {
            WaError::Search(format!("failed to read response body: {}", e))
        })?;

        let parsed: SearXNGResponse =
            serde_json::from_str(&body).map_err(|e| {
                WaError::Search(format!("invalid JSON from SearXNG: {}", e))
            })?;

        // Deduplicate by URL (SearXNG sometimes returns the same page under
        // multiple categories).
        let mut seen = std::collections::HashSet::new();
        let results: Vec<SearchResult> = parsed
            .results
            .into_iter()
            .filter(|r| seen.insert(r.url.clone()))
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.unwrap_or_default(),
                img_src: if r.category == "images" {
                    r.img_src.filter(|s| !s.is_empty())
                } else {
                    None
                },
            })
            .take(limit)
            .collect();

        Ok(results)
    }

    /// Execute a search and return the raw JSON response body.
    ///
    /// This skips parsing and deduplication, returning exactly what
    /// SearXNG sent back. Useful for `--format raw` or piping to jq.
    /// Error handling (429, timeout, connection refused) is identical
    /// to `search()`.
    pub async fn search_raw(&self, query: &str) -> Result<String, WaError> {
        if query.trim().is_empty() {
            return Err(WaError::Search("empty query".into()));
        }

        let url = self.build_search_url(query);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                WaError::Search("request timed out".into())
            } else if e.is_connect() {
                WaError::Search(format!("connection refused: {}", e))
            } else {
                WaError::Search(format!("request failed: {}", e))
            }
        })?;

        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(WaError::RateLimit(
                "SearXNG rate limited".into(),
            ));
        }

        if !status.is_success() {
            return Err(WaError::Search(format!(
                "HTTP {} from SearXNG",
                status.as_u16()
            )));
        }

        resp.text().await.map_err(|e| {
            WaError::Search(format!("failed to read response body: {}", e))
        })
    }

    /// Check whether the SearXNG instance is reachable.
    pub async fn health_check(&self) -> Result<(), WaError> {
        let url = format!("{}/search?q=test&format=json", self.instance_url);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            WaError::Search(format!("health check failed: {}", e))
        })?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(WaError::Search(format!(
                "health check returned HTTP {}",
                resp.status().as_u16()
            )))
        }
    }
}

/// URL-encode a query string for use in a SearXNG request.
fn default_category() -> String {
    "general".into()
}

fn urlencoding(s: &str) -> String {
    // We only encode the values that would break the query string:
    // spaces become +, and special characters get percent-encoded.
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b' ' => result.push('+'),
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => result.push(byte as char),
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}

/// The JSON shape returned by SearXNG.
#[derive(serde::Deserialize)]
struct SearXNGResponse {
    results: Vec<SearXNGResultItem>,
}

#[derive(serde::Deserialize)]
struct SearXNGResultItem {
    title: String,
    url: String,
    /// SearXNG can return either `content` or `snippet`.
    content: Option<String>,
    #[serde(rename = "snippet")]
    _snippet: Option<String>,
    /// Result category, e.g. "general", "images", "news".
    #[serde(default = "default_category")]
    category: String,
    /// Image URL (usually only present for image-category results).
    img_src: Option<String>,
}
