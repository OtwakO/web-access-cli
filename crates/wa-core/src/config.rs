use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Default SearXNG instance URL.
fn default_searxng_url() -> String {
    "http://localhost:8080".into()
}

/// Default fetch timeout in seconds.
fn default_fetch_timeout_secs() -> u64 {
    12
}

/// Default browser profile string.
fn default_browser_profile() -> String {
    "chrome".into()
}

/// Default max file size for git clone reading (100 KiB).
fn default_max_file_size() -> usize {
    102_400
}

/// Default max number of files to read from a git clone.
fn default_max_files() -> usize {
    100
}

/// Default browser endpoint URL for `wa browser`.
/// The target URL is appended to this base (URL-encoded).
fn default_browser_endpoint() -> String {
    "http://localhost:8000/html?url=".into()
}

/// Default number of retries for transient network failures.
fn default_retries() -> u32 {
    3
}

/// Default delay between retries in milliseconds.
fn default_retry_delay_ms() -> u64 {
    500
}

/// Full application configuration.
///
/// Loaded in layered precedence:
///   1. Hard-coded defaults (above)
///   2. Config file at `~/.config/wa/config.toml` (auto-discovered)
///      or `--config FILE` (explicit, replaces auto path)
///   3. Environment variables (`WA_SEARXNG_URL`, …)
///   4. CLI flags (highest precedence — applied by the CLI crate)
///
/// The CLI crate resolves CLI flags against the effective config from
/// layers 1-3, so a CLI flag always wins even when an env var is set.
///
/// NOTE: `serde` default functions must appear **before** the struct
/// because `#[serde(default = "…")]` references them at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// SearXNG instance URL (used by `wa search` only).
    #[serde(default = "default_searxng_url")]
    pub searxng_url: String,

    /// HTTP fetch timeout in seconds.
    #[serde(default = "default_fetch_timeout_secs")]
    pub fetch_timeout_secs: u64,

    /// Browser profile for TLS fingerprinting: `chrome`, `firefox`,
    /// `safari-ios`, or `random`.
    #[serde(default = "default_browser_profile")]
    pub browser_profile: String,

    /// Optional SOCKS / HTTP proxy URL.
    #[serde(default)]
    pub proxy: Option<String>,

    /// Maximum file size (bytes) to read from a cloned repo.
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,

    /// Maximum number of text files to read from a cloned repo.
    #[serde(default = "default_max_files")]
    pub max_files: usize,

    /// Browser-backed rendering endpoint for `wa browser`
    /// (e.g. `http://localhost:8000`).
    #[serde(default = "default_browser_endpoint")]
    pub browser_endpoint: String,

    /// Number of retries for transient failures (connection refused,
    /// DNS failure, timeout, HTTP 429, HTTP 503).
    #[serde(default = "default_retries")]
    pub retries: u32,

    /// Base delay between retries in milliseconds (exponential backoff
    /// with ±25 % jitter).
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,

    /// Ordered list of URL rewrite rules. Applied before every HTTP
    /// fetch; first match wins. If no rule matches, the original URL
    /// is used unchanged.
    #[serde(default)]
    pub url_rewrites: Vec<crate::url_rewrite::UrlRewriteRule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            searxng_url: default_searxng_url(),
            fetch_timeout_secs: default_fetch_timeout_secs(),
            browser_profile: default_browser_profile(),
            proxy: None,
            max_file_size: default_max_file_size(),
            max_files: default_max_files(),
            browser_endpoint: default_browser_endpoint(),
            retries: default_retries(),
            retry_delay_ms: default_retry_delay_ms(),
            url_rewrites: Vec::new(),
        }
    }
}

impl Config {
    /// Build a `Config` by layering:
    ///   1. `Config::default()`
    ///   2. Config file — `explicit_path` if given, otherwise
    ///      auto-discovered `~/.config/wa/config.toml` (if it exists)
    ///   3. `WA_*` environment variables
    ///
    /// If `explicit_path` is given and the file does not exist, this
    /// returns an error. If no explicit path is given and the
    /// auto-discovered file does not exist, it silently proceeds with
    /// defaults + env.
    ///
    /// CLI overrides are applied separately by the CLI crate (highest
    /// precedence) — they are not handled here.
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, crate::error::WaError> {
        let mut config = Self::default();

        // Layer 2: config file
        let file_path = match explicit_path {
            Some(p) => {
                if !p.exists() {
                    return Err(crate::error::WaError::Config(format!(
                        "config file not found: {}",
                        p.display()
                    )));
                }
                Some(p.to_path_buf())
            }
            None => Self::default_config_path().filter(|p| p.exists()),
        };

        if let Some(ref path) = file_path {
            let contents = std::fs::read_to_string(path).map_err(|e| {
                crate::error::WaError::Config(format!(
                    "failed to read config file {}: {}",
                    path.display(),
                    e
                ))
            })?;
            let file_cfg: Self = toml::from_str(&contents).map_err(|e| {
                crate::error::WaError::Config(format!(
                    "invalid TOML in {}: {}",
                    path.display(),
                    e
                ))
            })?;
            config.overlay_file(file_cfg);
        }

        // Layer 3: environment variables
        config.apply_env_overrides();

        Ok(config)
    }

    /// Auto-discovered config path.
    ///
    /// Resolves `$XDG_CONFIG_HOME/wa/config.toml`, falling back to
    /// `$HOME/.config/wa/config.toml`.
    pub fn default_config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let base = dirs::home_dir()?;
            Some(base.join(".web-access").join("config.toml"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let base = std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".config"))
                })
                .or_else(|| {
                    dirs::config_dir()
                })?;
            Some(base.join("wa").join("config.toml"))
        }
    }

    /// Resolve the effective config file path: explicit argument wins,
    /// otherwise auto-discovered.
    pub fn config_file_path(explicit: Option<&Path>) -> Option<PathBuf> {
        explicit
            .map(|p| p.to_path_buf())
            .or_else(Self::default_config_path)
    }

    /// Scaffold a config file at `path` (or the default location if
    /// `None`) with commented-out defaults. Returns the path written.
    ///
    /// Errors if the file already exists — use this for first-time
    /// setup only.
    pub fn init_config_file(path: Option<&Path>) -> Result<PathBuf, crate::error::WaError> {
        let target = match path {
            Some(p) => p.to_path_buf(),
            None => Self::default_config_path().ok_or_else(|| {
                crate::error::WaError::Config(
                    "could not determine config directory".into(),
                )
            })?,
        };

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::WaError::Config(format!(
                    "failed to create config directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        if target.exists() {
            return Err(crate::error::WaError::Config(format!(
                "config file already exists: {}",
                target.display()
            )));
        }

        std::fs::write(&target, CONFIG_TEMPLATE).map_err(|e| {
            crate::error::WaError::Config(format!(
                "failed to write config file {}: {}",
                target.display(),
                e
            ))
        })?;

        Ok(target)
    }

    /// Overlay values from `file_cfg` onto `self`. Every field is
    /// unconditionally copied — this is a simple file-over-defaults
    /// merge, not a smart merge.
    fn overlay_file(&mut self, file_cfg: Self) {
        self.searxng_url = file_cfg.searxng_url;
        self.fetch_timeout_secs = file_cfg.fetch_timeout_secs;
        self.browser_profile = file_cfg.browser_profile;
        self.proxy = file_cfg.proxy;
        self.max_file_size = file_cfg.max_file_size;
        self.max_files = file_cfg.max_files;
        self.browser_endpoint = file_cfg.browser_endpoint;
        self.retries = file_cfg.retries;
        self.retry_delay_ms = file_cfg.retry_delay_ms;
        self.url_rewrites = file_cfg.url_rewrites;
    }

    /// Override fields from `WA_*` environment variables.
    /// These override both defaults and config file values.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("WA_SEARXNG_URL") {
            self.searxng_url = v;
        }
        if let Ok(v) = std::env::var("WA_BROWSER_PROFILE") {
            self.browser_profile = v;
        }
        if let Ok(v) = std::env::var("WA_PROXY") {
            self.proxy = if v.is_empty() { None } else { Some(v) };
        }
        if let Ok(v) = std::env::var("WA_BROWSER_ENDPOINT") {
            self.browser_endpoint = v;
        }
        if let Ok(v) = std::env::var("WA_RETRIES") {
            if let Ok(n) = v.parse() {
                self.retries = n;
            }
        }
    }
}

/// Config file template scaffolded by `wa config init`.
const CONFIG_TEMPLATE: &str = r##"# wa configuration
# Uncomment and edit any of the settings below.
# Priority: CLI flags > environment variables > this file > defaults

# SearXNG instance URL (used by `wa search`)
# searxng_url = "http://localhost:8080"

# Browser profile for TLS fingerprinting: chrome, firefox, safari-ios, random
# browser_profile = "chrome"

# SOCKS/HTTP proxy URL
# proxy = "socks5://127.0.0.1:9050"

# Fetch timeout in seconds
# fetch_timeout_secs = 12

# Number of retries for transient failures
# retries = 3

# Base delay between retries in milliseconds
# retry_delay_ms = 500

# Browser rendering endpoint for `wa browser` (target URL is appended)
# browser_endpoint = "http://localhost:8000/html?url="

# Max file size (bytes) when cloning repos
# max_file_size = 102400

# Max text files to read from a cloned repo
# max_files = 100

# URL rewrite rules — applied before every HTTP fetch.
# First match wins. Use Rust regex syntax; $1, $2… are capture groups.
#
# [[url_rewrites]]
# match_regex = '^https?://www\\.reddit\\.com/(.*)$'
# replace = 'https://old.reddit.com/$1'
#
# [[url_rewrites]]
# match_regex = '^https?://(www\\.)?medium\\.com/(.*)$'
# replace = 'https://scribe.rip/$2'
#
# [[url_rewrites]]
# match_regex = '^https?://twitter\\.com/'
# replace = 'https://nitter.net/'
"##;
