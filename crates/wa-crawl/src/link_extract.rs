//! Link extraction from raw HTML.
//!
//! Uses `scraper` to parse HTML and extract all `<a href>` links,
//! resolving relative URLs against a base URL and filtering out
//! non-HTTP schemes.

use url::Url;

/// Extract all resolvable HTTP(S) links from `html`, resolving relative
/// URLs against `base_url`.
///
/// Filters out:
/// - `javascript:`, `mailto:`, `tel:`, `#` anchors
/// - non-http/https schemes
pub fn extract_links(html: &str, base_url: &str) -> Vec<String> {
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };

    let document = scraper::Html::parse_document(html);
    let selector = match scraper::Selector::parse("a[href]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    document
        .select(&selector)
        .filter_map(|el| el.value().attr("href"))
        .filter(|href| {
            !href.starts_with("javascript:")
                && !href.starts_with("mailto:")
                && !href.starts_with("tel:")
                && !href.starts_with('#')
        })
        .filter_map(|href| {
            // Empty href or whitespace-only → skip
            let trimmed = href.trim();
            if trimmed.is_empty() {
                return None;
            }
            base.join(trimmed).ok()
        })
        .filter(|url| url.scheme() == "http" || url.scheme() == "https")
        .map(|url| url.to_string())
        .collect()
}

/// Normalize a URL for deduplication:
/// - Strip fragment identifier
/// - Strip `utm_*` query parameters
/// - Remove trailing `/`
pub fn normalize_url(url_str: &str) -> Option<String> {
    let mut url = Url::parse(url_str).ok()?;
    url.set_fragment(None);

    // Strip utm_* params
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !k.starts_with("utm_"))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if pairs.is_empty() {
        url.set_query(None);
    } else {
        let _ = url.query_pairs_mut().clear().extend_pairs(&pairs);
    }

    let s = url.to_string();
    Some(s.trim_end_matches('/').to_string())
}

/// Check if a URL passes host, allow, and deny filters.
pub fn passes_filters(
    url: &Url,
    seed_host: &str,
    allow: &[String],
    deny: &[regex::Regex],
) -> bool {
    if url.host_str() != Some(seed_host) {
        return false;
    }
    for re in deny {
        if re.is_match(&url.to_string()) {
            return false;
        }
    }
    if !allow.is_empty() {
        let path = url.path();
        if !allow.iter().any(|sub| path.contains(sub)) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_links_basic() {
        let html = r#"<html><body>
            <a href="/page1">Page 1</a>
            <a href="https://other.com/page">External</a>
            <a href="page2.html">Relative</a>
        </body></html>"#;
        let links = extract_links(html, "https://example.com/");
        assert!(links.contains(&"https://example.com/page1".into()));
        assert!(links.contains(&"https://other.com/page".into()));
        assert!(links.contains(&"https://example.com/page2.html".into()));
    }

    #[test]
    fn extract_links_skips_javascript_mailto() {
        let html = concat!(
            "<a href=\"javascript:void(0)\">js</a>",
            "<a href=\"mailto:a@b.com\">mail</a>",
            "<a href=\"tel:123\">tel</a>",
            "<a href=\"#anchor\">anchor</a>",
            "<a href=\"https://example.com/valid\">valid</a>"
        );
        let links = extract_links(html, "https://example.com/");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0], "https://example.com/valid");
    }

    #[test]
    fn extract_links_resolves_relative() {
        let html = r#"<a href="../parent.html">up</a>
            <a href="./sibling.html">sibling</a>
            <a href="/absolute.html">absolute</a>"#;
        let links = extract_links(html, "https://example.com/dir/page.html");
        assert!(links.contains(&"https://example.com/parent.html".into()));
        assert!(links.contains(&"https://example.com/dir/sibling.html".into()));
        assert!(links.contains(&"https://example.com/absolute.html".into()));
    }

    #[test]
    fn extract_links_empty_href() {
        let html = r#"<a href="">empty</a>
            <a href="  ">whitespace</a>
            <a href="https://example.com/x">real</a>"#;
        let links = extract_links(html, "https://example.com/");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0], "https://example.com/x");
    }

    #[test]
    fn normalize_url_strips_fragment() {
        assert_eq!(
            normalize_url("https://example.com/page#section"),
            Some("https://example.com/page".into())
        );
    }

    #[test]
    fn normalize_url_strips_utm() {
        assert_eq!(
            normalize_url("https://example.com/?utm_source=x&page=2&utm_medium=y"),
            Some("https://example.com/?page=2".into())
        );
    }

    #[test]
    fn normalize_url_trims_slash() {
        assert_eq!(
            normalize_url("https://example.com/page/"),
            Some("https://example.com/page".into())
        );
    }

    #[test]
    fn normalize_url_empty_query_after_stripping() {
        assert_eq!(
            normalize_url("https://example.com/?utm_source=x"),
            Some("https://example.com".into())
        );
    }

    #[test]
    fn passes_filters_same_host() {
        let url = Url::parse("https://example.com/page").unwrap();
        assert!(passes_filters(&url, "example.com", &[], &[]));
    }

    #[test]
    fn passes_filters_different_host() {
        let url = Url::parse("https://other.com/page").unwrap();
        assert!(!passes_filters(&url, "example.com", &[], &[]));
    }

    #[test]
    fn passes_filters_deny_regex() {
        let url = Url::parse("https://example.com/admin/secret").unwrap();
        let deny = vec![regex::Regex::new(r"/admin/").unwrap()];
        assert!(!passes_filters(&url, "example.com", &[], &deny));
    }

    #[test]
    fn passes_filters_allow_list() {
        let url = Url::parse("https://example.com/docs/page").unwrap();
        let allow = vec!["docs".into(), "blog".into()];
        assert!(passes_filters(&url, "example.com", &allow, &[]));

        let url2 = Url::parse("https://example.com/other/page").unwrap();
        assert!(!passes_filters(&url2, "example.com", &allow, &[]));
    }

    #[test]
    fn passes_filters_empty_allow_is_pass() {
        let url = Url::parse("https://example.com/anything").unwrap();
        assert!(passes_filters(&url, "example.com", &[], &[]));
    }
}
