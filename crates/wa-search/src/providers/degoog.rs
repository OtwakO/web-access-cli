// Native Degoog HTTP adapter and response translation.

use super::{
    build_http_client, request_error, response_body, ProviderSearchResponse, UpstreamFailure,
};
use wa_core::error::WaError;
use wa_core::types::SearchResult;

pub(crate) struct DegoogProvider {
    instance_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl DegoogProvider {
    pub(crate) fn new(instance_url: String, api_key: Option<String>) -> Self {
        Self {
            instance_url: instance_url.trim_end_matches('/').to_owned(),
            api_key: api_key.filter(|value| !value.is_empty()),
            client: build_http_client(),
        }
    }

    pub(crate) async fn search_once(&self, query: &str) -> Result<ProviderSearchResponse, WaError> {
        let body = self.request(query).await?;
        let parsed: DegoogResponse = serde_json::from_str(&body)
            .map_err(|error| WaError::Search(format!("invalid JSON from Degoog: {error}")))?;

        Ok(ProviderSearchResponse {
            results: parsed
                .results
                .into_iter()
                .map(|result| SearchResult {
                    title: result.title,
                    url: result.url,
                    snippet: result.snippet,
                    img_src: result.image_url.filter(|value| !value.is_empty()),
                })
                .collect(),
            upstream_failures: parsed
                .engine_timings
                .into_iter()
                .filter_map(|timing| {
                    let status = timing.status?;
                    if status.eq_ignore_ascii_case("ok") {
                        return None;
                    }
                    Some(UpstreamFailure {
                        name: timing.name,
                        reason: timing
                            .error_reason
                            .filter(|reason| !reason.trim().is_empty())
                            .unwrap_or(status),
                    })
                })
                .collect(),
        })
    }

    pub(crate) async fn search_raw(&self, query: &str) -> Result<String, WaError> {
        self.request(query).await
    }

    async fn request(&self, query: &str) -> Result<String, WaError> {
        let mut request = self
            .client
            .get(format!("{}/api/search", self.instance_url))
            .query(&[("q", query), ("type", "web"), ("page", "1")]);
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|error| request_error(error, "Degoog"))?;
        response_body(response, "Degoog").await
    }
}

#[derive(serde::Deserialize)]
struct DegoogResponse {
    #[serde(default)]
    results: Vec<DegoogResult>,
    #[serde(default, rename = "engineTimings")]
    engine_timings: Vec<DegoogEngineTiming>,
}

#[derive(serde::Deserialize)]
struct DegoogResult {
    title: String,
    url: String,
    #[serde(default)]
    snippet: String,
    #[serde(default, rename = "imageUrl")]
    image_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct DegoogEngineTiming {
    name: String,
    status: Option<String>,
    #[serde(rename = "errorReason")]
    error_reason: Option<String>,
}
