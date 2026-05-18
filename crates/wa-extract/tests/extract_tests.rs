use wa_extract::{BrowserProfile, ExtractionOptions, Extractor};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: create a simple HTML page with a heading and paragraph.
fn basic_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
  <article>
    <h1>Hello World</h1>
    <p>This is a test paragraph with some content.</p>
    <p>Second paragraph with more text to extract.</p>
  </article>
</body>
</html>"#
    .into()
}

/// Helper: create HTML with code blocks.
fn html_with_code() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>Code Example</title></head>
<body>
  <article>
    <h1>Rust Example</h1>
    <p>Here is some code:</p>
    <pre><code>fn main() {
    println!("hello");
}</code></pre>
  </article>
</body>
</html>"#
    .into()
}

/// Helper: create HTML with links.
fn html_with_links() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>Links Page</title></head>
<body>
  <article>
    <h1>Useful Links</h1>
    <p>Check out <a href="https://www.rust-lang.org">Rust</a> and
       <a href="https://crates.io">crates.io</a>.</p>
  </article>
</body>
</html>"#
    .into()
}

/// Helper: create a minimal Extractor pointing at a mock server.
fn test_extractor() -> Extractor {
    Extractor::new(BrowserProfile::Chrome, None, None, 5)
}

// ---- T26: extract_basic_html -----------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_basic_html() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(basic_html()))
        .mount(&server)
        .await;

    let url = format!("{}/test", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    // webclaw-core produces markdown from the article
    assert!(!result.content.markdown.is_empty());
    assert!(result.content.markdown.contains("Hello World"));
    assert!(result.content.markdown.contains("test paragraph"));
}

// ---- T27: extract_with_code_blocks -----------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_with_code_blocks() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html_with_code()))
        .mount(&server)
        .await;

    let url = format!("{}/code", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    assert!(result.content.markdown.contains("println"));
}

// ---- T28: extract_with_links -----------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_with_links() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html_with_links()))
        .mount(&server)
        .await;

    let url = format!("{}/links", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    // Links should appear in the links vec
    let has_rust = result
        .content
        .links
        .iter()
        .any(|l| l.href.contains("rust-lang.org"));
    let has_crates = result
        .content
        .links
        .iter()
        .any(|l| l.href.contains("crates.io"));
    assert!(has_rust, "should have rust-lang.org link");
    assert!(has_crates, "should have crates.io link");
}

// ---- T29: extract_with_metadata --------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_with_metadata() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(basic_html()))
        .mount(&server)
        .await;

    let url = format!("{}/test", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    assert_eq!(result.metadata.title, Some("Test Page".into()));
}

// ---- T30: extract_empty_body -----------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_empty_body() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "<html><head><title>Empty</title></head><body></body></html>",
        ))
        .mount(&server)
        .await;

    let url = format!("{}/empty", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    // webclaw-core's fallback strategies should produce something
    // (even if just metadata). The result exists.
    let _ = result.content.markdown;
}

// ---- T31: extract_404_page -------------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_404_page() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_body_string("<html><body>Not Found</body></html>"))
        .mount(&server)
        .await;

    let url = format!("{}/not-found", server.uri());
    // webclaw-fetch extracts whatever content it gets, even on 404 —
    // this is desirable for AI agents who should see error page text
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();
    assert!(result.content.markdown.contains("Not Found"));
    assert_eq!(result.metadata.url, Some(url));
}

// ---- T32: extract_timeout --------------------------------------------------

#[tokio::test]
async fn extract_timeout() {
    let server = MockServer::start().await;
    // Create an extractor with a very short timeout
    let extractor = Extractor::new(BrowserProfile::Chrome, None, None, 1);

    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("will be slow")
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&server)
        .await;

    let url = format!("{}/slow", server.uri());
    let err = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("fetch error") || msg.contains("timeout"));
}

// ---- T33: extract_include_selectors ----------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_include_selectors() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    let html = r#"<!DOCTYPE html>
<html><head><title>Selectors</title></head>
<body>
  <article><h1>Main Content</h1><p>Keep this.</p></article>
  <aside><p>Sidebar content to skip.</p></aside>
</body></html>"#;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let mut options = ExtractionOptions::default();
    options.include_selectors = vec!["article".into()];

    let url = format!("{}/include", server.uri());
    let result = extractor.fetch_and_extract(&url, &options).await.unwrap();
    assert!(result.content.markdown.contains("Keep this"));
}

// ---- T34: extract_exclude_selectors ----------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_exclude_selectors() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    let html = r#"<!DOCTYPE html>
<html><head><title>Exclude</title></head>
<body>
  <article>
    <h1>Main</h1>
    <p>Keep this content.</p>
    <nav><p>Nav to exclude.</p></nav>
  </article>
</body></html>"#;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let mut options = ExtractionOptions::default();
    options.exclude_selectors = vec!["nav".into()];

    let url = format!("{}/exclude", server.uri());
    let result = extractor.fetch_and_extract(&url, &options).await.unwrap();
    assert!(!result.content.markdown.contains("Nav to exclude"));
    assert!(result.content.markdown.contains("Keep this"));
}

// ---- T35: extract_only_main_content ----------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_only_main_content() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    let html = r#"<!DOCTYPE html>
<html><head><title>Main Only</title></head>
<body>
  <nav><p>Navigation</p></nav>
  <main><h1>The Main Event</h1><p>This is the primary content.</p></main>
  <footer><p>Footer</p></footer>
</body></html>"#;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let mut options = ExtractionOptions::default();
    options.only_main_content = true;

    let url = format!("{}/main", server.uri());
    let result = extractor.fetch_and_extract(&url, &options).await.unwrap();
    assert!(result.content.markdown.contains("The Main Event"));
    assert!(result.content.markdown.contains("primary content"));
}

// ---- T36: extract_raw_html_flag --------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_raw_html_flag() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(basic_html()))
        .mount(&server)
        .await;

    let mut options = ExtractionOptions::default();
    options.include_raw_html = true;

    let url = format!("{}/raw", server.uri());
    let result = extractor.fetch_and_extract(&url, &options).await.unwrap();
    assert!(result.content.raw_html.is_some());
}

// ---- T37: extract_llm_format -----------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_llm_format() {
    let server = MockServer::start().await;
    let extractor = test_extractor();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(basic_html()))
        .mount(&server)
        .await;

    let url = format!("{}/llm", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    // to_llm_text() produces token-optimized output
    let llm_text = wa_extract::to_llm_text(&result, Some(&url));
    assert!(!llm_text.is_empty());
    assert!(llm_text.contains("Hello World"));
    // Should have compact metadata header
    assert!(llm_text.contains("Test Page"));
}

// ---- T38: extract_invalid_url ----------------------------------------------

#[tokio::test]
async fn extract_invalid_url() {
    let extractor = test_extractor();
    let err = extractor
        .fetch_and_extract("not-a-valid-url", &ExtractionOptions::default())
        .await
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("fetch error") || msg.contains("invalid"));
}

// ---- T39: extract_browser_profiles -----------------------------------------

#[tokio::test]
async fn extract_browser_profiles() {
    // Verify that BrowserProfile conversion works and Extractor can be
    // constructed with each variant.
    for profile in [BrowserProfile::Chrome, BrowserProfile::Firefox, BrowserProfile::SafariIos, BrowserProfile::Random] {
        let extractor = Extractor::new(profile, None, None, 5);
        let _ = extractor; // just ensure construction succeeds
    }
}

// ---- T40: extract_with_cookies ---------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_with_cookies() {
    let server = MockServer::start().await;
    let extractor = Extractor::new(
        BrowserProfile::Chrome,
        None,
        Some(vec!["session=abc123".into(), "token=xyz789".into()]),
        5,
    );

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(basic_html()))
        .mount(&server)
        .await;

    let url = format!("{}/cookies", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();
    // Basic content check — cookie handling is internal to the client
    assert!(result.content.markdown.contains("Hello World"));
}

// ---- T41: extract_redirects ------------------------------------------------

#[tokio::test]
#[ignore = "webclaw v0.6.2+ blocks localhost (SSRF guard); requires non-localhost test server"]
async fn extract_redirects() {
    let server = MockServer::start().await;

    // Mount a redirect from /old to /new
    Mock::given(method("GET"))
        .and(path("/old"))
        .respond_with(
            ResponseTemplate::new(301)
                .append_header("Location", format!("{}/new", server.uri())),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/new"))
        .respond_with(ResponseTemplate::new(200).set_body_string(basic_html()))
        .mount(&server)
        .await;

    let extractor = test_extractor();
    let url = format!("{}/old", server.uri());
    let result = extractor
        .fetch_and_extract(&url, &ExtractionOptions::default())
        .await
        .unwrap();

    // After redirect, we should get the content from /new
    assert!(result.content.markdown.contains("Hello World"));
}
