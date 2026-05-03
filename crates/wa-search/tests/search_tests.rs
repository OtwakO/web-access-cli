use wa_search::SearXNGClient;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---- T14: build_search_url -------------------------------------------------

#[tokio::test]
async fn build_search_url() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    // Start a mock that will catch the request so we can inspect the URL
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "rust async"))
        .and(query_param("format", "json"))
        .and(query_param("categories", "general"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({ "results": [] }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let results = client.search("rust async", 10).await.unwrap();
    assert!(results.is_empty());
}

// ---- T15: build_search_url_with_spaces -------------------------------------

#[tokio::test]
async fn build_search_url_with_spaces() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "hello world"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({ "results": [] }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let results = client.search("hello world", 10).await.unwrap();
    assert!(results.is_empty());
}

// ---- T16: parse_success_response --------------------------------------------

#[tokio::test]
async fn parse_success_response() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({
                "results": [
                    {
                        "title": "Rust Programming Language",
                        "url": "https://www.rust-lang.org/",
                        "content": "A language empowering everyone to build reliable and efficient software."
                    }
                ]
            }),
        ))
        .mount(&server)
        .await;

    let results = client.search("rust", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://www.rust-lang.org/");
    assert!(results[0].snippet.contains("empowering everyone"));
}

// ---- T17: parse_empty_results ----------------------------------------------

#[tokio::test]
async fn parse_empty_results() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({ "results": [] }),
        ))
        .mount(&server)
        .await;

    let results = client.search("xyznonexistent", 10).await.unwrap();
    assert!(results.is_empty());
}

// ---- T18: parse_missing_content_field --------------------------------------

#[tokio::test]
async fn parse_missing_content_field() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({
                "results": [
                    {
                        "title": "A Page",
                        "url": "https://example.com",
                        "snippet": "This is a snippet."
                    }
                ]
            }),
        ))
        .mount(&server)
        .await;

    let results = client.search("test", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].snippet, "");
}

// ---- T19: parse_malformed_json ---------------------------------------------

#[tokio::test]
async fn parse_malformed_json() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json {{{"))
        .mount(&server)
        .await;

    let err = client.search("test", 10).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("invalid JSON") || msg.contains("search error"));
}

// ---- T20: http_error_status ------------------------------------------------

#[tokio::test]
async fn http_error_status() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client.search("test", 10).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("500") || msg.contains("search error"));
}

// ---- T21: connection_refused -----------------------------------------------

#[tokio::test]
async fn connection_refused() {
    // Use an unreachable port — no wiremock needed
    let client = SearXNGClient::new("http://127.0.0.1:1".into());
    let err = client.search("test", 10).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("connection") || msg.contains("search error"));
}

// ---- T22: test_bad_instance_url --------------------------------------------

#[tokio::test]
async fn test_bad_instance_url() {
    // An invalid URL should fail at the reqwest level
    let client = SearXNGClient::new("not-a-url".into());
    let err = client.search("test", 10).await.unwrap_err();
    assert!(format!("{}", err).contains("search error"));
}

// ---- T23: test_result_limit ------------------------------------------------

#[tokio::test]
async fn test_result_limit() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    let mut results_json = Vec::new();
    for i in 0..5 {
        results_json.push(serde_json::json!({
            "title": format!("Result {}", i),
            "url": format!("https://example.com/{}", i),
            "content": format!("Content {}", i)
        }));
    }

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({ "results": results_json }),
        ))
        .mount(&server)
        .await;

    let results = client.search("test", 3).await.unwrap();
    assert_eq!(results.len(), 3);
}

// ---- T24: test_empty_query -------------------------------------------------

#[tokio::test]
async fn test_empty_query() {
    let client = SearXNGClient::new("http://localhost:8080".into());
    let err = client.search("", 10).await.unwrap_err();
    assert!(format!("{}", err).contains("empty query"));
}

// ---- T25: test_duplicate_urls ----------------------------------------------

#[tokio::test]
async fn test_duplicate_urls() {
    let server = MockServer::start().await;
    let client = SearXNGClient::new(server.uri());

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({
                "results": [
                    {
                        "title": "First",
                        "url": "https://example.com/same",
                        "content": "First occurrence"
                    },
                    {
                        "title": "Second",
                        "url": "https://example.com/same",
                        "content": "Duplicate — should be removed"
                    },
                    {
                        "title": "Third",
                        "url": "https://example.com/different",
                        "content": "Different page"
                    }
                ]
            }),
        ))
        .mount(&server)
        .await;

    let results = client.search("test", 10).await.unwrap();
    assert_eq!(results.len(), 2, "duplicate URL should be removed");
    assert_eq!(results[0].url, "https://example.com/same");
    assert_eq!(results[1].url, "https://example.com/different");
}
