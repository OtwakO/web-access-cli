// Internal dispatch and shared response shape for search provider adapters.

mod degoog;
mod searxng;

use wa_core::error::WaError;
use wa_core::types::SearchResult;

pub(crate) use degoog::DegoogProvider;
pub(crate) use searxng::SearxngProvider;

pub(crate) struct ProviderSearchResponse {
    pub results: Vec<SearchResult>,
    pub upstream_failures: Vec<UpstreamFailure>,
}

pub(crate) struct UpstreamFailure {
    pub name: String,
    pub reason: String,
}

pub(crate) enum Provider {
    Searxng(SearxngProvider),
    Degoog(DegoogProvider),
}

impl Provider {
    pub(crate) async fn search_once(&self, query: &str) -> Result<ProviderSearchResponse, WaError> {
        match self {
            Self::Searxng(provider) => provider.search_once(query).await,
            Self::Degoog(provider) => provider.search_once(query).await,
        }
    }

    pub(crate) async fn search_raw(&self, query: &str) -> Result<String, WaError> {
        match self {
            Self::Searxng(provider) => provider.search_raw(query).await,
            Self::Degoog(provider) => provider.search_raw(query).await,
        }
    }
}

pub(crate) fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("wa/0.1 search-client")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build reqwest client")
}

pub(crate) async fn response_body(
    response: reqwest::Response,
    provider_name: &str,
) -> Result<String, WaError> {
    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(WaError::RateLimit(format!("{provider_name} rate limited")));
    }
    if !status.is_success() {
        return Err(WaError::Search(format!(
            "HTTP {} from {provider_name}",
            status.as_u16()
        )));
    }

    response.text().await.map_err(|error| {
        WaError::Search(format!("failed to read {provider_name} response: {error}"))
    })
}

pub(crate) fn request_error(error: reqwest::Error, provider_name: &str) -> WaError {
    if error.is_timeout() {
        WaError::Search(format!("{provider_name} request timed out"))
    } else if error.is_connect() {
        WaError::Search(format!("{provider_name} connection failed: {error}"))
    } else {
        WaError::Search(format!("{provider_name} request failed: {error}"))
    }
}
