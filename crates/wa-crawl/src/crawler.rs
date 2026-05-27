//! BFS crawler with semaphore-limited concurrent fetching.
//!
//! A single coordinator task manages the BFS queue and visits set.
//! Each URL is fetched in a spawned Tokio task, with a semaphore
//! limiting the number of in-flight requests.

use crate::{
    CrawlOptions, CrawlResult, CrawlSource, link_extract,
};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use url::Url;
use wa_core::url_rewrite::UrlRewriter;
use wa_extract::{Extractor, ExtractionOptions};

/// Crawler instance — configured once, reused per crawl call.
pub struct Crawler {
    extractor: Extractor,
    rewriter: UrlRewriter,
    options: CrawlOptions,
}

impl Crawler {
    /// Create a new crawler.
    pub fn new(extractor: Extractor, rewriter: UrlRewriter, options: CrawlOptions) -> Self {
        Self {
            extractor,
            rewriter,
            options,
        }
    }

    /// Run a crawl starting from `seed`.
    ///
    /// Returns all successfully extracted pages in discovery order.
    /// Pages that fail to fetch are silently skipped.
    pub async fn crawl(&self, seed: &str) -> Result<Vec<CrawlResult>, wa_core::error::WaError> {
        let seed_url = Url::parse(seed).map_err(|e| {
            wa_core::error::WaError::InvalidUrl(format!("invalid seed URL: {e}"))
        })?;
        let seed_host = seed_url
            .host_str()
            .ok_or_else(|| wa_core::error::WaError::InvalidUrl("seed URL has no host".into()))?
            .to_string();

        // Gather initial URLs
        let mut queue: VecDeque<(String, usize, CrawlSource)> = VecDeque::new();

        if self.options.sitemap {
            let client = reqwest::Client::new();
            match crate::sitemap::fetch_sitemap(&client, seed).await {
                Ok(urls) => {
                    for u in urls {
                        queue.push_back((u, 0, CrawlSource::Sitemap));
                    }
                }
                Err(e) => {
                    return Err(wa_core::error::WaError::Fetch {
                        url: seed.into(),
                        detail: format!("sitemap fetch failed: {e}"),
                    });
                }
            }
        } else {
            queue.push_back((seed.into(), 0, CrawlSource::Seed));
        }

        if queue.is_empty() {
            return Ok(Vec::new());
        }

        let visited: Arc<Mutex<HashSet<String>>> = Arc::default();
        let results: Arc<Mutex<Vec<CrawlResult>>> = Arc::default();
        let semaphore = Arc::new(Semaphore::new(self.options.concurrency.max(1)));

        // Seed the visited set with initial URLs
        {
            let mut v = visited.lock().await;
            for (url, _, _) in &queue {
                if let Some(norm) = link_extract::normalize_url(url) {
                    v.insert(norm);
                }
            }
        }

        let mut handles: Vec<tokio::task::JoinHandle<Vec<(String, usize)>>> = Vec::new();

        while !queue.is_empty() || !handles.is_empty() {
            // Spawn tasks for queued items up to concurrency limit
            while !queue.is_empty() {
                let permit = match semaphore.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => break,
                };

                let (url, depth, source) = queue.pop_front().unwrap();
                let worker = CrawlWorker {
                    extractor: self.extractor.clone(),
                    rewriter: self.rewriter.clone(),
                    options: self.options.clone(),
                    seed_host: seed_host.clone(),
                    visited: visited.clone(),
                    results: results.clone(),
                };

                let handle = tokio::spawn(async move {
                    let child_urls = worker.process(url, depth, source).await;
                    drop(permit);
                    child_urls
                });
                handles.push(handle);
            }

            // Wait for at least one task to complete and collect child URLs
            if !handles.is_empty() {
                let (completed, _idx, remaining) = futures::future::select_all(handles).await;
                handles = remaining;
                if let Ok(child_urls) = completed {
                    for (url, depth) in child_urls {
                        queue.push_back((url, depth, CrawlSource::Link));
                    }
                }
            }
        }

        let mutex = Arc::try_unwrap(results)
            .map_err(|_| wa_core::error::WaError::Config("results still locked".into()))?;
        let mut guard = mutex.try_lock()
            .map_err(|_| wa_core::error::WaError::Config("mutex still held".into()))?;
        let results_vec = std::mem::take(&mut *guard);
        Ok(results_vec)
    }
}

/// Per-page worker — fetches one URL and returns child URLs to enqueue.
struct CrawlWorker {
    extractor: Extractor,
    rewriter: UrlRewriter,
    options: CrawlOptions,
    seed_host: String,
    visited: Arc<Mutex<HashSet<String>>>,
    results: Arc<Mutex<Vec<CrawlResult>>>,
}

impl CrawlWorker {
    /// Process a single URL. Returns child URLs to enqueue.
    async fn process(
        &self,
        url: String,
        depth: usize,
        source: CrawlSource,
    ) -> Vec<(String, usize)> {
        // Apply URL rewrite
        let fetch_url = self.rewriter.apply(&url).unwrap_or_else(|| url.clone());
        let was_rewritten = fetch_url != url;

        // Fetch raw HTML first (needed for link extraction)
        let raw_html = match self.extractor.fetch_raw(&fetch_url).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("fetch raw failed for {}: {}", url, e);
                return Vec::new();
            }
        };

        // Extract content
        let extraction = match self
            .extractor
            .fetch_and_extract(&fetch_url, &ExtractionOptions::default())
            .await
        {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("extract failed for {}: {}", url, e);
                return Vec::new();
            }
        };

        // Store result
        let result = CrawlResult {
            url: url.clone(),
            fetched_url: if was_rewritten { Some(fetch_url.clone()) } else { None },
            depth,
            source,
            extraction,
        };
        self.results.lock().await.push(result);

        // Extract and filter child links
        if depth >= self.options.depth {
            return Vec::new();
        }

        let links = link_extract::extract_links(&raw_html, &fetch_url);
        let mut children = Vec::new();

        for link in links {
            let Some(norm) = link_extract::normalize_url(&link) else { continue };
            let Ok(parsed) = Url::parse(&norm) else { continue };
            if !link_extract::passes_filters(&parsed, &self.seed_host, &self.options.allow, &self.options.deny) {
                continue;
            }

            let mut visited = self.visited.lock().await;
            if visited.insert(norm.clone()) {
                children.push((norm, depth + 1));
            }
        }

        children
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wa_extract::BrowserProfile;

    fn test_crawler() -> Crawler {
        let extractor = Extractor::new(BrowserProfile::Chrome, None, None, 10);
        let rewriter = UrlRewriter::new(&[]).unwrap();
        Crawler::new(extractor, rewriter, CrawlOptions::default())
    }

    #[tokio::test]
    async fn crawl_invalid_seed_url() {
        let crawler = test_crawler();
        let err = crawler.crawl("not-a-url").await.unwrap_err().to_string();
        assert!(err.contains("invalid seed URL"));
    }

    #[tokio::test]
    async fn crawl_empty_sitemap_returns_empty() {
        // This would need wiremock for a real test; for now just verify structure
        let extractor = Extractor::new(BrowserProfile::Chrome, None, None, 10);
        let rewriter = UrlRewriter::new(&[]).unwrap();
        let opts = CrawlOptions {
            sitemap: true,
            ..Default::default()
        };
        let _crawler = Crawler::new(extractor, rewriter, opts);
        // Cannot test without mock server, but the code path compiles
    }
}
