use std::io;

/// All errors from the wa family of crates.
#[derive(Debug, thiserror::Error)]
pub enum WaError {
    /// Configuration loading or parsing failure.
    #[error("config error: {0}")]
    Config(String),

    /// SearXNG search failure (HTTP error, bad JSON, empty response, etc.).
    #[error("search error: {0}")]
    Search(String),

    /// Fetch / content extraction failure.
    #[error("fetch error for {url}: {detail}")]
    Fetch {
        url: String,
        detail: String,
    },

    /// Git clone or repository read failure.
    #[error("git error: {0}")]
    Git(String),

    /// Invalid URL (malformed, unsupported protocol, not a recognised host).
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    /// Rate limit response (HTTP 429 from SearXNG or upstream).
    #[error("rate limited: {0}")]
    RateLimit(String),

    /// The `git` binary was not found on the system PATH.
    #[error("git not found: ensure git is installed and on your PATH")]
    GitNotFound,

    /// I/O error (filesystem, pipe, etc.).
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
}

// Allow converting FetchError and ExtractError from webclaw crates.
// These are implemented in wa-extract where webclaw-fetch is a dependency,
// so we provide a `from_fetch_error` helper instead of blanket `From` impls.
impl WaError {
    /// Construct a Fetch variant from a URL and an error message.
    pub fn fetch(url: impl Into<String>, detail: impl Into<String>) -> Self {
        WaError::Fetch {
            url: url.into(),
            detail: detail.into(),
        }
    }
}
