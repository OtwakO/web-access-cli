// Typed configuration for supported search provider adapters.

use serde::{Deserialize, Serialize};

/// Supported web search providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchProvider {
    #[default]
    Searxng,
    Degoog,
}

impl std::str::FromStr for SearchProvider {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "searxng" => Ok(Self::Searxng),
            "degoog" => Ok(Self::Degoog),
            _ => Err(format!("unsupported search provider: {value}")),
        }
    }
}

/// Search provider selection and provider-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SearchConfig {
    #[serde(default)]
    pub provider: SearchProvider,
    #[serde(default)]
    pub searxng: SearxngConfig,
    #[serde(default)]
    pub degoog: DegoogConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearxngConfig {
    #[serde(default = "default_searxng_url")]
    pub url: String,
}

impl Default for SearxngConfig {
    fn default() -> Self {
        Self {
            url: default_searxng_url(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DegoogConfig {
    #[serde(default = "default_degoog_url")]
    pub url: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

impl Default for DegoogConfig {
    fn default() -> Self {
        Self {
            url: default_degoog_url(),
            api_key: None,
        }
    }
}

fn default_searxng_url() -> String {
    "http://localhost:8080".into()
}

fn default_degoog_url() -> String {
    "http://localhost:4444".into()
}
