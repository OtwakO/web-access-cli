//! XML sitemap parser and auto-discovery.
//!
//! Supports:
//!   - Direct sitemap URLs (`fetch_sitemap`)
//!   - Auto-discovery via `/robots.txt` + common sitemap paths (`discover`)
//!   - Sitemap index files (`<sitemapindex>`), recursively resolved
//!   - Gzipped sitemaps (detected by gzip magic bytes)

use std::collections::HashSet;
use std::io::Read;

use quick_xml::de::from_str;
use serde::Deserialize;

/// Common sitemap paths to probe when robots.txt has no `Sitemap:` directive.
const FALLBACK_SITEMAP_PATHS: &[&str] = &[
    "/sitemap.xml",
    "/sitemap_index.xml",
    "/sitemap-index.xml",
    "/sitemap1.xml",
    "/sitemaps.xml",
    "/sitemap/index.xml",
    "/wp-sitemap.xml",
    "/sitemap/sitemap-index.xml",
];

/// Maximum recursion depth for nested sitemap index files.
const MAX_RECURSION_DEPTH: usize = 5;

/// A single `<url>` entry in a sitemap.
#[derive(Debug, Deserialize)]
struct UrlEntry {
    #[serde(rename = "loc")]
    loc: String,
}

/// A `<urlset>` document (regular sitemap).
#[derive(Debug, Deserialize)]
struct UrlSet {
    #[serde(rename = "url")]
    urls: Vec<UrlEntry>,
}

/// A single `<sitemap>` entry in a sitemap index.
#[derive(Debug, Deserialize)]
struct SitemapEntry {
    #[serde(rename = "loc")]
    loc: String,
}

/// A `<sitemapindex>` document.
#[derive(Debug, Deserialize)]
struct SitemapIndex {
    #[serde(rename = "sitemap")]
    sitemaps: Vec<SitemapEntry>,
}

/// Fetch and parse a sitemap URL, returning all page URLs.
///
/// Supports regular sitemaps (`<urlset>`), sitemap index files
/// (`<sitemapindex>`), and gzipped bodies. For index files, recursively
/// fetches all child sitemaps up to `MAX_RECURSION_DEPTH`.
pub async fn fetch_sitemap(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<String>, wa_core::error::WaError> {
    fetch_sitemap_inner(client, url, 0).await
}

async fn fetch_sitemap_inner(
    client: &reqwest::Client,
    url: &str,
    depth: usize,
) -> Result<Vec<String>, wa_core::error::WaError> {
    if depth > MAX_RECURSION_DEPTH {
        tracing::warn!("sitemap recursion limit reached at {}", url);
        return Ok(Vec::new());
    }

    let body = fetch_body(client, url).await?;
    let xml = decode_body(&body, url).ok_or_else(|| wa_core::error::WaError::Fetch {
        url: url.into(),
        detail: "sitemap body could not be decoded".into(),
    })?;

    // Try sitemapindex first.
    if let Ok(index) = from_str::<SitemapIndex>(&xml) {
        let mut all = Vec::new();
        for entry in index.sitemaps {
            match Box::pin(fetch_sitemap_inner(client, &entry.loc, depth + 1)).await {
                Ok(urls) => all.extend(urls),
                Err(e) => {
                    tracing::warn!("failed to fetch child sitemap {}: {}", entry.loc, e);
                }
            }
        }
        return Ok(all);
    }

    // Regular urlset.
    let set: UrlSet = from_str(&xml).map_err(|e| wa_core::error::WaError::Fetch {
        url: url.into(),
        detail: format!("sitemap XML parse error: {e}"),
    })?;
    Ok(set.urls.into_iter().map(|u| u.loc).collect())
}

/// Discover sitemap URLs for a host.
///
/// 1. Fetch `/robots.txt` and parse `Sitemap:` directives.
/// 2. Probe common sitemap paths as fallback.
/// 3. Recursively resolve any sitemap indexes.
///
/// Returns an empty vec if no sitemaps are found. Never errors on missing
/// sitemaps — discovery is best-effort.
pub async fn discover(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<String>, wa_core::error::WaError> {
    let base = base_url.trim_end_matches('/');
    let mut sitemap_urls: Vec<String> = Vec::new();

    // Step 1: robots.txt
    let robots_url = format!("{base}/robots.txt");
    match fetch_body(client, &robots_url).await {
        Ok(body) => {
            let text = String::from_utf8_lossy(&body);
            let found = parse_robots_txt(&text);
            tracing::debug!(count = found.len(), "sitemap URLs from robots.txt");
            sitemap_urls.extend(found);
        }
        Err(e) => {
            tracing::debug!(error = %e, "failed to fetch robots.txt");
        }
    }

    // Step 2: common paths (skip duplicates already found via robots.txt)
    for path in FALLBACK_SITEMAP_PATHS {
        let candidate = format!("{base}{path}");
        if !sitemap_urls.iter().any(|u| u == &candidate) {
            sitemap_urls.push(candidate);
        }
    }

    // Step 3: fetch each candidate, accumulating page URLs.
    let mut seen: HashSet<String> = HashSet::new();
    let mut page_urls: Vec<String> = Vec::new();

    for sitemap_url in sitemap_urls {
        match fetch_sitemap(client, &sitemap_url).await {
            Ok(urls) => {
                for u in urls {
                    if seen.insert(u.clone()) {
                        page_urls.push(u);
                    }
                }
            }
            Err(e) => {
                tracing::debug!(url = %sitemap_url, error = %e, "sitemap discovery skipped");
            }
        }
    }

    tracing::debug!(total = page_urls.len(), "sitemap discovery complete");
    Ok(page_urls)
}

async fn fetch_body(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, wa_core::error::WaError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| wa_core::error::WaError::Fetch {
            url: url.into(),
            detail: format!("HTTP request failed: {e}"),
        })?;
    let status = resp.status();
    let body = resp.bytes().await.map_err(|e| wa_core::error::WaError::Fetch {
        url: url.into(),
        detail: format!("failed to read response body: {e}"),
    })?;
    if !status.is_success() {
        return Err(wa_core::error::WaError::Fetch {
            url: url.into(),
            detail: format!("HTTP {status}"),
        });
    }
    Ok(body.to_vec())
}

/// Decode a raw sitemap body into UTF-8 XML.
///
/// Sitemaps are commonly served gzipped with `Content-Type: application/gzip`
/// and no `Content-Encoding`, so the HTTP layer never inflates them. We detect
/// gzip magic bytes (`0x1f 0x8b`) and gunzip in-process; otherwise treat the
/// body as plain XML.
fn decode_body(body: &[u8], url: &str) -> Option<String> {
    if body.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = flate2::read::GzDecoder::new(body);
        let mut out = String::new();
        match decoder.read_to_string(&mut out) {
            Ok(_) => Some(out),
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "failed to gunzip sitemap body");
                None
            }
        }
    } else {
        Some(String::from_utf8_lossy(body).into_owned())
    }
}

/// Parse `Sitemap:` directives from robots.txt content.
fn parse_robots_txt(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // Sitemap directive is case-insensitive per spec.
            let rest = line.strip_prefix("Sitemap:")
                .or_else(|| line.strip_prefix("sitemap:"))?;
            let url = rest.trim();
            if url.is_empty() {
                None
            } else {
                Some(url.to_string())
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn parse_urlset() {
        let server = MockServer::start().await;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/page1</loc></url>
  <url><loc>https://example.com/page2</loc></url>
</urlset>"#;
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(xml))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let urls = fetch_sitemap(&client, &format!("{}/sitemap.xml", server.uri())).await.unwrap();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://example.com/page1");
        assert_eq!(urls[1], "https://example.com/page2");
    }

    #[tokio::test]
    async fn parse_sitemapindex() {
        let server = MockServer::start().await;
        let index_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap><loc>CHILD_URL</loc></sitemap>
</sitemapindex>"#;
        let child_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/child-page</loc></url>
</urlset>"#;

        let child_url = format!("{}/child.xml", server.uri());
        let index_xml = index_xml.replace("CHILD_URL", &child_url);

        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(index_xml))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/child.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(child_xml))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let urls = fetch_sitemap(&client, &format!("{}/sitemap.xml", server.uri())).await.unwrap();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/child-page");
    }

    #[test]
    fn parse_robots_txt_finds_sitemaps() {
        let robots = r#"User-agent: *
Disallow: /admin/
Sitemap: https://example.com/sitemap.xml
sitemap: https://example.com/sitemap-news.xml
"#;
        let urls = parse_robots_txt(robots);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://example.com/sitemap.xml");
        assert_eq!(urls[1], "https://example.com/sitemap-news.xml");
    }

    #[test]
    fn parse_robots_txt_ignores_empty_and_garbage() {
        let robots = r#"User-agent: *
Sitemap:
Allow: /
NotASitemap: https://example.com/foo.xml
"#;
        let urls = parse_robots_txt(robots);
        assert!(urls.is_empty());
    }
}
