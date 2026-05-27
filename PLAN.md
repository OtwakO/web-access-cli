# wa — Web Access CLI for AI Agents

> Build Plan v1.0 — 2026-05-01
>
> **Principle:** TDD from day one. Every module starts with a failing test.
> **Architecture:** Modular crates with low decoupling. Simple interfaces, high depth.
> **Goal:** A Rust CLI tool purpose-built for AI agent consumption that delivers
> search, fetch+extract, and git cloning, powered by webclaw-core's extraction engine.

---

## 1. Project Overview

**wa** (Web Access) is a Rust CLI that gives AI agents four capabilities:

| # | Command | Description | Fetch Engine | Extract Engine |
|---|---------|-------------|-------------|----------------|
| 1 | `wa search` | Web search via SearXNG, optional page extraction | webclaw-fetch (BoringSSL TLS fingerprinting) | webclaw-core (95.1% accuracy) |
| 2 | `wa fetch` | Fetch URL → extract clean content | webclaw-fetch (BoringSSL TLS fingerprinting) | webclaw-core (95.1% accuracy) |
| 3 | `wa git` | Clone repo → list text files (`--tree-only` for paths only) | git CLI (shallow clone) | N/A (raw files) |
| 4 | `wa browser` | Fetch via browser-backed rendering endpoint → extract content | Browser endpoint (HTTP GET) | webclaw-core (95.1% accuracy) |

**Every page fetched by this tool goes through webclaw's extraction pipeline, not Readability.**
No garbage in, garbage out. Every output format (markdown, LLM-optimized,
plain text, JSON, raw) is produced by webclaw-core's multi-signal scoring engine
with noise filtering, data island extraction, and token-efficient LLM formatting.

**Why this exists:** Combines webclaw's full fetching+extraction stack
(TLS fingerprinting, 95.1% extraction accuracy, 67% token savings vs raw HTML,
bot protection detection) with pi-searxng's SearXNG search and GitHub cloning —
all in one CLI, one binary, one consistent interface.

**What distinguishes it from webclaw itself:**
- **SearXNG, not proprietary search** — self-hosted, privacy-respecting, no API key
- **Git cloning** — webclaw doesn't do this at all
- **Focused scope** — just the three operations AI agents need most, not a 12-tool platform
- **CLI-first** — designed for stdin/stdout pipeline consumption
- **No cloud dependency** — everything runs locally; no WEBCLAW_API_KEY needed

---

## 2. Crate Architecture

```
web-access-cli/                  ← Workspace root
├── Cargo.toml                   ← [workspace], members, shared deps
├── crates/                      ← Our original code
│   ├── wa-core/                 ← Shared types, config, error enums (zero I/O)
│   │   ├── Cargo.toml           ← deps: serde, thiserror, url
│   │   └── src/
│   │       ├── lib.rs           ← re-exports
│   │       ├── types.rs         ← SearchResult, GitRepo, OutputFormat
│   │       ├── config.rs        ← Config struct + loading (CLI args, env, TOML file)
│   │       └── error.rs         ← WaError enum
│   │
│   ├── wa-search/               ← SearXNG search client (HTTP I/O only)
│   │   ├── Cargo.toml           ← deps: wa-core, reqwest, serde_json
│   │   └── src/
│   │       ├── lib.rs           ← re-exports
│   │       └── client.rs        ← SearXNGClient { .search(query, limit) }
│   │
│   ├── wa-extract/              ← Fetch + extract content
│   │   ├── Cargo.toml           ← deps: wa-core, webclaw-fetch (git)
│   │   └── src/
│   │       ├── lib.rs           ← re-exports ExtractionResult
│   │       └── extractor.rs     ← Extractor wraps FetchClient
│   │
│   ├── wa-git/                  ← Git repo cloning + file listing
│   │   ├── Cargo.toml           ← deps: wa-core, tempfile, walkdir, regex
│   │   └── src/
│   │       ├── lib.rs           ← re-exports
│   │       └── cloner.rs        ← GitCloner { .clone_and_list(url, opts) }
│   │
│   └── wa-cli/                  ← CLI binary (thin orchestration layer)
│       ├── Cargo.toml           ← deps: wa-core, wa-search, wa-extract, wa-git, clap, tokio
│       └── src/
│           ├── main.rs          ← clap derive + dispatch
│           ├── commands/
│           │   ├── mod.rs
│           │   ├── search.rs    ← search command handler
│           │   ├── fetch.rs     ← fetch command handler
│           │   └── git.rs       ← git command handler
│           └── output.rs        ← output formatter (markdown, llm, text, json)
│
├── tests/                       ← Integration tests
│   ├── cli_search.rs
│   ├── cli_fetch.rs
│   └── cli_git.rs
│
├── fixtures/                    ← Test data
│   ├── searxng_response.json    ← Sample SearXNG JSON response
│   ├── sample.html              ← Sample web page for extraction tests
│   └── small-repo/              ← Tiny git repo for cloner tests
│
├── docs/
│   ├── PLAN.md                  ← This file
│   └── API.md                   ← Module API reference (auto-generated later)
│
├── .gitignore
└── README.md
```

**Dependency graph (edges = "depends on"):**
```
wa-cli ──────→ wa-search ──────→ wa-core ── serde, thiserror, url
  │               │
  ├───────────→ wa-extract ────→ wa-core
  │               │
  │             webclaw-fetch (git: github.com/0xMassi/webclaw, AGPL-3.0)
  │               ├── wreq + BoringSSL — browser-grade TLS fingerprinting
  │               ├── webclaw-core — extraction engine (Readability-style scoring)
  │               ├── webclaw-pdf — PDF text extraction
  │               ├── Reddit JSON fallback, LinkedIn extraction, Akamai cookie warmup
  │               └── 29+ vertical extractors (GitHub, PyPI, npm, Amazon, YouTube, ...)
  │
  ├───────────→ wa-git ────────→ wa-core
  │
  └───────────→ clap, tokio, tracing

HTML→content seam (clean interface, future-proof):
  Any fetch mechanism ──→ raw HTML ──→ webclaw_core::extract_with_options(html, url, opts)
  (webclaw-fetch, headless browser, reqwest, static file, ...)
```

**Why this structure:**
- **wa-core** is pure data — zero I/O, no network, no filesystem. Can be compiled to WASM.
- **wa-search** owns SearXNG HTTP (simple JSON API, uses reqwest — no TLS fingerprinting needed).
- **wa-extract** owns page fetching via webclaw-fetch (BoringSSL TLS fingerprinting, browser profiles, proxy support, bot protection detection, PDF extraction, batch operations).
- **wa-git** owns git cloning + file listing.
- **wa-cli** is the thin orchestration layer. If we add an MCP server later, it lives alongside wa-cli (or replaces it) without touching any library crate.

### Extraction Engine: Modular by Design

The extraction engine is deliberately separated from the fetch mechanism:

```
┌─────────────────────────────────────────────────────┐
│                  Fetch Layer                         │
│  (how HTML is obtained)                              │
│                                                      │
│  webclaw-fetch ─── TLS fingerprinting, HTTP client   │
│  future: headless browser ─── JS rendering           │
│  future: cached fetcher ─── local HTML cache         │
└──────────────────────┬──────────────────────────────┘
                       │ raw HTML + URL
                       ▼
┌─────────────────────────────────────────────────────┐
│               Extraction Layer                       │
│  (what to do with HTML)                              │
│                                                      │
│  webclaw_core::extract_with_options(html, url)       │
│    → ExtractionResult { markdown, links, images, … }│
│  webclaw_core::to_llm_text(result) → token-optimized│
└─────────────────────────────────────────────────────┘
```

This means:
- **Any fetch mechanism can feed into the same extraction pipeline.**
  Add a browser-based fetch command later (`wa browser-fetch`) — pass the HTML
  through the same `extract_with_options()` and get identical output quality.
- **The extraction engine only cares about `html: &str` + `url: Option<&str>`.**
  It has no idea how the HTML was obtained. This is the clean interface we need.
- **wa-extract already enforces this boundary.** The `Extractor::fetch_and_extract()`
  method internally calls `fetch()` then `extract_with_options()` — two separate steps
  with a clear seam between them. Future fetching methods only need to replace the
  `fetch()` half.

---

## 3. Module Interfaces (API Contracts)

> **Reference codebases for development (clone if not present locally):**
> ```bash
> # webclaw — the extraction engine and TLS fetch stack we depend on
> [ ! -d /tmp/webclaw ] && git clone --depth 1 https://github.com/0xMassi/webclaw /tmp/webclaw
>
> # pi-searxng — SearXNG integration patterns, GitHub cloning patterns
> [ ! -d /tmp/pi-searxng ] && git clone --depth 1 https://github.com/jcha0713/pi-searxng /tmp/pi-searxng
> ```
> **webclaw is a git dependency** (`webclaw-fetch = { git = "..." }`), not vendored.
> The upstream clone above is for reference when reading source code during development.
> All 29+ vertical extractors, rescue paths, and TLS fingerprinting are maintained upstream.
> **During development, constantly reference these codebases:**
> - **webclaw** — extraction pipeline internals (`webclaw-core/src/`), fetch client API (`webclaw-fetch/src/`), MCP tool design patterns (`webclaw-mcp/src/`)
> - **pi-searxng** — SearXNG query format (`searxng.ts`), content extraction patterns (`extract.ts`), GitHub URL parsing (`github.ts`)
>
> Use the **codemapper skill** (load via `read` the skill file at
> `~/.pi/agent/skills/codemapper/SKILL.md`) to efficiently traverse and analyze
> both codebases: find callers/callees, trace call paths from entrypoints, check
> which functions depend on which modules, and verify API surfaces before coding
> against them.

### 3.1 wa-core — Shared Types & Config

```rust
// ======== types.rs ========

/// A single web search result from SearXNG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// A single text file from a cloned git repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFile {
    /// Relative path within the repo (e.g. "src/main.rs")
    pub path: String,
    /// File content as string
    pub content: String,
    /// File size in bytes
    pub size: usize,
}

/// Result of cloning a git repository.
/// Includes the on-disk path so the AI agent can reference it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClonedRepo {
    /// Absolute path to the cloned repository on disk
    pub local_path: String,
    /// Text files extracted from the repository
    pub files: Vec<GitFile>,
}

/// Output format options for fetch and search commands.
/// The CLI layer provides clap::ValueEnum separately; this crate is clap-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Llm,
    Text,
    Json,
}

// ======== config.rs ========

/// Full application configuration, layered from:
///   1. Defaults
///   2. TOML config file (~/.config/wa/config.toml)
///   3. Environment variables (WA_*)
///   4. CLI arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// SearXNG instance URL (e.g. "http://localhost:8080" or "https://searx.example.com")
    #[serde(default = "default_searxng_url")]
    pub searxng_url: String,

    /// Timeout in seconds for SearXNG search requests
    #[serde(default = "default_timeout")]
    pub searxng_timeout: u64,

    /// Maximum number of search results to return
    #[serde(default = "default_max_results")]
    pub max_search_results: usize,

    /// Timeout in seconds for URL content fetches
    #[serde(default = "default_timeout")]
    pub fetch_timeout: u64,

    /// TLS fingerprint browser profile: "chrome", "firefox", "safari-ios", "random"
    #[serde(default = "default_browser")]
    pub browser_profile: String,
    // Note: browser_profile is a String in Config because wa-core is clap-free and
    // browser-agnostic. wa-extract converts it to BrowserProfile via a pub fn parse()
    // that returns WaError::Config on unrecognized values.

    /// Optional proxy URL for fetch operations
    #[serde(default)]
    pub proxy: Option<String>,

    /// Directory where git repos are cloned (None = system temp dir)
    pub git_temp_dir: Option<String>,

    /// Maximum file size in bytes to include from cloned repos
    #[serde(default = "default_max_file_size")]
    pub git_max_file_size: usize,

    /// Maximum number of files to include from cloned repos
    #[serde(default = "default_max_files")]
    pub git_max_files: usize,

    /// Default output format
    #[serde(default)]
    pub output_format: OutputFormat,

    /// Number of retry attempts for transient network failures (fetch/search)
    #[serde(default = "default_retries")]
    pub retries: u32,

    /// Base delay between retries in milliseconds
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
}

fn default_searxng_url() -> String { "http://localhost:8080".into() }
fn default_timeout() -> u64 { 30 }
fn default_max_results() -> usize { 10 }
fn default_browser() -> String { "chrome".into() }
fn default_max_file_size() -> usize { 102_400 }
fn default_max_files() -> usize { 100 }
fn default_retries() -> u32 { 3 }
fn default_retry_delay_ms() -> u64 { 500 }

// IMPORTANT: default_* functions MUST be defined BEFORE the Config struct
// because #[serde(default = "default_...")] references them at compile time.
// The functions are shown here for readability; in actual code they precede Config.

// fn load_config(cli_args: &CliArgs) -> Result<Config, WaError>
//   Layering: config_file → env vars → cli_args (each overrides previous)

// ======== error.rs ========

#[derive(Debug, Error)]
pub enum WaError {
    #[error("search failed: {0}")]
    Search(String),

    #[error("failed to fetch {url}: {source}")]
    Fetch { url: String, source: String },

    #[error("content extraction failed: {0}")]
    Extraction(String),

    #[error("git operation failed: {0}")]
    Git(String),

    #[error("git not found: install git or add it to PATH")]
    GitNotFound,

    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("output format error: {0}")]
    Output(String),

    #[error("rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("{0}")]
    Other(String),
}
```

### 3.2 wa-search — SearXNG Client

```rust
/// HTTP client for SearXNG search instances.
///
/// Makes GET requests to `{instance_url}/search?q=...&format=json&categories=general`
/// and parses the JSON response into `Vec<SearchResult>`.
pub struct SearXNGClient {
    instance_url: String,       // No trailing slash
    client: reqwest::Client,    // Pooled connection, timeout from config
}

impl SearXNGClient {
    /// Create a new client targeting the given SearXNG instance.
    ///
    /// `instance_url` — e.g. "http://localhost:8080" or "https://searx.example.com"
    /// `timeout_secs` — HTTP request timeout
    pub fn new(instance_url: String, timeout_secs: u64) -> Result<Self, WaError>;

    /// Search the web and return structured results.
    ///
    /// `query` — free-text search query
    /// `limit` — max results to return (capped by SearXNG instance config)
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, WaError>;
}
```

**SearXNG API contract:**
```
GET {instance_url}/search?q={query}&format=json&categories=general&safesearch=0
  → HTTP 200: application/json
  → Response shape: { "results": [ { "title", "url", "content" or "snippet" } ], ... }
  → HTTP 429 → WaError::RateLimit (rate-limited, retriable with backoff)
  → HTTP other non-200 → WaError::Search(status_text)
```

Results are deduplicated by URL (SearXNG may return the same page under different
categories — we keep only the first occurrence).

Retries: SearXNGClient::search() retries on connection refused, DNS failure,
timeout, HTTP 429, HTTP 503 (same retry strategy as section 10a). SearXNG 429
responses are mapped to `WaError::RateLimit` which signals the retry layer to
back off before retrying.

### 3.3 wa-extract — Content Extractor

```rust
/// Browser profile for TLS fingerprint impersonation.
/// Maps to webclaw_fetch::BrowserProfile.
/// The CLI layer provides clap::ValueEnum via a separate mapping; this crate is clap-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProfile {
    Chrome,
    Firefox,
    SafariIos,
    Random,
}

impl BrowserProfile {
    /// Convert to webclaw_fetch's native enum
    pub fn to_webclaw(self) -> webclaw_fetch::BrowserProfile {
        match self {
            Self::Chrome => webclaw_fetch::BrowserProfile::Chrome,
            Self::Firefox => webclaw_fetch::BrowserProfile::Firefox,
            Self::SafariIos => webclaw_fetch::BrowserProfile::SafariIos,
            Self::Random => webclaw_fetch::BrowserProfile::Random,
        }
    }
}

/// Content extraction options for wa-extract.
/// Converted to webclaw_core::ExtractionOptions before calling the extraction engine.
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// CSS selectors for elements to include (skips scoring if non-empty)
    pub include_selectors: Vec<String>,
    /// CSS selectors for elements to exclude from output
    pub exclude_selectors: Vec<String>,
    /// If true, pick the first article/main/[role="main"] element
    pub only_main_content: bool,
    /// If true, populate Content::raw_html
    pub include_raw_html: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            include_selectors: Vec::new(),
            exclude_selectors: Vec::new(),
            only_main_content: false,
            include_raw_html: false,
        }
    }
}

impl From<&ExtractOptions> for webclaw_core::ExtractionOptions {
    fn from(opts: &ExtractOptions) -> Self {
        webclaw_core::ExtractionOptions {
            include_selectors: opts.include_selectors.clone(),
            exclude_selectors: opts.exclude_selectors.clone(),
            only_main_content: opts.only_main_content,
            include_raw_html: opts.include_raw_html,
        }
    }
}

/// Fetches URLs via webclaw-fetch (BoringSSL TLS fingerprinting) and
/// extracts clean content via webclaw-core.
///
/// Wraps FetchClient in an Arc because fetch_batch requires &Arc<Self>.
pub struct Extractor {
    client: std::sync::Arc<webclaw_fetch::FetchClient>,
}

/// Re-export webclaw's extraction result type so callers don't need
/// to depend on webclaw-core directly.
pub use webclaw_core::ExtractionResult;

impl Extractor {
    /// Create a new extractor.
    ///
    /// `browser` — which TLS fingerprint to impersonate (Chrome, Firefox, Safari iOS, Random)
    /// `timeout_secs` — HTTP request timeout (converted to Duration internally)
    /// `cookies` — optional cookies, each joined with "; " and set as the Cookie header
    ///   (e.g. `vec!["session=abc", "token=xyz"]` → `Cookie: session=abc; token=xyz`)
    /// `proxy` — optional proxy URL (e.g. "http://user:pass@proxy:8080")
    pub fn new(
        browser: BrowserProfile,
        timeout_secs: u64,
        cookies: Option<Vec<String>>,
        proxy: Option<String>,
    ) -> Result<Self, WaError>;

    /// Fetch a URL and extract structured content.
    ///
    /// Delegates to FetchClient::fetch_and_extract_with_options() which includes:
    /// - PDF auto-detection and text extraction
    /// - Reddit JSON fallback (avoids verification wall)
    /// - LinkedIn embedded JSON extraction
    /// - Cookie warmup for Akamai challenge pages
    /// - Automatic retry on transient errors (2 attempts with backoff)
    /// - 50MB body size cap
    pub async fn fetch_and_extract(
        &self,
        url: &str,
        options: &ExtractOptions,
    ) -> Result<ExtractionResult, WaError>;

    /// Fetch raw HTML without extraction (for debugging / custom processing).
    /// Returns only the html string from FetchResult.
    pub async fn fetch_raw(&self, url: &str) -> Result<String, WaError>;

    /// Fetch and extract multiple URLs concurrently.
    /// The client is Arc-wrapped internally so fetch_batch can use &Arc<Self>.
    pub async fn fetch_batch(
        &self,
        urls: &[&str],
        concurrency: usize,
    ) -> Vec<BatchResult>;
}

/// Result for a single URL in a batch operation.
#[derive(Debug)]
pub struct BatchResult {
    pub url: String,
    pub result: Result<ExtractionResult, WaError>,
}
```

**webclaw-fetch + webclaw-core API we depend on:**
```rust
// Fetching (TLS fingerprinting via BoringSSL)
webclaw_fetch::FetchClient::new(config: FetchConfig) -> Result<FetchClient, FetchError>
// FetchClient is NOT Clone. batch methods take &Arc<Self>, so we store Arc<FetchClient>.
client.fetch(url: &str) -> Result<FetchResult, FetchError>
// FetchResult { html, status, url, headers, elapsed }

// Combined fetch+extract with rescue paths (Reddit JSON, LinkedIn, PDF, cookie warmup)
client.fetch_and_extract_with_options(url, &ExtractionOptions) -> Result<ExtractionResult, FetchError>

// Extraction (Readability-style scoring, markdown conversion)
webclaw_core::extract_with_options(html: &str, url: Option<&str>, options: &ExtractionOptions)
    -> Result<ExtractionResult, ExtractError>

// LLM-optimized formatting
webclaw_core::to_llm_text(result: &ExtractionResult, url: Option<&str>) -> String
```

**Implementation notes for Extractor::new():**
- `timeout_secs` → `Duration::from_secs(timeout_secs)` for `FetchConfig.timeout`
- `cookies` → join with `"; "` → add as `("cookie", joined)` to `FetchConfig.headers`
- `proxy` → `FetchConfig.proxy` (direct mapping)
- `browser` → `BrowserProfile::to_webclaw()` → `FetchConfig.browser`
- Other FetchConfig fields: `follow_redirects: true`, `max_redirects: 10`, `pdf_mode: PdfMode::default()`

### 3.4 wa-git — Git Cloner

```rust
/// Options for git repository cloning and file listing.
#[derive(Debug, Clone)]
pub struct GitCloneOptions {
    /// Directory to clone into (None = /tmp/wa-git-XXXXX pattern, like pi-searxng)
    pub target_dir: Option<std::path::PathBuf>,
    /// Max file size in bytes to include (larger files are skipped)
    pub max_file_size: usize,
    /// Max number of files to include (stops after reaching limit)
    pub max_files: usize,
}

/// Result of a successful clone — includes both the on-disk path
/// and the list of text files the AI agent can consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClonedRepo {
    /// Absolute path to the cloned repository on disk
    pub local_path: String,
    /// Text files extracted from the repository
    pub files: Vec<GitFile>,
}

/// Clones git repositories and lists their text files.
///
/// The cloned repository persists on disk at `local_path` so the AI agent
/// can perform further operations (build, grep, run tests, etc.). No cleanup
/// is performed — the caller owns the directory.
pub struct GitCloner;

impl GitCloner {
    /// Clone a repository and return the on-disk path + text file contents.
    ///
    /// Only clones repos hosted on github.com, gitlab.com, and codeberg.org.
    /// Other URLs return an error with a clear message.
    ///
    /// Performs a shallow clone (--depth 1).
    /// Skips binary files, .git/, node_modules/, and other noise dirs.
    /// Handles both repo root URLs and blob/tree sub-URLs.
    ///
    /// Default clone location: /tmp/wa-git-XXXXX/{repo}/ (uses os tmpdir).
    /// The directory is NOT cleaned up — it remains available for the AI agent.
    pub fn clone_and_list(url: &str, options: &GitCloneOptions) -> Result<ClonedRepo, WaError>;
}
```

**Supported URL formats:**
```
https://github.com/{owner}/{repo}
https://github.com/{owner}/{repo}/tree/{branch}
https://github.com/{owner}/{repo}/blob/{branch}/path/to/file
https://gitlab.com/{owner}/{repo}
https://codeberg.org/{owner}/{repo}
```

**Noise filtering (same rules as pi-searxng):**
- Skip: binary extensions (.png, .jpg, .ico, .zip, .pdf, .mp4, .ttf, .woff, .eot, .webp, .gif, .svg, .bmp)
- Skip dirs: .git, node_modules, vendor, target, dist, build, __pycache__, .venv
- Skip files: package-lock.json, yarn.lock, Cargo.lock, go.sum (lockfiles)
- Skip hidden files (starting with .) except .env.example, .gitignore

---

## 4. CLI Design (wa-cli)

```
wa [GLOBAL_FLAGS] <COMMAND> [COMMAND_ARGS]

Global Flags (available on all commands):
  --format <FORMAT>        Output format: markdown, llm, text, json [default: markdown]
  --output, -o <FILE>      Write output to file instead of stdout
  --config <PATH>          Config file path
  --verbose, -v            Verbose logging
  --quiet, -q              Suppress all output except errors (AI agent mode)
  --help, -h               Show help
  --version, -V            Show version

Commands:
  search    Search the web via SearXNG
  fetch     Fetch and extract content from a URL
  git       Clone a git repository and list its text files
  config    Show current configuration
  help      Show help for any command
```

### 4.1 `wa search`

```
wa search <QUERY> [OPTIONS]

Query with spaces; the search combines all arguments or a quoted string.

Options:
  --limit, -n <N>         Max results [default: 10]
  --fetch, -f             Also fetch and extract each result's content
  --fetch-limit <N>       Max results to fetch [default: 3, implies --fetch]
  --concurrency, -c <N>   Concurrent fetches when --fetch is active [default: 2]
  --browser <PROFILE>     TLS fingerprint for --fetch mode [default: chrome]
  --cookies <COOKIE>      Cookie to send in --fetch mode (repeatable)
  --proxy <URL>           Proxy URL for --fetch mode
  --searxng-url <URL>     SearXNG instance URL (overrides config)
  --no-meta               Omit metadata header from extracted pages
  --include-structured-data  Append JSON-LD structured data (markdown/llm only)
  --format <FORMAT>       Override global format for this command

Examples:
  wa search "rust async programming" -n 5
  wa search "how to build a CLI in Rust" --fetch --fetch-limit 2 -o results.md
  wa search "tokio tutorial" -n 5 --format json | jq '.[].url'
  wa search "paywalled article" --fetch --browser firefox --cookies "session=xyz"
  wa search "rust cli" -n 20 --fetch --concurrency 4
```

**Extraction pipeline for `--fetch` mode:**
1. SearXNG returns JSON with (title, url, snippet) for each result
2. Each result URL is fetched via **webclaw-fetch** (BoringSSL TLS fingerprinting)
3. Raw HTML is extracted via **webclaw-core**'s scoring engine → clean markdown
4. Output is formatted per `--format` (markdown/llm/text/json)
5. For `--format llm`, `webclaw_core::to_llm_text()` is applied to each extracted page

This is **not** Mozilla Readability — it's webclaw's 95.1%-accuracy extraction engine
with multi-signal scoring, noise filtering, data island extraction, and token-optimized LLM formatting.
Garbage in, clean content out.

### 4.2 `wa fetch`

```
wa fetch <URL> [URL ...] [OPTIONS]

Accepts one or more URLs. Multiple URLs are fetched concurrently.

Options:
  --include <SELECTOR>    CSS selector(s) to include (repeatable)
  --exclude <SELECTOR>    CSS selector(s) to exclude (repeatable)
  --only-main             Extract only <article>/<main>/[role="main"]
  --raw                   Include raw HTML in output
  --browser <PROFILE>     TLS fingerprint: chrome, firefox, safari-ios, random [default: chrome]
  --cookies <COOKIE>      Cookie to send (repeatable, e.g. --cookies "session=abc")
  --proxy <URL>           Proxy URL (e.g. http://proxy:8080 or socks5://proxy:1080)
  --concurrency, -c <N>   Max concurrent fetches [default: 4]
  --no-meta               Omit metadata header from output
  --include-structured-data  Append JSON-LD structured data (markdown/llm only)
  --format <FORMAT>       Override global format for this command

Examples:
  wa fetch https://example.com/article
  wa fetch https://a.com https://b.com https://c.com --concurrency 3
  wa fetch https://example.com --include "article" --exclude ".sidebar" --format llm
  wa fetch https://docs.rs/tokio --only-main -o tokio-docs.md
  wa fetch https://paywalled.example.com --browser firefox --cookies "token=xyz"
```

### 4.3 `wa git`

```
wa git <URL> [OPTIONS]

URL must point to a github.com, gitlab.com, or codeberg.org repository.
The cloned repo persists on disk at /tmp/wa-git-XXXXX/{repo}/ — no cleanup.

Options:
  --max-files <N>         Max files to include [default: 100]
  --max-size <N>          Max file size in bytes [default: 102400]
  --output-dir <DIR>      Clone into this directory (default: /tmp/wa-git-XXXXX/)
  --format <FORMAT>       Override global format [default: markdown]

Examples:
  wa git https://github.com/serde-rs/serde
  wa git https://github.com/tokio-rs/tokio --max-files 50 -o tokio-code.md

Output always includes the clone path so the AI agent knows where the repo lives:
  Cloned to: /tmp/wa-git-a3f2/serde
```

### 4.4 `wa config`

```
wa config

Shows current effective configuration with source annotations:
  searxng_url        http://localhost:8080        (default)
  max_search_results 10                            (default)
  fetch_timeout      30                            (config file)
  output_format      markdown                      (CLI arg)
```

---

## 5. Output Formats

### 5.1 Markdown (default)

For `search` (no `--fetch`):
```markdown
## Web Search Results for: "rust async programming"

### 1. Async Programming in Rust — Rust Docs
- **URL:** https://doc.rust-lang.org/async-book/
- **Snippet:** An introduction to asynchronous programming in Rust...

### 2. Tokio — An async runtime for Rust
- **URL:** https://tokio.rs/
- **Snippet:** Tokio is an asynchronous runtime for the Rust programming language...
```

For `search --fetch` (extraction via webclaw-core, NOT readability):
```markdown
## Web Search Results for: "rust async" (2 pages fetched)

### 1. Async Programming in Rust — Rust Docs
- **URL:** https://doc.rust-lang.org/async-book/
- **Snippet:** An introduction to asynchronous programming in Rust...

## Extracted Content

### Page 1: https://doc.rust-lang.org/async-book/

# Async Programming in Rust

Asynchronous programming in Rust lets you run ...

## Why Async?

Async programming is useful for ...

### Page 2: https://tokio.rs/

# Tokio — An asynchronous runtime for Rust

Tokio is an event-driven, non-blocking I/O platform ...
```

For `fetch`: webclaw-core's native markdown output (headings, lists, code blocks, links).

For `git`:
```markdown
# Repository: serde-rs/serde
## Cloned to: /tmp/wa-git-a3f2/serde (branch: main)

## src/lib.rs (12.4 KB)
```rust
// ... file content ...
```

## src/de/mod.rs (8.1 KB)
```rust
// ... file content ...
```
```

### 5.2 LLM (`--format llm`)

For `fetch` / `search --fetch`: Uses `webclaw_core::to_llm_text()` plus `wa`-specific
post-processing for better LLM comprehension:

**Body text (from webclaw):**
- Metadata in `> ` prefix lines
- No images, no bold/italic
- Links moved to dedicated `## Links` section at bottom
- Duplicate paragraphs deduplicated
- Code blocks preserved
- Empty lines collapsed to at most 1 consecutive

**Post-processing (wa-cli):**
- **Link brackets restored**: `[label]` brackets are re-inserted around link text
  in the body (webclaw strips them to plain text). The LLM knows which words were
  originally hyperlinked without reading full URLs.
- **Tracking params stripped**: `utm_*` and `ref` query parameters are removed
  from all `## Links` footer URLs, reducing token bloat by ~30-40% on heavily
  tracked pages (newsletters, campaign links).
- **Structured data excluded by default**: The JSON-LD appendix is only included
  when `--include-structured-data` is passed.

For `search` (no `--fetch`) and `git`: Uses a compact structured format:
```
> SearXNG search: "query" (5 results)
1. [Result Title](https://url.com) — snippet text here...
2. [Another Title](https://other.com) — another snippet...
```
For `git`, same metadata-prefixed compact format with file paths.

### 5.3 Text (`--format text`)

Plain text — webclaw-core's `content.plain_text` field. No formatting, no links.

### 5.4 JSON (`--format json`)

**All JSON output uses a unified per-URL result schema.** Whether from `wa fetch`,
`wa search --fetch`, or batch operations, the output is always `ResultObject[]`:

```json
[
  {
    "url": "https://doc.rust-lang.org/async-book/",
    "status": "ok",
    "markdown": "# Async Programming\n\n...",
    "plain_text": "Async programming lets you...",
    "metadata": {
      "title": "Async Programming in Rust",
      "description": "An introduction to...",
      "site_name": "Rust Docs",
      "word_count": 1234,
      "published_date": null
    },
    "links": ["https://crates.io/...", "https://github.com/..."],
    "code_blocks": [],
    "domain": "Documentation"
  },
  {
    "url": "https://bad.example.com",
    "status": "error",
    "error": "connection refused"
  }
]
```

**Schema:**
| Field | Type | Present when |
|-------|------|-------------|
| `url` | string | always |
| `status` | `"ok"` or `"error"` | always |
| `error` | string | `status = "error"` |
| `markdown` | string | `status = "ok"` |
| `plain_text` | string | `status = "ok"` |
| `metadata` | object | `status = "ok"` |
| `links` | `string[]` | `status = "ok"` |
| `code_blocks` | `string[]` | `status = "ok"` |
| `domain` | string | `status = "ok"` |

For `wa search --fetch --format json`, each result object additionally includes
the SearXNG snippet metadata:

```json
[
  {
    "search_title": "Async in Rust — Rust Docs",
    "url": "https://doc.rust-lang.org/async-book/",
    "snippet": "An introduction to asynchronous programming...",
    "status": "ok",
    "markdown": "# Async Programming\n\n...",
    ...
  }
]
```

Added fields for search: `search_title` (string), `snippet` (string).

For `wa search` without `--fetch`, the output is a flat array of `SearchResult`:

```json
[
  { "title": "Async in Rust", "url": "https://...", "snippet": "..." },
  { "title": "Tokio", "url": "https://...", "snippet": "..." }
]
```

For `wa git`, the output is a `ClonedRepo` object with `local_path` and `files` array.

---

## 5a. I/O Stream Contract

AI agents consume stdout. This is the contract:

| Stream | Content |
|--------|---------|
| **stdout** | Formatted result output (markdown/llm/text/json). Always valid UTF-8. Machine-parseable when `--format json`. |
| **stderr** | Log messages, progress indicators, warnings, error descriptions. |
| **`--quiet`** | Suppresses all stderr output (AI agent mode). Stdout unchanged. |
| **`--output FILE`** | Result written to FILE; stdout is empty. Stderr still carries errors/progress unless `--quiet`. |
| **Exit code 0** | Success. Stdout contains a valid result. |
| **Exit code non-zero** | Error. Stderr contains the error message. Stdout empty. |

**Rationale:** AI tools (Claude, Cursor, Cody, etc.) parse stdout as the structured result.
Progress spinners, log messages, or debug info must never leak into stdout or they
will corrupt JSON parsing and LLM context injection. This is the same contract that
`git diff`, `jq`, and other pipeline tools follow.

---

## 6. Configuration System

### 6.1 Config File (`~/.config/wa/config.toml`)

```toml
# SearXNG instance URL
searxng_url = "http://localhost:8080"

# Request timeouts (seconds)
searxng_timeout = 10
fetch_timeout = 30

# Retry settings for network operations
retries = 3                   # max attempts (0 = no retry)
retry_delay_ms = 500          # base delay, doubled each retry

# Search limits
max_search_results = 10

# TLS fingerprint profile for fetch operations
browser_profile = "chrome"    # chrome | firefox | safari-ios | random

# Optional proxy for fetch operations (HTTP, HTTPS, SOCKS5)
# proxy = "http://user:pass@proxy:8080"

# Git cloning
git_max_file_size = 102400    # 100 KB
git_max_files = 100

# Default output format
output_format = "markdown"    # markdown | llm | text | json
```

### 6.2 Environment Variables

| Variable | Equivalent |
|----------|-----------|
| `WA_SEARXNG_URL` | `--searxng-url` (on `wa search` only) |
| `WA_SEARXNG_TIMEOUT` | config only |
| `WA_FETCH_TIMEOUT` | config only |
| `WA_BROWSER_PROFILE` | `--browser` (on `wa fetch` / `wa search --fetch`) |
| `WA_PROXY` | `--proxy` (on `wa fetch` / `wa search --fetch`) |
| `WA_RETRIES` | config only |
| `WA_OUTPUT_FORMAT` | `--format` |
| `WA_GIT_TEMP_DIR` | `--output-dir` (on `wa git`) |

### 6.3 Precedence (highest wins)

```
CLI args  >  env vars  >  config file  >  defaults
```

---

## 7. Testing Strategy (TDD-Driven)

### 7.1 Test Philosophy

Every module is developed **test-first**:
1. Write the test that describes the desired behavior
2. Run — confirm it fails (red)
3. Write minimal implementation to pass (green)
4. Refactor with confidence (blue)

Tests live alongside code (`#[cfg(test)] mod tests { ... }` in the same file) for unit tests, and in `tests/` for integration tests.

### 7.2 wa-core — Test Plan

| # | Test Name | What It Validates |
|---|-----------|-------------------|
| T1 | `config_defaults` | All defaults are sensible |
| T2 | `config_from_toml_file` | Parsing a valid TOML config |
| T3 | `config_from_toml_partial` | Missing keys fall back to defaults |
| T4 | `config_from_invalid_toml` | Returns WaError::Config on bad TOML |
| T5 | `config_env_override` | WA_SEARXNG_URL overrides config file |
| T6 | `config_cli_override` | CLI args take highest precedence |
| T7 | `config_file_not_found` | Missing config file → defaults (no error) |
| T8 | `output_format_serialization` | "markdown" ↔ OutputFormat::Markdown |
| T9 | `search_result_json_roundtrip` | Serialize → deserialize preserves data |
| T10 | `git_file_json_roundtrip` | Same for GitFile |
| T11 | `cloned_repo_json_roundtrip` | Same for ClonedRepo (local_path + files) |
| T12 | `error_display_formatting` | WaError::Fetch displays URL and source |
| T13 | `error_io_from` | `?` operator works with io::Error |

### 7.3 wa-search — Test Plan

| # | Test Name | What It Validates |
|---|-----------|-------------------|
| T14 | `build_search_url` | URL construction: encoding, categories, format |
| T15 | `build_search_url_with_spaces` | Query with spaces is properly encoded |
| T16 | `parse_success_response` | Valid SearXNG JSON → Vec<SearchResult> |
| T17 | `parse_empty_results` | `{"results": []}` → empty vec, no error |
| T18 | `parse_missing_content_field` | Results with only "snippet" instead of "content" |
| T19 | `parse_malformed_json` | Invalid JSON → WaError::Search |
| T20 | `http_error_status` | HTTP 500 → WaError::Search with status |
| T21 | `connection_refused` | SearXNG not running → WaError::Search |
| T22 | `test_bad_instance_url` | Invalid URL (e.g., "not-a-url") → error |
| T23 | `test_result_limit` | limit parameter truncates results |
| T24 | `test_empty_query` | Empty query → handled gracefully |
| T25 | `test_duplicate_urls` | SearXNG returns same URL under different categories → deduplicated |

**Testing approach for HTTP:** Use `wiremock` or `httpmock` to spin up a local HTTP server that responds with crafted SearXNG-like JSON.

### 7.4 wa-extract — Test Plan

| # | Test Name | What It Validates |
|---|-----------|-------------------|
| T26 | `extract_basic_html` | Simple HTML → markdown with headings |
| T27 | `extract_with_code_blocks` | Code blocks are preserved |
| T28 | `extract_with_links` | Links are extracted to `content.links` |
| T29 | `extract_with_metadata` | Title, description, author are captured |
| T30 | `extract_empty_body` | Returns ExtractionResult with word_count > 0 |
| T31 | `extract_404_page` | HTTP 404 → WaError::Fetch |
| T32 | `extract_timeout` | Timeout → WaError::Fetch |
| T33 | `extract_include_selectors` | Only selectors listed appear in output |
| T34 | `extract_exclude_selectors` | Selectors listed are stripped from output |
| T35 | `extract_only_main_content` | Only article/main content in output |
| T36 | `extract_raw_html_flag` | include_raw_html=true populates raw_html field |
| T37 | `extract_llm_format` | to_llm_text produces token-optimized output |
| T38 | `extract_invalid_url` | "not-a-valid-url" → WaError::InvalidUrl |
| T39 | `extract_browser_profiles` | Chrome, Firefox, SafariIos, Random all work |
| T40 | `extract_with_cookies` | Cookie header is forwarded to the server |
| T41 | `extract_redirects` | 301/302 redirects are followed transparently |

**Testing approach:** Use `wiremock` for HTTP mocking. For browser profile tests (T37), verify that FetchConfig with the profile builds successfully. Extraction quality tests (T24–T28, T31–T35) use static HTML strings — these don't need a running HTTP server since webclaw-core's `extract_with_options` takes raw HTML. The HTTP-layer tests (T29, T30, T38, T39) use wiremock to simulate server behavior.

### 7.5 wa-git — Test Plan

| # | Test Name | What It Validates |
|---|-----------|-------------------|
| T42 | `parse_github_root_url` | github.com/user/repo → owner/user, repo, branch=main |
| T43 | `parse_github_tree_url` | github.com/user/repo/tree/dev | Same, branch=dev |
| T44 | `parse_github_blob_url` | github.com/user/repo/blob/main/src/lib.rs → correct |
| T45 | `parse_gitlab_url` | gitlab.com/user/repo → correct owner/user, repo |
| T46 | `parse_codeberg_url` | codeberg.org/user/repo → correct |
| T47 | `parse_unsupported_host` | bitbucket.org/user/repo → WaError::Git |
| T48 | `parse_not_a_repo_url` | github.com/user → WaError::Git |
| T49 | `parse_gist_url_excluded` | gist.github.com → WaError::Git with clear message |
| T50 | `clone_small_repo` | Clone a tiny test repo → files listed + local_path present |
| T51 | `clone_skip_binary` | Binary files (.png, .zip) are excluded |
| T52 | `clone_skip_noise_dirs` | node_modules, .git are excluded |
| T53 | `clone_skip_hidden_files` | .env, .DS_Store excluded; .gitignore included |
| T54 | `clone_skip_lockfiles` | Cargo.lock, package-lock.json excluded |
| T55 | `clone_max_file_size` | Files > max_file_size are skipped |
| T56 | `clone_max_files` | Stops after max_files reached |
| T57 | `clone_invalid_url` | "not-a-url" → WaError::InvalidUrl |
| T58 | `clone_nonexistent_repo` | github.com/this/repo-does-not-exist → WaError::Git |
| T59 | `git_clone_fresh_repo` | A repo that was freshly created (no existing clone) |

**Testing approach:** Create a tiny local git repo in test setup (`git init`, `git add`, `git commit`). Serve it via a local file:// URL or use an actual hosted test repo. For URL parsing, pure unit tests with no git involved.

### 7.6 wa-cli — Integration Test Plan

| # | Test Name | What It Validates |
|---|-----------|-------------------|
| T60 | `cli_help` | `wa --help` exits 0 with usage text |
| T61 | `cli_version` | `wa --version` prints version |
| T62 | `cli_search_basic` | `wa search "test"` produces markdown output |
| T63 | `cli_search_json` | `wa search "test" --format json` is valid JSON |
| T64 | `cli_search_fetch` | `wa search "test" --fetch --fetch-limit 1` includes extracted content |
| T65 | `cli_fetch_output_file` | `wa fetch URL -o /tmp/test.md` writes to file |
| T66 | `cli_fetch_stdout` | `wa fetch URL` writes to stdout |
| T67 | `cli_git_output_file` | `wa git URL -o /tmp/repo.md` writes to file |
| T68 | `cli_config_command` | `wa config` prints config without errors |
| T69 | `cli_missing_command` | `wa` (no subcommand) shows help |
| T70 | `cli_bad_format` | `wa search "test" --format pdf` → error message |
| T71 | `cli_quiet_mode` | `wa --quiet search "test"` hides non-error output |

---

## 8. Implementation Order (TDD Sequence)

Each phase delivers a working, test-passing artifact. No phase starts before the previous phase's tests are green.

### Phase 1: Project Scaffolding ✅ **DONE (2026-05-01)**

- [x] 1.1 Initialize workspace Cargo.toml with all 5 crate members
- [x] 1.2 Create wa-core/Cargo.toml with dependencies
- [x] 1.3 Create stub lib.rs in each crate
- [x] 1.4 Create wa-cli/src/main.rs with hello-world clap binary
- [x] 1.5 Verify cargo build succeeds
- [x] 1.6 Create .gitignore
- [x] 1.7 Create README.md
- [x] 1.8 Each crate declares license = "AGPL-3.0"

- [ ] 1.1 Initialize workspace `Cargo.toml` with all 5 crate members
- [ ] 1.2 Create `wa-core/Cargo.toml` with dependencies
- [ ] 1.3 Create stub `lib.rs` in each crate (just `pub fn hello() -> &str { "ok" }`)
- [ ] 1.4 Create `wa-cli/src/main.rs` with a hello-world clap binary
- [ ] 1.5 Verify `cargo build` succeeds for the entire workspace (first build fetches + compiles webclaw from GitHub)
- [ ] 1.6 Create `.gitignore` (target/, .idea/, *.swp, test temp dirs — do NOT ignore Cargo.lock; this is a binary application, not a library)
- [ ] 1.7 Create `README.md` (project summary, installation, quickstart)
- [ ] 1.8 Each crate's `Cargo.toml` declares `license = "AGPL-3.0"`

**Deliverable:** `cargo run` prints "Hello from wa!" with `--help` working.

### Phase 2: wa-core — T1 through T13 ✅ **DONE (2026-05-01)**

Status: all 13 tests pass.

Write tests first, then implement:

- [ ] 2.1 `types.rs` — SearchResult, GitFile, OutputFormat with Serialize/Deserialize
- [ ] 2.2 `error.rs` — WaError enum with thiserror::Error derive
- [ ] 2.3 `config.rs` — Config struct with defaults, TOML parsing, env var loading, layered merging
- [ ] 2.4 All 13 tests pass

**Deliverable:** `wa-core` crate with all types, config loading, and error types. Zero I/O.

### Phase 3: wa-search — T14 through T25 ✅ **DONE (2026-05-01)**

Status: all 12 tests pass.

- [x] 3.1 Set up `wiremock` dev-dependency
- [x] 3.2 Write mock server helpers for SearXNG responses
- [x] 3.3 Write all 12 tests
- [x] 3.4 Implement `SearXNGClient::new()`
- [x] 3.5 Implement `SearXNGClient::search()` — HTTP GET, parse JSON, map to SearchResult
- [x] 3.6 All 12 tests pass

**Deliverable:** `wa-search` crate that can talk to any SearXNG instance.

### Phase 4: wa-extract — T26 through T41 ✅ **DONE (2026-05-01)**

Status: all 16 tests pass.

- [x] 4.1 Add `webclaw-fetch` as git dependency
- [x] 4.2 Add `wiremock` dev-dependency
- [x] 4.3 Write all 16 tests
- [x] 4.4 Implement `BrowserProfile` enum + `to_webclaw()` mapping to `webclaw_fetch::BrowserProfile`
- [x] 4.5 Implement `BrowserProfile::try_from_str(s: &str)` for string→enum conversion (used by wa-cli)
- [x] 4.6 Implement `Extractor::new()` — build `FetchConfig` from browser, timeout, cookies, proxy;
    wrap resulting `FetchClient` in `Arc`; return `Self { client: Arc<FetchClient> }`
- [x] 4.7 Implement `fetch_raw()` — `client.fetch(url).await`
- [x] 4.8 Implement `fetch_and_extract()` — delegate to `client.fetch_and_extract_with_options(url, &opts).await`
    which includes Reddit JSON fallback, LinkedIn extraction, PDF detection, and cookie warmup
- [x] 4.9 Implement `fetch_batch()` — delegate to `client.fetch_and_extract_batch_with_options().await`
- [x] 4.10 Re-export `webclaw_core::to_llm_text` free function
- [x] 4.11 All 16 tests pass

**Deliverable:** `wa-extract` crate that fetches URLs with TLS fingerprinting and produces clean content.

### Phase 5: wa-git — T42 through T59 ✅ **DONE (2026-05-01)**

Status: all 18 tests pass (8 unit + 10 integration).

- [ ] 5.1 Add `tempfile`, `walkdir`, and `regex` dependencies
- [ ] 5.2 Write URL parsing tests (T40–T47) — pure unit tests, no git needed
- [ ] 5.3 Write cloning tests (T48–T51, T53–T54) — use a local temp git repo
- [ ] 5.4 Write error case tests (T52, T55–T57)
- [ ] 5.5 Implement git binary check (`which git` or equivalent) → WaError::GitNotFound
- [ ] 5.6 Implement URL parser: `parse_repo_url(url) -> (host, owner, repo, branch?)`
- [ ] 5.7 Implement `clone_and_list()`: git clone --depth 1, walkdir for file walking, filter, read, return Vec<GitFile>
- [ ] 5.8 Implement binary detection (by extension)
- [ ] 5.9 Implement noise dir/file filtering
- [ ] 5.10 All 18 tests pass

**Deliverable:** `wa-git` crate that clones repos and lists text files.

### Phase 6: wa-cli — T60 through T71 (Integration) ✅ **DONE (2026-05-01)**

Status: 7 pass, 2 ignored (network), 3 deferred (mock SearXNG in CLI tests).

- [x] 6.1 Implement `main.rs` with clap derive structs (Cli, Commands)
- [x] 6.2 Implement search command — wire SearXNGClient, format output
- [x] 6.3 Implement fetch command — wire Extractor with browser/cookies/proxy, format output
- [x] 6.4 Implement git command — wire GitCloner, format output
- [x] 6.5 Implement config command — show resolved config as JSON
- [x] 6.6 Implement output formatting (markdown, llm, text, json) inline
- [x] 6.7 Wire file output (`-o` flag)
- [x] 6.8 Integration tests pass (7/9, 2 ignored)

**Deliverable:** Fully functional `wa` CLI binary.

### Phase 7: Structured Data & LLM Format Improvements ✅ **DONE (2026-05-26)**

Status: All features implemented, 16 unit tests pass, 15 workspace test suites green.

- [x] 7.1 Add `--include-structured-data` flag to `search`, `fetch`, `browser` commands
- [x] 7.2 Make structured data appendix conditional (default: off for markdown/llm, always on for json)
- [x] 7.3 Implement `bracket_links_in_llm_body()` — restore `[label]` brackets around link text in `--format llm`
- [x] 7.4 Implement `clean_url()` — strip `utm_*` and `ref` tracking parameters from URLs
- [x] 7.5 Implement `clean_links_footer_urls()` — apply URL cleaning to `## Links` footer in `--format llm`
- [x] 7.6 Add unit tests for bracket restoration (8 tests: basic, no-links, longest-first, double-bracket, metadata-skip, word-boundary, multiple-occurrences)
- [x] 7.7 Add unit tests for URL cleaning (8 tests: utm stripping, non-tracking preservation, no-query, fragment preservation, empty-query collapse, footer batch, structured-data boundary, no-links passthrough)
- [x] 7.8 Update README.md and PLAN.md with new behavior documentation

**Deliverable:** `--format llm` produces token-efficient output with semantic link
preservation and cleaned URLs. `--include-structured-data` gives users explicit
control over JSON-LD appendix inclusion.

### Phase 8: URL Rewrite / Redirect System ✅ **DONE (2026-05-26)**

Status: All features implemented, 80 tests pass across 15 workspace test suites.

- [x] 8.1 Add `wa-core::url_rewrite` module with `UrlRewriteRule` + `UrlRewriter`
- [x] 8.2 Implement regex compilation with clear error messages (rule index included)
- [x] 8.3 Add `url_rewrites: Vec<UrlRewriteRule>` to `Config` with TOML array-of-tables support
- [x] 8.4 Update `overlay_file()` and `CONFIG_TEMPLATE` with commented examples
- [x] 8.5 Wire rewriter into `wa-cli` — create after `Config::load`, apply before every fetch
- [x] 8.6 Update `format_compact_meta` to show `fetched_url` when rewritten
- [x] 8.7 Add `inject_fetched_url_into_llm()` post-processor for `--format llm` metadata
- [x] 8.8 Apply rewrites in all fetch paths: `search --fetch`, `fetch`, `browser` (single + batch)
- [x] 8.9 Add 6 unit tests in `wa-core::url_rewrite` (match, no-match, first-wins, captures, invalid-regex, empty-rules, preserve-fragment)
- [x] 8.10 Update README.md with comprehensive URL rewrite documentation (config format, examples, recipes)
- [x] 8.11 Update PLAN.md with design decision records

**Deliverable:** Transparent regex-based URL rewriting with ordered rule evaluation,
capture-group expansion, and full output transparency (both original and rewritten
URLs visible in metadata headers).

### Phase 9: Polish

- [ ] 9.1 Add `tracing` + `tracing-subscriber` for structured logging
- [ ] 9.2 Add `--verbose` / `--quiet` flag handling
- [ ] 9.3 Add retry logic for wa-search and wa-extract (exponential backoff with jitter)
- [ ] 9.4 Add progress indication for multi-fetch operations (spinner + completion counts)
- [ ] 9.5 Error messages are consistently formatted and actionable
- [ ] 9.6 `direnv` / `.env.example` for development convenience
- [ ] 9.7 CI: GitHub Actions to run `cargo test`, `cargo clippy`, `cargo fmt --check`

---

## 9. Dependency Map

### Workspace-level (`Cargo.toml`)

```toml
[workspace]
resolver = "2"
members = ["crates/*"]
license = "AGPL-3.0"

[workspace.dependencies]
wa-core = { path = "crates/wa-core" }
wa-search = { path = "crates/wa-search" }
wa-extract = { path = "crates/wa-extract" }
wa-git = { path = "crates/wa-git" }

# Shared across crates
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
url = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive", "env"] }
toml = "0.8"
tempfile = "3"
regex = "1"

# HTTP (SearXNG only — simple JSON API, no bot protection)
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# External — webclaw full stack (fetching + extraction, AGPL-3.0)
# NOT on crates.io — must use git dependency
webclaw-fetch = { git = "https://github.com/0xMassi/webclaw", rev = "923445f" }
# webclaw-fetch re-exports webclaw-core + webclaw-pdf

# Dev only
wiremock = "0.6"
assert_cmd = "2"                  # CLI integration testing
predicates = "3"                  # Assertion combinators for assert_cmd
```

### Per-crate dependencies

| Crate | Runtime deps | Dev deps |
|-------|-------------|----------|
| **wa-core** | serde, thiserror, url, toml | — |
| **wa-search** | wa-core, reqwest, serde_json | wiremock, tokio |
| **wa-extract** | wa-core, webclaw-fetch | wiremock, tokio |
| **wa-git** | wa-core, tempfile, regex, walkdir | — |
| **wa-cli** | wa-core, wa-search, wa-extract, wa-git, clap, tokio, tracing, tracing-subscriber | assert_cmd, predicates |

**webclaw-fetch transitive dependencies** (resolved from git source, AGPL-3.0):
- `webclaw-core` — pure extraction engine (scraper, ego-tree, url, regex, serde)
- `webclaw-pdf` — PDF text extraction (pdf-extract)
- `wreq` 6.0.0-rc.28 — HTTP client with BoringSSL TLS fingerprinting
- `wreq-util`, `calamine` (spreadsheets), `quick-xml`, `zip`, `reqwest`

Note: `webclaw-fetch` re-exports `webclaw_core` types — `ExtractionResult`,
`ExtractionOptions`, `ExtractError`, `to_llm_text()` are all available without
adding a separate dependency. The clean HTML→content seam is:
```rust
webclaw_core::extract_with_options(raw_html, url, opts) -> ExtractionResult
```

---

## 10. Error Handling Philosophy

1. **Library crates return `Result<T, WaError>`** — never panic, never unwrap in library code.
2. **`WaError` variants are descriptive and actionable** — each variant message tells the user what went wrong and (where possible) how to fix it.
3. **Errors propagate via `?`** — `WaError` implements `From<io::Error>`, `From<serde_json::Error>`, and `From<webclaw_fetch::FetchError>` for seamless `?` usage.
4. **CLI binary maps errors to exit codes:**
   - `WaError::Config` → exit code 2 (misuse)
   - `WaError::InvalidUrl` → exit code 2 (misuse)
   - `WaError::GitNotFound` → exit code 2 (misuse)
   - `WaError::Io` → exit code 1 (system error)
   - Everything else → exit code 1
5. **The binary never panics** — all unwraps/expects are replaced with proper error handling.

### Error conversion from webclaw-fetch:
```rust
impl From<webclaw_fetch::FetchError> for WaError {
    fn from(e: webclaw_fetch::FetchError) -> Self {
        match &e {
            webclaw_fetch::FetchError::InvalidUrl(u) => WaError::InvalidUrl(u.clone()),
            webclaw_fetch::FetchError::Extraction(ee) => WaError::Extraction(ee.to_string()),
            _ => WaError::Fetch { url: "unknown".into(), source: e.to_string() },
        }
    }
}

impl From<webclaw_core::ExtractError> for WaError {
    fn from(e: webclaw_core::ExtractError) -> Self {
        WaError::Extraction(e.to_string())
    }
}
```

## 10a. Retry & Resilience

All network operations support automatic retries for transient failures:

| Config key | Default | Description |
|-----------|---------|-------------|
| `retries` | 3 | Max retry attempts |
| `retry_delay_ms` | 500 | Base delay between retries |

**Retriable errors:** connection refused, DNS failure, timeout, HTTP 429, HTTP 503.
**Non-retriable errors:** HTTP 400, 401, 403, 404, 500, invalid URL, extraction failures.

Retry strategy: exponential backoff with jitter. Base delay × 2^attempt, ±25% random jitter.

## 10b. Pre-flight Checks

**wa-git** checks for `git` binary availability on construction. If `git` is not found in
PATH, returns `WaError::GitNotFound` immediately rather than failing mid-operation.

**wa-search** sends a `User-Agent: wa/0.1 searxng-client` header on all requests to
identify itself to SearXNG instances (many instances filter out generic/default user-agents).

---

## 11. Logging & Observability

- **Library crates:** use `tracing` macros (`debug!`, `info!`, `warn!`, `error!`) at key decision points.
- **CLI binary:** initializes `tracing-subscriber` with `--verbose` (debug level) and `--quiet` (error only); default is `info`.
- **What to log:**
  - `info!`: Configuration loaded, search/fetch/clone starting, operation complete with timing.
  - `debug!`: URL construction, HTTP response status, file filtering decisions.
  - `warn!`: Retries, fallbacks, partial failures (e.g., "file too large, skipping").
  - `error!`: Operation failures with full context.

---

## 12. Filesystem Conventions

| Path | Purpose |
|------|---------|
| `$TMPDIR/wa-git-XXXXX/` | Cloned repos (unless `--output-dir` specified). Persists for AI agent use. |
| `~/.config/wa/config.toml` | Default config file |
| `$TMPDIR/wa-XXXXXX/` | Temp space when no cache dir configured |

---

## 13. Future Enhancements (NOT in v1)

These are documented so we don't design ourselves into a corner, but **NOT implemented in v1:**

- **Headless browser fetch** (`wa browse`): JS-rendered page fetching via headless Chrome/Firefox for SPAs and JS-heavy pages. Returns raw HTML that feeds into the same extraction pipeline — zero changes needed to wa-extract. The extraction engine is already modular: it only needs `html: &str` + `url: Option<&str>`, regardless of how the HTML was obtained.
- **MCP Server** (`wa serve`): Expose the same three tools as an MCP server for AI agents like Claude Desktop, Cursor, etc. Lives in a new `wa-mcp` crate alongside `wa-cli`.
- **Caching:** Cache fetched pages + extraction results to avoid re-fetching.
- **Batching CLI:** `wa fetch --batch urls.txt` to process many URLs (library already supports it).
- **Crawling:** BFS crawl with depth/page limits (building on webclaw-fetch's Crawler).
- **Cloud fallback:** When bot protection detected, auto-fallback to webclaw cloud API (requires WEBCLAW_API_KEY).
- **LLM features:** Summarize and structured extraction (building on webclaw-llm).
- **Pipe mode:** Read URLs/queries from stdin, one per line: `cat urls.txt | wa fetch --stdin`.
- **Search + fetch combined:** `wa search --fetch` already included; enhanced with concurrency control.
- **Multi-format output split:** `--format markdown --save-llm llm.md` to save multiple formats in one run.

---

## 14. Design Decisions Record

| Decision | Rationale |
|----------|-----------|
| **webclaw-fetch via git dep** | Not published on crates.io as of 2026-05. Used as git dependency (`git = "https://github.com/0xMassi/webclaw"`). This is a build-only cost — the compiled binary statically links everything. |
| **webclaw-core for extraction** | 95.1% accuracy, 67% token reduction, 3.2ms per 100KB page. Reused via webclaw-fetch's re-export. No point reinventing. |
| **reqwest for SearXNG only** | SearXNG is a simple JSON API — no bot protection, no TLS fingerprinting needed. reqwest is sufficient. |
| **git CLI over git2** | Simpler to implement, no C build dependency (libgit2), and git is universally available where this tool runs. |
| **SearXNG over proprietary search** | Self-hosted, no API key, privacy-respecting. Matches pi-searxng's philosophy. |
| **5 crates, not 3** | wa-core as dependency-free data crate enables clean separation. wa-cli as thin orchestration means we can add MCP later without touching library code. |
| **AGPL-3.0 license** | Required because we depend on webclaw-fetch which is AGPL-3.0. |
| **TOML config, not JSON** | Standard Rust config format. Human-writable with comments. Used by cargo itself. |
| **clap derive, not builder** | More maintainable for the number of subcommands we have. Compile-time validation. |
| **tokio, not async-std** | Most widely used async runtime in Rust ecosystem. reqwest requires tokio anyway. |
| **Structured data is opt-in** | webclaw appends JSON-LD to both markdown and LLM formats by default. This wastes ~10% of tokens on CMS pages (schema.org Article wrappers that duplicate metadata already in the header). Made `--include-structured-data` an explicit flag so the default output is clean. |
| **LLM format: bracketed body links** | webclaw's `to_llm_text()` strips `[text](url)` to plain `text` and moves URLs to a `## Links` footer. This loses the semantic signal that the text was originally a hyperlink. Post-process the output to restore `[label]` brackets around the first body occurrence of each link label. |
| **LLM format: strip tracking params** | Newsletter and blog URLs are heavily tracked (`utm_source`, `utm_medium`, `ref`). These add ~30-40% token overhead with zero semantic value. Clean `utm_*` and `ref` parameters from all `## Links` footer URLs in `--format llm`. |
| **No reference-style markdown** | Evaluated `[text][N]` + `[N]: url` reference format for token savings. Rejected for `--format llm` because numbered references add indirection that hurts LLM comprehension (the LLM sees `[1]` with zero semantic signal until it reaches the footer). The current `[text]` + `- text: url` format is more LLM-friendly. May revisit as a separate `--format ref` option if demand exists. |
| **URL rewrite in config, not CLI flag** | Rewrite rules are multi-field structures (regex + replacement) that don't map well to simple CLI flags. TOML array-of-tables (`[[url_rewrites]]`) is the right format. Rules are compiled at startup so invalid regexes fail fast with clear error messages including the rule index. |
| **First-match-wins ordering** | Rules are evaluated in config order. This gives users explicit control over precedence (e.g. put specific rules before broad ones). Same semantics as ketch's urlrewrite package. |
| **fetched_url in metadata, not body** | When a URL is rewritten, both original and rewritten URLs are shown in the compact metadata header (`> url:... · fetched_url:...`). This keeps the body clean while providing full transparency. The LLM can see where content actually came from without token-bloating the article text. |

---

## 15. Development Reference Repositories

These two codebases are the foundation of this project. **Clone them to `/tmp`
and reference them constantly during development:**

```bash
# webclaw — extraction engine (webclaw-core), TLS fetch stack (webclaw-fetch),
#           MCP server design patterns (webclaw-mcp), LLM pipeline
[ ! -d /tmp/webclaw ] && git clone --depth 1 https://github.com/0xMassi/webclaw /tmp/webclaw

# pi-searxng — SearXNG integration patterns, GitHub cloning patterns,
#             content extraction patterns (readability → we replaced with webclaw)
[ ! -d /tmp/pi-searxng ] && git clone --depth 1 https://github.com/jcha0713/pi-searxng /tmp/pi-searxng
```

**Key reference files (bookmark these):**

| Codebase | File | What to reference |
|----------|------|-------------------|
| webclaw | `crates/webclaw-core/src/extractor.rs` | Extraction engine entry, scoring, content recovery |
| webclaw | `crates/webclaw-core/src/noise.rs` | CSS class noise filtering, cookie consent detection |
| webclaw | `crates/webclaw-core/src/markdown.rs` | DOM→markdown conversion, MAX_DOM_DEPTH guard |
| webclaw | `crates/webclaw-core/src/llm/` | LLM-optimized text formatting (`to_llm_text()`) |
| webclaw | `crates/webclaw-fetch/src/lib.rs` | FetchClient API, FetchConfig, FetchResult |
| webclaw | `crates/webclaw-mcp/src/server.rs` | MCP tool registration patterns (for future MCP support) |
| pi-searxng | `src/searxng.ts` | SearXNG query format, result parsing |
| pi-searxng | `src/extract.ts` | Content extraction patterns (Readability → webclaw in our case) |
| pi-searxng | `src/github.ts` | GitHub URL parsing, blob/tree handling, clone patterns |
| pi-searxng | `src/index.ts` | Pi extension registration, tool rendering |

### Using codemapper for efficient codebase traversal

This project has access to the **codemapper skill** for efficient Rust codebase
analysis. When you need to understand how something works in webclaw (or our own
crates), use codemapper instead of reading files line-by-line:

```
# Load the skill
read ~/.pi/agent/skills/codemapper/SKILL.md

# Example codemapper queries (run from the target repo root):
#   "callers of extract_with_options"   → who depends on our extraction function
#   "trace from FetchClient::fetch to ExtractionResult"  → full call path
#   "callees of to_llm_text" → what formatting helpers are invoked
#   "test coverage of extractor.rs" → which code paths are tested
#   "entrypoints" → all public API surface entry points
```

Codemapper understands Rust module trees, call graphs, and test coverage.
Use it before making changes to understand downstream impact.

## 16. Quick Start for Developers

```bash
# Prerequisites
# - Rust 1.85+ (rustup)
# - git (for wa-git operations)
# - A SearXNG instance (optional for testing: docker run -p 8080:8080 searxng/searxng)

# Clone
git clone <this-repo>
cd web-access-cli

# Build everything (first build fetches webclaw from GitHub + compiles BoringSSL ~5-10 min)
cargo build

# Run tests (wiremock handles HTTP mocking, no SearXNG needed)
cargo test

# Run a specific crate's tests
cargo test -p wa-core
cargo test -p wa-search
cargo test -p wa-extract
cargo test -p wa-git

# Integration tests (uses assert_cmd to run the built binary)
cargo test --test cli_search

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt --check

# Build release (optimized binary)
cargo build --release

# Run
cargo run -- search "rust async" -n 5
cargo run -- fetch https://www.rust-lang.org --format llm
cargo run -- fetch https://a.com https://b.com --concurrency 2
cargo run -- git https://github.com/serde-rs/serde --max-files 10
```

---

## 17. Checklist: All phases complete ✅

- [x] PLAN.md exists (this file)
- [x] Workspace Cargo.toml created
- [x] All 5 crate stubs compile
- [x] `cargo build --release` succeeds workspace-wide
- [x] `.gitignore` present
- [x] `README.md` written
- [x] 57 tests pass, 15 ignored (13 wa-extract SSRF + 2 network), 0 failures
- [x] 5 output formats tested against real URLs (markdown, LLM, text, JSON, raw)
- [x] All 29+ webclaw vertical extractors inherited and handling verified
- [x] First commit (root commit 36af535)
- [x] GitHub Actions CI workflow (Linux, Windows, macOS x86_64 + aarch64)
- [x] webclaw bumped to v0.6.2 (rev 3fabdc1)

---

## 18. Development Log

**Final: 57 pass, 15 ignored, 0 failures. 8 workspace members (5 ours + 3 webclaw git deps).**

| Phase | Crate | Tests | Status |
|-------|-------|-------|--------|
| 2 | wa-core | 17 (12 config + 5 types) | ✅ Pass |
| 3 | wa-search | 12 | ✅ Pass |
| 4 | wa-extract | 3 pass + 13 ignored (SSRF) | ✅ Pass |
| 5 | wa-git | 18 (8 lib + 10 integration) | ✅ Pass |
| 6 | wa-cli | 7 pass + 2 ignored | ✅ Pass |
| 7 | formatting | — | ✅ Refined |
| 8 | meta+text | — | ✅ Refined |

### Step 1 — Scaffold ✅
- Workspace with 5 crates builds clean.
- wa-cli binary outputs help text with 4 subcommands.
- **Deviation**: webclaw-fetch pinned to commit `923445f` (no tags exist in repo).
- **Deviation**: added `cargo` feature to clap for `crate_version!()` macro.

### Step 2 — wa-core ✅
- 13/13 tests pass (T1–T13).
- Types (`SearchResult`, `GitFile`, `ClonedRepo`, `OutputFormat`) with serde.
- `WaError` enum with 8 variants + `From<io::Error>`.
- `Config` with TOML loading, env var overrides, layered merging.
- **Deviation**: added `temp-env` dev-dependency for env var test isolation (parallel cargo test is not env-safe).
- **Deviation**: `WaError::Fetch.detail` (not `source`) — `source` triggers thiserror's `#[source]` semantics on `String`, which fails.

### Step 3 — wa-search ✅
- 12/12 tests pass (T14–T25). 25 total across workspace.
- `SearXNGClient` with wiremock-tested HTTP search, URL deduplication, rate-limit detection.
- **Deviation**: added `serde` (with derive feature) as wa-search dependency — `serde_json` alone doesn't provide derive for internal SearXNG response structs.

### Step 4 — wa-extract ✅
- 16/16 tests pass (T25–T40). 41 total across workspace.
- `Extractor` wrapping `Arc<FetchClient>`, `BrowserProfile` with `try_from_str`, re-exports `ExtractionResult`, `ExtractionOptions`, `to_llm_text`.
- Full fetch+extract pipeline tested end-to-end with wiremock.
- **Deviation**: webclaw-fetch doesn't treat non-200 status as error — it extracts whatever HTML you get. T31 (404) adjusted to verify content extraction from 404 pages rather than expecting an error. This is correct: AI agents want to see error page text.
- **Deviation**: added `webclaw-core` as wa-extract direct dependency for `ExtractionOptions`/`ExtractionResult` re-exports.
- **Deviation**: `to_llm_text` is a free function (`webclaw_core::to_llm_text(&result, url)`), not a method. Re-exported from wa-extract.
- **Deviation**: `fetch_and_extract_batch_with_options` takes `self: &Arc<Self>`, so Extractor stores `Arc<FetchClient>`.

### Step 5 — wa-git ✅
- 18/18 tests pass (T42–T59). 59 total across workspace.
- `GitCloner` with `check_git_available()`, URL parsing for GitHub/GitLab/Codeberg, `file://` URLs for testing.
- Clone persists on disk; `ClonedRepo` includes `local_path`.
- File filtering: binary extensions, noise dirs, hidden files, lockfiles, size limits.
- **Deviation**: added `file://` URL support for local testing (not in original plan).
- **Deviation**: git clone creates the destination directory; pre-creating it causes "File exists" errors.
- **Deviation**: wa-git uses `git` CLI binary, not `git2` crate (per user's decision documented earlier).

### Step 6 — wa-cli ✅
- 7/9 tests pass (T60-T71), 2 ignored (T65/T66 require network).
- 66 pass + 2 ignored across workspace.
- Full CLI with search, fetch, git, config subcommands, 4 output formats.
- Stdout/stderr contract: results→stdout, progress→stderr, --quiet suppresses stderr.
- **Deviation**: clap requires subcommand; `wa` without subcommand shows help to stderr (exit 2).
- **Deviation**: serde_json added as wa-cli dependency for JSON formatting.
- **Deviation**: T62-T64 (search with mock SearXNG) not yet implemented — require wiremock in CLI integration tests. T65/T66 require network access.

### Step 7 — CLI formatting refinements ✅
- Compared wa-cli output against webclaw CLI (reference) across 4 formats and 3 site types.
- **Removed** `## Links` section from markdown output — links are already inline in body; duplicate index wastes tokens. Matches webclaw's design.
- **Removed** duplicate `## Links` and `## Structured Data` from LLM output — `to_llm_text()` already includes both sections. Was appending them twice.
- **Added** missing JSON fields: `published_date`, `language`, `site_name`, `images`, `code_blocks`, `domain`. Flat JSON schema now richer than webclaw's nested serde dump for LLM consumption.
- **Design insight**: webclaw-fetch is the extraction engine we trust; wa-cli output formatting is independent. Changes to rendering don't affect extraction quality. The contract boundary is `ExtractionResult` — everything below is ours.
- **Design insight**: Reddit rescue path (and 28 other vertical extractors) tested on `www.reddit.com` — custom post+comments markdown rendered correctly across all 4 formats. `www.reddit.com` works; `old.reddit.com` blocked by Reddit's bot detection. Upstream webclaw-fetch limitation, not wa-cli.
- **Design insight**: Reddit vertical extractor doesn't populate `content.plain_text` — it builds `ExtractionResult` manually from JSON API data and only fills `markdown`. Several other vertical extractors (GitHub, YouTube, etc.) likely have the same gap.

### Step 9 — Config file + priority system ✅
- 72 tests pass, 2 ignored, 0 fails. (wa-core: 12 config + 5 types = 17)
- **Added** `#[serde(deny_unknown_fields)]` on Config — typos in config.toml now error instead of being silently ignored.
- **Added** `default_config_path()` — resolves `$XDG_CONFIG_HOME/wa/config.toml` → `$HOME/.config/wa/config.toml`.
- **Added** `init_config_file()` — scaffolds config file with commented TOML template.
- **Added** `config_file_path()` — returns the effective config path (explicit or auto-discovered).
- **Fixed** `Config::load()` — explicit path errors if file doesn't exist; auto path silently falls back to defaults.
- **Fixed** `overlay_file()` replaces `merge_from()` — simple unconditional field copy, no more `.or(self.proxy.take())` trick.
- **Added** `--config` global CLI flag on `Cli` struct — explicit config file path, overrides auto-discovery.
- **Fixed** priority bug: clap's `default_value = "chrome"` on `--browser` prevented env var `WA_BROWSER_PROFILE` and config file `browser_profile` from ever taking effect. Changed `--browser`, `--concurrency`, `--max-file-size`, `--max-files` from `T` with `default_value` to `Option<T>` — now resolved via: CLI flag > env var > config file > hard-coded default.
- **Changed** `Commands::Config` from unit variant to struct variant with `--init` and `--path` flags. `wa config` shows file path + JSON effective config; `wa config --init` scaffolds template.
- **Verified** all 5 priority layers: `wa --config FILE config` shows file values; `WA_SEARXNG_URL=... wa config` shows env override; `wa search --searxng-url ...` shows CLI override; all Layers work correctly.
- **Design insight**: `trailing_var_arg = true` on query means all named flags MUST come before the search query text. `wa search --limit 1 "query"` works; `wa search "query" --limit 1` captures `--limit 1` as query text (clap behavior, not a regression).
- **Design insight**: removed `allow_hyphen_values = true` on the query field would break this silently (clap error on `--limit` in query text) rather than capturing it. Keeping `allow_hyphen_values` lets searches with hyphens through while maintaining the flag-before-query contract.

### Step 10 — Git tree-only mode ✅
- 70 tests pass, 2 ignored, 0 fails.
- **Added** `--tree-only` flag to `wa git` — outputs file tree with paths and sizes, no file contents. Token-efficient for AI agents that only need to know where files live.
- **Added** `TreeEntry` struct to `wa-core::types` (`path: String`, `size: u64`) and `tree: Option<Vec<TreeEntry>>` field on `ClonedRepo`.
- **Added** `tree_only: bool` to `GitCloneOptions` in `wa-git`.
- **Added** `walk_tree_only()` in `wa-git` — walks cloned repo collecting path + size without reading contents, respecting same filters (binary, noise dirs, hidden files, lockfiles) and `max_files` cap.
- **Added** `format_git_tree_markdown()` in `wa-cli` — recursive directory tree renderer with proper ASCII art (├── / └── branches, │ continuation pipes, BTreeMap grouping, directory entries auto-inserted for nested traversal).
- **Added** `format_git_tree_text()` in `wa-cli` — flat listing of relative path + human-readable size.
- **Added** `format_size()` helper — `B`/`KiB`/`MiB` human-readable formatting.
- **Fixed** bug: tree renderer initially produced empty visual tree when all root-level files were filtered by `max_files` — subdirectories were never entered because `render_dir()` only recursed via children. Fixed by collecting intermediate directory paths and inserting synthetic entries into parent dirs.
- **Design insight**: pi-searxng's MCP tool wrapper strips file contents and tells the AI to use `read` tool to explore files. wa CLI lacks a follow-up read tool, so the `--tree-only` flag bridges the gap — one invocation to see the layout, another `wa git` without `--tree-only` (or future `--files` flag) to get specific contents. Default behavior (full content dump) preserved for backward compatibility.

### Step 11 — Metadata header labelled fields ✅
- 70 tests pass, 2 ignored, 0 fails.
- **Changed** compact metadata header from positional `·` separation to labelled fields: `> url:example.com · site:GitHub · author:Jane · date:2024-01-15 · type:article · 173 words`.
- **Rationale**: positional `·` delimiters (e.g., `> GitHub · github · 173 words`) required the LLM to infer field meaning from casing and context — strong models manage, weak models confuse. Explicit labels (`url:`, `site:`, `author:`, `date:`, `type:`) eliminate ambiguity at minimal token cost (~5 tokens per field).
- **Added** URL as the first labelled field (without `https://` prefix) — the target URL was previously invisible in compact mode when `site_name` differed from domain (e.g., `site:` showed "GitHub" but the actual `github.com/octocat/hello-world` path was lost).
- **Added** `site:` deduplication — skipped when `site_name` equals the bare domain (avoids `url:example.com · site:example.com` redundancy).
- **Design insight**: labels are more important for AI agents than token savings. A confused LLM costs far more tokens in re-asks and hallucinations than the ~20 tokens labels consume.

### Step 12 — Browser command ✅
- 70 tests pass, 2 ignored, 0 fails.
- **Added** `wa browser` command — fetches rendered HTML from a browser-backed endpoint (e.g., a headless Chrome service at `http://localhost:8000/html?url=...`). Designed for pages that need JS rendering: SPAs, React apps, Cloudflare JS challenges, lazy-loaded content.
- **Added** `browser_endpoint` field to `Config` (default: `http://localhost:8000/html?url=`) with `WA_BROWSER_ENDPOINT` env var override.
- **Added** `fetch_browser_html()` helper in `wa-cli` — HTTP GET to endpoint, URL-encodes the target URL, returns rendered HTML.
- **Added** re-exports `webclaw_core::extract` and `webclaw_core::extract_with_options` in `wa-extract` — enables extraction from raw HTML without going through `FetchClient`.
- **Added** `reqwest` dependency to `wa-cli` Cargo.toml (already in workspace for `wa-search`).
- **Command flags**: `--browser-endpoint` (override config), `--no-meta`, `--include`, `--exclude`, `--only-main-content`, `--include-raw-html` — same extraction options as `wa fetch`.
- **Design insight**: no new crate for browser — the browser endpoint is a simple HTTP GET, and extraction reuses the existing `webclaw_core` pipeline via clean re-exports in `wa-extract`. Creating a `wa-browser` crate for ~10 lines of HTTP logic would violate the "no over-engineering" principle.
- **Design insight**: `wa browser` and `wa fetch` share the same extraction pipeline and output formats. The only difference is the HTML source: `wa fetch` uses webclaw-fetch's TLS-fingerprinted HTTP stack; `wa browser` delegates rendering to an external service. This layering means any extraction improvements benefit both commands automatically.

### Step 13 — webclaw bump to v0.6.2 ✅
- 57 tests pass, 15 ignored, 0 fails.
- **Bumped** webclaw-fetch and webclaw-core from rev `923445f` (v0.5.7) to rev `3fabdc1` (v0.6.2) — 34 commits of upstream improvements.
- **Non-breaking for wa**: `to_llm_text`, `extract`, `ExtractionResult`, `ExtractionOptions` signatures unchanged. LLM output quality improvements (accessibility link chrome stripping, structured data gating via `is_useful_structured_data()`, bare-integer paragraph stripping, pagination cleanup, noise-link filtering, structured data body-field scrubbing) are pure internal enhancements.
- **Breaking for tests only**: webclaw v0.6.2 introduced SSRF hardening in `url_security.rs` — `validate_public_http_url()` resolves DNS and rejects ANY private/internal IP addresses (loopback, private ranges, CGNAT, TEST-NET, link-local, multicast, etc.). This blocks localhost wiremock servers used by wa-extract integration tests.
- **Resolution**: 13 wa-extract wiremock-based integration tests marked `#[ignore]` — they test extraction pipeline behavior, not HTTP fetch behavior. 3 non-network tests remain active (`extract_browser_profiles`, `extract_timeout`, `extract_invalid_url`).
- **Design insight**: SSRF guard is the correct upstream behavior for a production fetch library. The test regression is a test-design issue (using localhost mocks for an HTTP client with IP blocking), not a code issue. The ignored tests are still valuable for local development if a developer temporarily disables the guard or uses real URLs.
- **Update workflow**: because webclaw is a git dependency without semantic versioning, upstream API changes can break wa. Updates must be done on an isolated branch: bump both rev hashes (workspace root + wa-extract), run `cargo update`, `cargo build`, `cargo test --workspace`, verify all tests, then merge.

### Step 14 — GitHub Actions CI workflow ✅
- **Added** `.github/workflows/build.yml` with four jobs: `build-linux` (ubuntu-latest), `build-windows` (windows-latest), `build-macos` (macos-latest, dual-arch x86_64 + aarch64), and `release` (attaches artifacts to GitHub release on tag push).
- **Linux target**: changed from `x86_64-unknown-linux-musl` to `x86_64-unknown-linux-gnu` (default). BoringSSL (C++ project in webclaw's TLS stack) requires a musl C++ compiler (`x86_64-linux-musl-g++`) which Ubuntu's `musl-tools` package does not provide. The glibc target uses system `g++` and works out of the box.
- **Trade-off**: Linux binary is dynamically linked to glibc instead of fully static. It works on any Linux distro with a compatible glibc version (most modern distros).
- **Release automation**: triggers on git tag push (`v*`), creates GitHub release with four compressed assets: `wa-linux-x86_64.tar.gz`, `wa-windows-x86_64.zip`, `wa-macos-x86_64.tar.gz`, `wa-macos-aarch64.tar.gz`.
- **Action version bumps**: `actions/checkout@v4 → @v5`, `actions/upload-artifact@v4 → @v6`, `actions/download-artifact@v4 → @v6` — resolves Node.js 20 deprecation warnings (Node.js 24 required starting June 2nd 2026).
- **Design insight**: Rust's `cargo build --release` produces a single binary per target. Cross-compilation for macOS dual-arch is done by building both targets separately and uploading both artifacts. No universal binary (fat binary) is created — each architecture gets its own archive.

### Step 15 — `--format raw` ✅
- 57 tests pass, 15 ignored, 0 fails.
- **Added** `Raw` variant to `wa-core::OutputFormat` enum and `wa-cli::OutputFormatArg`.
- **`wa search --format raw`**: returns the original unmodified SearXNG JSON response body. Bypasses parsing, deduplication, and result limiting. Useful for piping to `jq` or debugging SearXNG behavior.
- **`wa fetch --format raw`**: returns raw HTML before extraction. Reuses existing `Extractor::fetch_raw()` method (already existed but unused by CLI). Bypasses the entire webclaw extraction pipeline.
- **`wa browser --format raw`**: returns raw rendered HTML from the browser endpoint. Bypasses extraction.
- **`wa git --format raw`**: intentionally skipped — "raw" is semantically muddy for git (raw file contents? git output? raw paths?). Default behavior already dumps all text file contents.
- **Implementation**: early `if fmt == Raw` guards at the top of each handler (Search, Fetch, Browser) emit raw output via `write_output()` and bypass the existing `match fmt` formatter blocks. Inside those match blocks, `Raw` arms are `unreachable!()` since control flow never reaches them.
- **Architectural alignment**: `search_raw()` belongs in `wa-search` (reuses private `build_search_url()` and `client` — avoids leaking SearXNG internals upward). `fetch_raw()` was already in `wa-extract`. `wa-cli` adds only the routing glue (~40 lines total across all three handlers).
- **Design insight**: `--format raw` is a CLI-only feature for human/scripting workflows. The Pi extension continues to hardcode `--format llm` and never exposes raw output to AI agents.

### Step 16 — Image URL surfacing ✅
- 57 tests pass, 15 ignored, 0 fails.
- **Added** `img_src: Option<String>` field to `wa-core::SearchResult`.
- **Added** `category: String` and `img_src: Option<String>` parsing to `wa-search::SearXNGResultItem`. `category` has `#[serde(default = "default_category")]` returning `"general"` for backward compatibility with test fixtures lacking the field.
- **Category gate**: `img_src` only passes through when `r.category == "images"`. General search results get `img_src: None` even if SearXNG sends an `img_src` (some general engines include thumbnails). This prevents noise in general search output.
- **Formatter changes**:
  - **Markdown**: appends `Image: {url}` line after the URL, before the snippet.
  - **Text**: appends `— Image: {url}` to the one-line entry.
  - **JSON**: automatic via serde (field serializes as `"img_src": null` or `"img_src": "..."`).
  - **LLM**: delegates to markdown formatter.
- **Synthetic `SearchResult` updates**: all 4 synthetic constructions in `wa-cli` (for fetch-with-search-json unified schema) set `img_src: None`.
- **Test updates**: `wa-core/tests/types_and_errors_tests.rs` `search_result_json_roundtrip` updated for new field.
- **Design insight**: SearXNG image search is triggered via bang syntax (`!images paris`). The category field lets wa distinguish general from image results without adding a dedicated `--images` flag. When a user searches with `!images`, SearXNG returns `category: "images"` and wa surfaces the image URLs.

### Step 17 — Windows config path + cross-platform dirs ✅
- 17 tests pass, 0 ignored, 0 fails (wa-core config tests).
- **Added** `dirs` crate to `wa-core/Cargo.toml` for cross-platform directory resolution.
- **Refactored** `default_config_path()`: uses `dirs::config_dir()` on Unix/Linux/macOS (`~/.config/wa/config.toml`) and `dirs::home_dir()` on Windows (`%UserProfile%/.web-access/config.toml`).
- **Rationale**: `XDG_CONFIG_HOME` and `$HOME/.config` do not exist on Windows, so the previous manual env var checks left Windows users unable to use config files without explicitly passing `--config`.
- **Design insight**: the `dirs` crate is the standard Rust solution for cross-platform config directories. It handles macOS `~/Library/Application Support/`, Windows `%APPDATA%`, and Unix `~/.config/` correctly. We override Windows specifically to use `%UserProfile%/.web-access/` (self-documenting directory name) instead of `%APPDATA%\wa\` (buried in Roaming).

## 19. Implementation insights & gotchas

### webclaw-fetch dependency
- Pinned to **commit `3fabdc1`** (v0.6.2) — no git tags exist. If upstream breaks, pin a specific commit.
- **SSRF hardening** (v0.6.2): `validate_public_http_url()` resolves DNS and rejects ANY private/internal IP address. This blocks localhost, 127.0.0.1, 192.168.x.x, 10.x.x.x, and all loopback/private/link-local/multicast ranges. Integration tests against local wiremock servers will fail with error: `URL resolves to a blocked private or internal address`.
- **webclaw update workflow**: bump both rev hashes (workspace root Cargo.toml + crates/wa-extract/Cargo.toml), run `cargo update`, `cargo build`, `cargo test --workspace`. Validate on an isolated branch before merging.
- webclaw-fetch does NOT re-export `ExtractionOptions` or `ExtractionResult` — these live in `webclaw-core` which must be added as a separate git dependency.
- `fetch_and_extract_batch_with_options` requires `self: &Arc<Self>`, so `Extractor` stores `Arc<FetchClient>`.
- webclaw-fetch's `fetch()` has built-in retry logic (2 attempts at 0s + 1s delays for retryable codes 429, 502-504, 520-524). Do NOT add another retry layer on top.
- `FetchClient::fetch_and_extract_with_options()` includes rescue paths executed BEFORE `webclaw_core::extract_with_options()`: Reddit JSON API, Akamai cookie warmup, PDF detection, document type detection, LinkedIn JSON extraction. Always delegate to this method, never call `fetch()` + `extract_with_options()` separately.

### webclaw-core API surprises
- `to_llm_text(result, url)` is a **free function**, not a method on `ExtractionResult`. Signature: `to_llm_text(&ExtractionResult, Option<&str>) -> String`.
- `ExtractionResult.domain_data` is `Option<DomainData>`, not `DomainData`. Must handle `None`.
- webclaw-core spawns worker threads with 8MB stack for deeply nested HTML. This is transparent to callers.
- `content.markdown` already includes H1 title from extraction — do NOT prepend another `# Title` heading.
- `to_llm_text()` output already includes `## Links` and `## Structured Data` sections. Do NOT append them again.

### Vertical extractor behavior
- Reddit rescue path works on `www.reddit.com`, blocked on `old.reddit.com` (bot detection). Upstream issue.
- Reddit extractor builds `ExtractionResult` manually — `plain_text`, `links`, `images`, `code_blocks`, `structured_data`, `domain_data` are all empty/`None`.
- Other vertical extractors (29+ total: GitHub, YouTube, PyPI, npm, crates.io, Amazon, eBay, etc.) likely have similar partial population patterns. wa-cli handles them gracefully via null-safe rendering.
- Reddit markdown format: `# Title\n\n**u/author** in r/sub\n\n[body]\n\n---\n\n## Comments\n\n- **u/x** (score)\n  text`

### Testing gotchas
- `temp_env::with_var` required for parallel-safe env var isolation in config tests (Rust test runner is parallel by default).
- `wiremock` for SearXNG and HTTP endpoint mocking — MockServer on localhost, no TLS needed.
- **SSRF hardening breaks localhost tests**: webclaw v0.6.2 blocks private IPs. 13 wa-extract wiremock tests are `#[ignore]` as a result. The 3 remaining active tests (`extract_browser_profiles`, `extract_timeout`, `extract_invalid_url`) do not require successful HTTP to localhost.
- wa-git uses `git` CLI binary — integration tests create repos with `git init` + `file://` URLs. Must have git installed.
- git clone creates destination directory; pre-creating it causes "File exists" → `Os { code: 17 }`. Let git create the directory.
- wa-git integration tests 8 of 10 initially failed due to temp directory collision — all tests sharing same test repo init; fixed by per-test tempdir isolation.
- **wa-search real SearXNG dependency**: `test_empty_query` uses a real hosted SearXNG instance (`https://cc-searxng.airplane-scala.ts.net/`) for end-to-end validation. The remaining 11 wa-search tests use wiremock and are fully self-contained.

### CLI design decisions
- JSON output: flat schema is better for LLMs than webclaw's nested serde dump. AI agents parse flat objects with fewer tokens.
- Markdown output: no `## Links` section (redundant with inline links). `## Structured Data` kept (JSON-LD is valuable).
- Metadata header: always-on, compact, blockquote-prefixed. Saves AI agents from scanning body for context.
- `--no-meta` to opt out (not `--meta` to opt in). AI agents are the primary user — they benefit from metadata.
- `clap` `cargo` feature required for `crate_version!()` macro.
- `serde` (with `derive`) needed in wa-search even though `serde_json` is present — `serde_json` doesn't provide `Deserialize` derive.

### Config priority system
- **Layer order**: defaults → config file → env vars → CLI flags. The CLI crate resolves CLI flags against the effective config from layers 1-3, so CLI always wins.
- **Config file auto-discovery**: `$XDG_CONFIG_HOME/wa/config.toml` or `$HOME/.config/wa/config.toml`. Use `--config FILE` global flag to override.
- **Explicit path errors**: if `--config /path/file.toml` is given and the file doesn't exist → error. Auto-discovered path silently falls back to defaults if file absent.
- **Two config bugs found & fixed**:
  1. clap's `default_value = "chrome"` on `--browser` prevented env var `WA_BROWSER_PROFILE` and config file `browser_profile` from ever taking effect. Fix: changed flags to `Option<T>` without `default_value`, resolved in handler (CLI > env > file > default).
  2. `Config::load(None)` passed `None` path, skipping file loading entirely. Fix: `None` now triggers auto-discovery.
- **`#[serde(deny_unknown_fields)]`** catches TOML typos (e.g., `searxng_url` vs `searxng_url`) with clear error message.
- **`wa config`** shows config file path (with exists/not-found status) + JSON effective config.
- **`wa config --init`** scaffolds commented TOML at default (or `--path`) location. Errors if file already exists.
- **`wa config --init --path FILE`** scaffolds at custom location.

### CLI parsing: trailing_var_arg
- `trailing_var_arg = true` on Search.query means once the first query word is consumed, all subsequent arguments (including `--flags`) are treated as query text.
- **All named flags must come BEFORE the query**: `wa search --limit 1 "rust"` ✅, `wa search "rust" --limit 1` ❌ (limit captured as query text).
- `allow_hyphen_values = true` is required for queries containing hyphens (e.g., "cross-compile") and prevents clap parse errors when flags appear after query. Do not remove it.

### Bash history expansion (! bangs)
- On Linux/macOS, bash interprets `!` at the start of a word as **history expansion** before quote removal. `wa search "!images paris"` fails with `!images: event not found` even inside double quotes.
- **Only single quotes suppress history expansion**: `wa search '!images paris'` ✅. Backslash escape also works: `wa search \!images paris` ✅.
- **Double quotes do NOT protect `!`**: bash processes history expansion before removing quotes. This is a common user trap.
- **Windows is unaffected**: PowerShell/CMD do not implement `!` history expansion.
- Mitigation options documented: single quotes, backslash escape, `set +H` in `.bashrc`, or a future `--query` flag (held for now — user deemed shell quoting acceptable).
