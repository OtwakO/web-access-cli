//! XML sitemap parser — supports both `<urlset>` and `<sitemapindex>` documents.

use quick_xml::de::from_reader;
use serde::Deserialize;

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
/// Supports both regular sitemaps (`<urlset>`) and sitemap index files
/// (`<sitemapindex>`). For index files, recursively fetches all child
/// sitemaps.
pub async fn fetch_sitemap(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<String>, wa_core::error::WaError> {
    let body = fetch_body(client, url).await?;

    // Try sitemapindex first
    if let Ok(index) = from_reader::<_, SitemapIndex>(&body[..]) {
        let mut all = Vec::new();
        for entry in index.sitemaps {
            match Box::pin(fetch_sitemap(client, &entry.loc)).await {
                Ok(urls) => all.extend(urls),
                Err(e) => {
                    tracing::warn!("failed to fetch child sitemap {}: {}", entry.loc, e);
                }
            }
        }
        return Ok(all);
    }

    // Regular urlset
    let set: UrlSet = from_reader(&body[..])
        .map_err(|e| wa_core::error::WaError::Fetch {
            url: url.into(),
            detail: format!("sitemap XML parse error: {e}"),
        })?;
    Ok(set.urls.into_iter().map(|u| u.loc).collect())
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
}
