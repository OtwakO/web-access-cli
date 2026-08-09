// Native SearXNG HTTP adapter and response translation.

use super::{
    build_http_client, request_error, response_body, ProviderSearchResponse, UpstreamFailure,
};
use wa_core::error::WaError;
use wa_core::types::SearchResult;

pub(crate) struct SearxngProvider {
    instance_url: String,
    client: reqwest::Client,
}

impl SearxngProvider {
    pub(crate) fn new(instance_url: String) -> Self {
        Self {
            instance_url: instance_url.trim_end_matches('/').to_owned(),
            client: build_http_client(),
        }
    }

    fn search_url(&self, query: &str) -> String {
        format!(
            "{}/search?q={}&format=json&categories=general&safesearch=0",
            self.instance_url,
            encode_query(query)
        )
    }

    pub(crate) async fn search_once(&self, query: &str) -> Result<ProviderSearchResponse, WaError> {
        let body = self.request(query).await?;
        let parsed: SearxngResponse = serde_json::from_str(&body)
            .map_err(|error| WaError::Search(format!("invalid JSON from SearXNG: {error}")))?;

        Ok(ProviderSearchResponse {
            results: parsed
                .results
                .into_iter()
                .map(|result| SearchResult {
                    title: result.title,
                    url: result.url,
                    snippet: result.content.unwrap_or_default(),
                    img_src: if result.category == "images" {
                        result.img_src.filter(|value| !value.is_empty())
                    } else {
                        None
                    },
                })
                .collect(),
            upstream_failures: parsed
                .unresponsive_engines
                .into_iter()
                .map(|(name, reason)| UpstreamFailure { name, reason })
                .collect(),
        })
    }

    pub(crate) async fn search_raw(&self, query: &str) -> Result<String, WaError> {
        self.request(query).await
    }

    async fn request(&self, query: &str) -> Result<String, WaError> {
        let response = self
            .client
            .get(self.search_url(query))
            .send()
            .await
            .map_err(|error| request_error(error, "SearXNG"))?;
        response_body(response, "SearXNG").await
    }
}

fn default_category() -> String {
    "general".into()
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b' ' => encoded.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => {
                encoded.push('%');
                encoded.push_str(&format!("{byte:02X}"));
            }
        }
    }
    encoded
}

#[derive(serde::Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
    #[serde(default)]
    unresponsive_engines: Vec<(String, String)>,
}

#[derive(serde::Deserialize)]
struct SearxngResult {
    title: String,
    url: String,
    content: Option<String>,
    #[serde(rename = "snippet")]
    _snippet: Option<String>,
    #[serde(default = "default_category")]
    category: String,
    img_src: Option<String>,
}
