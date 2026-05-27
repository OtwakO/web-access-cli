//! wa-crawl — BFS and sitemap-based web crawler for AI agents.
//!
//! Crawls a single host starting from a seed URL, extracting content
//! from each page and following internal links up to a configurable
//! depth. Supports both BFS link discovery and XML sitemap parsing.

pub mod crawler;
pub use crawler::Crawler;
pub mod link_extract;
pub mod sitemap;

use serde::{Deserialize, Serialize};
use wa_core::url_rewrite::UrlRewriter;
use wa_extract::Extractor;

/// Configuration options for a crawl.
#[derive(Debug, Clone)]
pub struct CrawlOptions {
    /// Maximum BFS depth (0 = seed only).
    pub depth: usize,
    /// Number of concurrent fetch workers.
    pub concurrency: usize,
    /// Path substrings that URLs must contain (any match passes).
    /// Empty means no allow-list filtering.
    pub allow: Vec<String>,
    /// Regex patterns to reject URLs.
    pub deny: Vec<regex::Regex>,
    /// If true, treat seed URL as a sitemap instead of BFS seed.
    pub sitemap: bool,
}

impl Default for CrawlOptions {
    fn default() -> Self {
        Self {
            depth: 3,
            concurrency: 4,
            allow: Vec::new(),
            deny: Vec::new(),
            sitemap: false,
        }
    }
}

/// The source of a crawled URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrawlSource {
    /// The user-provided seed URL.
    Seed,
    /// Discovered via `<a href>` link on another page.
    Link,
    /// Listed in an XML sitemap.
    Sitemap,
}

/// A single page result from a crawl.
#[derive(Debug, Clone)]
pub struct CrawlResult {
    /// Original URL (before any rewrite).
    pub url: String,
    /// Rewritten URL actually fetched (None if no rewrite applied).
    pub fetched_url: Option<String>,
    /// Crawl depth (0 = seed/sitemap).
    pub depth: usize,
    /// How this URL was discovered.
    pub source: CrawlSource,
    /// Extracted content from the page.
    pub extraction: wa_extract::ExtractionResult,
}

/// Build an [`Extractor`] and optional [`UrlRewriter`] from a [`wa_core::config::Config`].
///
/// Convenience helper so `wa-cli` doesn't need to know the details of
/// constructing an extractor from config fields.
pub fn build_extractor_from_config(
    cfg: &wa_core::config::Config,
    browser: Option<String>,
    proxy: Option<String>,
    cookies: Option<Vec<String>>,
) -> Result<(Extractor, UrlRewriter), wa_core::error::WaError> {
    let profile_str = browser.unwrap_or_else(|| cfg.browser_profile.clone());
    let profile = wa_extract::BrowserProfile::try_from_str(&profile_str)
        .map_err(|e| wa_core::error::WaError::Config(format!("invalid browser profile: {e}")))?;
    let proxy_resolved = proxy.or_else(|| cfg.proxy.clone());
    let extractor = Extractor::new(profile, proxy_resolved, cookies, cfg.fetch_timeout_secs as u64);
    let rewriter = UrlRewriter::new(&cfg.url_rewrites)?;
    Ok((extractor, rewriter))
}
