//! Content extraction via webclaw-fetch.
//!
//! Wraps webclaw-fetch's `FetchClient` for HTTP fetching + webclaw-core
//! extraction in a single call.  The extraction pipeline produces
//! **webclaw-core** `ExtractionResult` values — NOT readability output.
//!
//! Re-exports `webclaw_core::ExtractionResult` so callers (wa-cli) don't
//! need a direct dependency on webclaw-core.

use std::sync::Arc;
use wa_core::error::WaError;

// Re-export for callers
pub use webclaw_core::endpoints::EndpointReport;
pub use webclaw_core::extract;
pub use webclaw_core::extract_with_options;
pub use webclaw_core::to_llm_text;
pub use webclaw_core::ExtractionOptions;
pub use webclaw_core::ExtractionResult;

/// Browser profile for TLS fingerprinting.
///
/// This is a simpler enum that mirrors `webclaw_fetch::BrowserProfile`.
/// The CLI crate provides its own `clap::ValueEnum` and converts via
/// `BrowserProfile::try_from_str`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProfile {
    Chrome,
    Firefox,
    SafariIos,
    Random,
}

impl BrowserProfile {
    /// Parse from a CLI argument string (case-insensitive).
    pub fn try_from_str(s: &str) -> Result<Self, WaError> {
        match s.to_lowercase().as_str() {
            "chrome" => Ok(Self::Chrome),
            "firefox" => Ok(Self::Firefox),
            "safari-ios" | "safari_ios" | "safariios" => Ok(Self::SafariIos),
            "random" => Ok(Self::Random),
            other => Err(WaError::Config(format!(
                "unknown browser profile '{}'; expected chrome, firefox, safari-ios, or random",
                other
            ))),
        }
    }

    /// Convert to webclaw-fetch's BrowserProfile.
    pub fn to_webclaw(self) -> webclaw_fetch::BrowserProfile {
        match self {
            BrowserProfile::Chrome => webclaw_fetch::BrowserProfile::Chrome,
            BrowserProfile::Firefox => webclaw_fetch::BrowserProfile::Firefox,
            BrowserProfile::SafariIos => webclaw_fetch::BrowserProfile::SafariIos,
            BrowserProfile::Random => webclaw_fetch::BrowserProfile::Random,
        }
    }
}

/// Content extractor wrapping webclaw-fetch.
///
/// Holds an `Arc<FetchClient>` so it can be shared across tasks
/// for concurrent batch fetches.
#[derive(Clone)]
pub struct Extractor {
    client: Arc<webclaw_fetch::FetchClient>,
}

impl Extractor {
    /// Create a new extractor with the given configuration.
    ///
    /// # Arguments
    /// - `browser` — which browser profile to use for TLS fingerprinting
    /// - `proxy` — optional SOCKS/HTTP proxy URL
    /// - `cookies` — optional list of `"name=value"` strings joined into a Cookie header
    /// - `timeout_secs` — request timeout in seconds
    pub fn new(
        browser: BrowserProfile,
        proxy: Option<String>,
        cookies: Option<Vec<String>>,
        timeout_secs: u64,
    ) -> Self {
        let mut headers = std::collections::HashMap::new();
        if let Some(ref cookies_vec) = cookies {
            let cookie_header = cookies_vec.join("; ");
            if !cookie_header.is_empty() {
                headers.insert("Cookie".to_string(), cookie_header);
            }
        }

        let config = webclaw_fetch::FetchConfig {
            browser: browser.to_webclaw(),
            proxy,
            proxy_pool: vec![],
            timeout: std::time::Duration::from_secs(timeout_secs),
            follow_redirects: true,
            max_redirects: 10,
            headers,
            pdf_mode: webclaw_fetch::PdfMode::default(),
        };

        let client = webclaw_fetch::FetchClient::new(config)
            .expect("failed to create webclaw FetchClient");

        Self {
            client: Arc::new(client),
        }
    }

    /// Fetch a URL, extract content, and return the result.
    ///
    /// Delegates to `FetchClient::fetch_and_extract_with_options()` which
    /// handles Reddit JSON fallback, PDF detection, Akamai cookie warmup,
    /// and LinkedIn embedded JSON extraction *before* calling
    /// `webclaw_core::extract_with_options()`.
    pub async fn fetch_and_extract(
        &self,
        url: &str,
        options: &ExtractionOptions,
    ) -> Result<ExtractionResult, WaError> {
        self.client
            .fetch_and_extract_with_options(url, options)
            .await
            .map_err(|e| WaError::fetch(url, format!("{}", e)))
    }

    /// Fetch raw HTML without extraction.
    ///
    /// Uses `fetch_smart`, so Reddit URLs are rewritten to `old.reddit.com`
    /// and Akamai-style challenge pages trigger a homepage cookie warmup
    /// before retrying. This keeps every HTML-returning path consistent
    /// with `fetch_and_extract`'s rescue behavior.
    pub async fn fetch_raw(&self, url: &str) -> Result<String, WaError> {
        self.client
            .fetch_smart(url)
            .await
            .map_err(|e| WaError::fetch(url, format!("{}", e)))
            .map(|r| r.html)
    }

    /// Fetch a URL and extract content from the same HTML, in a single HTTP
    /// request.
    ///
    /// Delegates to `FetchClient::fetch_smart` (Reddit rewrite + Akamai
    /// cookie warmup + SSRF guard + transient retry), then runs
    /// `webclaw_core::extract_with_options` on the returned HTML. Returns
    /// both the raw HTML (for callers that need the full document, e.g.
    /// crawl link discovery) and the extraction result.
    ///
    /// Unlike `fetch_and_extract`, this does **not** run webclaw's PDF,
    /// LinkedIn, or document rescue paths — those targets carry no HTML
    /// links and aren't crawlable, so they're intentionally omitted here
    /// to keep a single request and expose the raw HTML.
    pub async fn fetch_html_and_extract(
        &self,
        url: &str,
        options: &ExtractionOptions,
    ) -> Result<(String, ExtractionResult), WaError> {
        let fetched = self
            .client
            .fetch_smart(url)
            .await
            .map_err(|e| WaError::fetch(url, format!("{}", e)))?;
        let html = fetched.html;
        let extraction = extract_with_options(&html, Some(url), options)
            .map_err(|e| WaError::fetch(url, format!("{}", e)))?;
        Ok((html, extraction))
    }

    /// Discover API endpoints embedded in a page and its JavaScript bundles.
    ///
    /// Fetches the page HTML, collects `<script src>` URLs, fetches each
    /// bundle, then runs webclaw-core's endpoint surface discovery over the
    /// combined text. Returns relative paths, absolute URLs, GraphQL ops,
    /// and WebSocket endpoints.
    pub async fn extract_endpoints(&self, url: &str) -> Result<EndpointReport, WaError> {
        let html = self.fetch_raw(url).await?;
        let script_urls = webclaw_core::endpoints::script_srcs(&html, url);

        let mut bundles = Vec::with_capacity(script_urls.len());
        for script_url in script_urls {
            match self.fetch_raw(&script_url).await {
                Ok(text) => bundles.push((script_url, text)),
                Err(e) => {
                    tracing::warn!("failed to fetch script bundle {}: {}", script_url, e);
                }
            }
        }

        Ok(webclaw_core::endpoints::extract_endpoints(&html, url, &bundles))
    }

    /// Fetch and extract multiple URLs concurrently.
    ///
    /// Uses `FetchClient::fetch_and_extract_batch_with_options()` for
    /// connection reuse and bounded concurrency.
    pub async fn fetch_batch(
        &self,
        urls: &[&str],
        concurrency: usize,
        options: &ExtractionOptions,
    ) -> Vec<BatchExtractResult> {
        let results = self
            .client
            .fetch_and_extract_batch_with_options(urls, concurrency, options)
            .await;

        results.into_iter().map(BatchExtractResult::from).collect()
    }
}

/// Result of a batch extract operation — one entry per URL.
pub struct BatchExtractResult {
    pub url: String,
    pub result: Result<ExtractionResult, WaError>,
}

impl From<webclaw_fetch::BatchExtractResult> for BatchExtractResult {
    fn from(r: webclaw_fetch::BatchExtractResult) -> Self {
        let url = r.url.clone();
        BatchExtractResult {
            url: r.url,
            result: r
                .result
                .map_err(|e| WaError::fetch(url, format!("{}", e))),
        }
    }
}
