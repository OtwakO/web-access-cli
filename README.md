# wa — Web Access CLI for AI Agents

Rust CLI giving AI agents four web capabilities:

| Command | Description |
|---------|-------------|
| `wa search` | Web search via SearXNG, optional per-result extraction |
| `wa fetch` | Fetch URL → extract clean content via webclaw |
| `wa browser` | Fetch via browser-backed rendering endpoint → extract |
| `wa crawl` | BFS or sitemap crawl a single host, extract all pages |
| `wa git` | Clone repo → file listing or tree |

All extraction uses webclaw-core (95.1% extraction accuracy, 29+ vertical extractors)
rather than Readability. Output formats: **markdown** (default), `--format llm`
(token-optimised for LLM consumption), `text`, `json`, or `raw`.

**Recent improvements:** `--format llm` now preserves `[link text]` brackets in the
body (so the LLM knows which text was originally hyperlinked) and strips tracking
parameters (`utm_*`, `ref`) from footer URLs. Structured data (JSON-LD) is now
**opt-in** via `--include-structured-data` for markdown and llm formats.

## Quick Start

```bash
# Prerequisites: Rust 1.85+, git
git clone <this-repo>
cd web-access-cli

cargo build --release
./target/release/wa --help
```

To make `wa` available globally:

```bash
cargo install --path crates/wa-cli
# or symlink:  ln -s $PWD/target/release/wa /usr/local/bin/wa
```

## Configuration

Config is layered — each level overrides the one below:

1. **Config file** at the platform default (auto-discovered, optional)
2. **Environment variables** (`WA_*` prefix)
3. **CLI flags** (highest precedence)

**Config file locations:**
- **Linux / macOS:** `~/.config/wa/config.toml`
- **Windows:** `%UserProfile%\.web-access\config.toml`

Scaffold a config file with commented defaults:

```bash
wa config --init
```

View effective config (after layering):

```bash
wa config
```

Use an explicit config file:

```bash
wa --config /path/to/custom.toml search "query"
```

### Config Fields

```toml
# ~/.config/wa/config.toml
searxng_url = "http://localhost:8080"       # SearXNG instance URL
browser_profile = "chrome"                   # chrome | firefox | safari-ios | random
browser_endpoint = "http://localhost:8000/html?url="  # base URL for wa browser
proxy = "socks5://127.0.0.1:9050"           # SOCKS/HTTP proxy (optional)
fetch_timeout_secs = 12                      # HTTP request timeout
retries = 3                                  # transient failure retries
retry_delay_ms = 500                         # base delay (exponential backoff + 25% jitter)
max_file_size = 102400                       # max bytes per file from git clone
max_files = 100                              # max text files from git clone
max_pages = 100                              # max pages for wa crawl
```

### Environment Variables

| Variable | Config Field |
|----------|-------------|
| `WA_SEARXNG_URL` | `searxng_url` |
| `WA_BROWSER_PROFILE` | `browser_profile` |
| `WA_BROWSER_ENDPOINT` | `browser_endpoint` |
| `WA_PROXY` | `proxy` (empty string = unset) |
| `WA_RETRIES` | `retries` |

---

## URL Rewrite Rules

Transparently rewrite request URLs before any fetch. Applied in `wa fetch`,
`wa browser`, and `wa search --fetch`. Rules are ordered — **first match wins**.

### Config Format

Add `[[url_rewrites]]` tables to `~/.config/wa/config.toml`:

```toml
[[url_rewrites]]
match_regex = '^https?://www\.reddit\.com/(.*)$'
replace = 'https://old.reddit.com/$1'

[[url_rewrites]]
match_regex = '^https?://(www\.)?medium\.com/(.*)$'
replace = 'https://scribe.rip/$2'

[[url_rewrites]]
match_regex = '^https?://twitter\.com/'
replace = 'https://nitter.net/'
```

### How It Works

| Original URL | Rule | Rewritten URL |
|-------------|------|---------------|
| `https://www.reddit.com/r/rust` | `^https?://www\.reddit\.com/(.*)$` → `https://old.reddit.com/$1` | `https://old.reddit.com/r/rust` |
| `https://medium.com/@author/post` | `^https?://(www\.)?medium\.com/(.*)$` → `https://scribe.rip/$2` | `https://scribe.rip/@author/post` |
| `https://twitter.com/elonmusk` | `^https?://twitter\.com/` → `https://nitter.net/` | `https://nitter.net/elonmusk` |
| `https://github.com/torvalds/linux` | *(no rule matches)* | *(unchanged)* |

### Regex Syntax

- `match_regex` uses **Rust regex syntax** (`regex` crate)
- `$1`, `$2`, … reference capture groups
- `^` and `$` anchors are recommended for precise matching
- Double backslashes in TOML: `\.` matches a literal dot

### Output Transparency

When a rewrite is applied, both URLs are shown in the metadata header:

```markdown
> url:www.reddit.com/r/rust · fetched_url:old.reddit.com/r/rust · author:... · 14603 words
```

If no rule matched, only `url:` appears (no extra field).

### Common Recipes

```toml
# Reddit: old.reddit.com serves clean HTML without JS bot wall
[[url_rewrites]]
match_regex = '^https?://www\.reddit\.com/(.*)$'
replace = 'https://old.reddit.com/$1'

# Medium: scribe.rip is a readability proxy
[[url_rewrites]]
match_regex = '^https?://(www\.)?medium\.com/(.*)$'
replace = 'https://scribe.rip/$2'

# Twitter/X: nitter is a privacy frontend
[[url_rewrites]]
match_regex = '^https?://(www\.)?(twitter|x)\.com/'
replace = 'https://nitter.net/'

# Stack Overflow: mobile site is lighter
[[url_rewrites]]
match_regex = '^https?://stackoverflow\.com/questions/(\d+)(/.*)?$'
replace = 'https://stackoverflow.com/questions/$1'
```

### Shell Quoting

Bash history expansion and word splitting are the most common ways a URL gets
mangled before it reaches `wa`:

```bash
# ❌ fails: bash expands !images as history
wa search !images paris

# ✅ single quotes suppress history expansion
wa search '!images paris'

# ❌ fails: bash backgrounds at & and drops y=2
wa fetch https://api.example.com?x=1&y=2

# ✅ single quotes keep the URL intact
wa fetch 'https://api.example.com?x=1&y=2'

# ✅ or use --url-encoded to suppress the warning on an intentional URL
wa fetch --url-encoded 'https://api.example.com?foo'
```

`wa` warns when a positional URL looks truncated (ends with `&` or has a bare
query key like `?foo`). Use `--url-encoded` to silence the warning.

## Commands

All commands support global flags: `--quiet`, `--format <fmt>`, `--output PATH`,
`--config PATH`.

### `wa search` — Web Search

```bash
wa search "rust async programming"

# With auto-fetch and extraction of result pages
wa search --fetch --fetch-limit 5 "rust async"

# Control result count
wa search --limit 20 "rust async"
```

| Flag | Default | Description |
|------|---------|-------------|
| `--fetch` | off | Fetch and extract each result URL |
| `--fetch-limit <n>` | `3` | Max results to fetch (with `--fetch`) |
| `--limit <n>` | `10` | Search results to return |
| `--concurrency <n>` | `4` | Parallel fetches (with `--fetch`) |
| `--searxng-url <url>` | config | Override SearXNG instance |
| `--browser <profile>` | config | chrome, firefox, safari-ios, random |

If SearXNG returns no results while reporting failed upstream engines, `wa search`
retries once. If the retry is still degraded, Markdown/LLM and text return a
prominent warning instead of silently reporting no matches; JSON returns an
empty `results` array plus a structured `search_engines_unavailable` warning.
Raw format remains the untouched SearXNG response.
| `--proxy <url>` | config | SOCKS/HTTP proxy |
| `--no-meta` | off | Omit metadata header from extracted pages |
| `--cookie "k=v"` | none | Cookies (repeatable) |
| `--include-structured-data` | off | Append JSON-LD structured data appendix |

*Note: all named flags must appear before the query text.*

### `wa fetch` — Fetch & Extract

```bash
# Single URL
wa fetch https://example.com

# Multiple URLs (concurrent)
wa fetch https://rust-lang.org https://docs.rs

# With CSS selector filtering
wa fetch https://example.com --include "article" --exclude ".sidebar,nav"

# Include the extractor-selected content HTML alongside normal output
wa fetch https://example.com --include-extracted-html

# Return the complete upstream HTML without content extraction
wa fetch https://example.com --format raw

# Discover API endpoints in page + JS bundles (JSON output)
wa fetch https://api.example.com --endpoints

# Shell-safe URL quoting (bash splits unquoted URLs at &)
wa fetch 'https://example.com?foo=1&bar=2'
```

| Flag | Default | Description |
|------|---------|-------------|
| `URLS...` | required | One or more URLs |
| `--no-meta` | off | Omit metadata header |
| `--browser <profile>` | config | TLS fingerprint profile |
| `--proxy <url>` | config | SOCKS/HTTP proxy |
| `--cookie "k=v"` | none | Cookies (repeatable) |
| `--concurrency <n>` | `4` | Parallel fetches (multi-URL) |
| `--include <selector>` | none | CSS selectors to keep (repeatable) |
| `--exclude <selector>` | none | CSS selectors to strip (repeatable) |
| `--only-main-content` | off | Auto-detect and extract main content only |
| `--include-extracted-html` | off | Append extractor-selected content HTML to normal output |
| `--include-structured-data` | off | Append JSON-LD structured data appendix |
| `--endpoints` | off | Discover API endpoints instead of extracting content |
| `--url-encoded` | off | Suppress shell-splitting warning |

### `wa browser` — Browser-Backed Fetch

Renders pages through a browser endpoint (e.g. headless Chrome service). Use for
JavaScript-heavy pages that need a real DOM: SPAs, React apps, Cloudflare JS
challenges.

```bash
wa browser https://spa.example.com

# Custom endpoint
wa browser https://spa.example.com --browser-endpoint "http://localhost:8000/html?url="
```

| Flag | Default | Description |
|------|---------|-------------|
| `URLS...` | required | One or more URLs |
| `--browser-endpoint <url>` | config | Browser rendering endpoint (target URL appended) |
| `--no-meta` | off | Omit metadata header |
| `--include <selector>` | none | CSS selectors to keep (repeatable) |
| `--exclude <selector>` | none | CSS selectors to strip (repeatable) |
| `--only-main-content` | off | Auto-detect main content |
| `--include-extracted-html` | off | Append extractor-selected content HTML to normal output |
| `--include-structured-data` | off | Append JSON-LD structured data appendix |
| `--url-encoded` | off | Suppress shell-splitting warning |

*`wa browser` and `wa fetch` share the same extraction pipeline — only the
HTML source differs. `--format raw` returns the complete upstream document;
`--include-extracted-html` preserves only the HTML selected by the extractor.*

When a sparse extraction omits a substantially richer semantic content region,
a suspicious single-URL `wa fetch` or any `wa browser` result prepends an
agent-visible warning to the returned context. Multi-URL `wa fetch` intentionally
skips diagnostic probes. Markdown and LLM use a warning callout, text uses a
warning banner, and JSON adds a structured `warnings` array to the affected
result. The warning remains under `--quiet`; raw passthrough output stays
byte-for-byte unchanged.

### `wa crawl` — Crawl a Website

BFS or sitemap-based crawling with same-host restriction. Extracts content
from every page discovered.

```bash
# BFS crawl from a seed URL (depth 3, concurrency 4)
wa crawl https://example.com

# Limit depth and concurrency
wa crawl https://example.com --depth 2 --concurrency 8

# Sitemap-based crawl (falls back to BFS if sitemap is empty)
wa crawl https://example.com/sitemap.xml --sitemap

# Sitemap from host root (auto-discovers robots.txt + common paths, falls back to BFS)
wa crawl https://example.com --sitemap

# Cap pages and use auto-discovered sitemap
wa crawl https://docs.rs --sitemap --max-pages 20 --format llm

# Filter to specific paths only
wa crawl https://docs.rs --depth 2 --allow "/tokio" --allow "/async"

# Include only docs paths via glob
wa crawl https://example.com --include "/docs/**" --include "/blog/**"

# Exclude paths by glob
wa crawl https://example.com --exclude "/admin/**" --exclude "/cdn/**"

# Exclude paths by regex
wa crawl https://example.com --deny ".*login.*"

# Output as LLM-optimized text
wa crawl https://example.com --format llm --output site-content.md
```

| Flag | Default | Description |
|------|---------|-------------|
| `URL` | required | Seed URL (or sitemap URL with `--sitemap`) |
| `--depth` | `3` | Max BFS depth (0 = seed only) |
| `--concurrency` | `4` | Parallel fetch workers |
| `--max-pages` | config/`100` | Maximum pages to fetch (safety cap) |
| `--allow` | none | Path substrings URLs must contain (repeatable) |
| `--include` | none | Glob patterns for URL paths to include, e.g. `/docs/**` (repeatable) |
| `--deny` | none | Regex patterns to reject URLs (repeatable) |
| `--exclude` | none | Glob patterns for URL paths to exclude, e.g. `/admin/**` (repeatable) |
| `--sitemap` | off | Treat seed as XML sitemap or auto-discover from host root; falls back to BFS if empty |
| `--no-meta` | off | Omit metadata header from each page |
| `--include-structured-data` | off | Append JSON-LD to each page |
| `--url-encoded` | off | Suppress shell-splitting warning |

**Behavior:**
- Only crawls URLs on the **same host** as the seed
- Discovers links from the full raw HTML (every `<a href>`, including sidebar/menu links that content extraction skips)
- Applies URL rewrite rules from config before fetching
- Deduplicates by normalized URL (strips fragments, `utm_*` params, trailing `/`)
- Failed pages are silently skipped (other pages still returned)
- `--max-pages` caps total pages fetched and the in-memory link frontier
- `--sitemap` falls back to BFS from the host root if the sitemap is empty or fails to parse

### `wa git` — Git Repository

```bash
# Clone and show full file contents
wa git https://github.com/octocat/hello-world

# File tree only (paths + sizes, no content) — token-efficient for AI agents
wa git --tree-only https://github.com/octocat/Spoon-Knife

# Limit files and size
wa git https://github.com/serde-rs/serde --max-files 20 --max-file-size 51200

# Branch / sub-path URLs work
wa git https://github.com/serde-rs/serde/tree/dev
wa git https://github.com/serde-rs/serde/blob/main/src/lib.rs
```

Supported hosts: `github.com`, `gitlab.com`, `codeberg.org`, `git@` SSH URLs,
`file://` local repos. Gist URLs are not supported (use `wa fetch` for gists).

| Flag | Default | Description |
|------|---------|-------------|
| `URL` | required | Repository URL |
| `--tree-only` | off | Show file tree (paths + sizes), skip contents |
| `--max-file-size <bytes>` | config | Max bytes per file |
| `--max-files <n>` | config | Max text files to collect |
| `--output-dir <path>` | `/tmp/wa-git-<hex>/` | Clone destination |

### `wa config` — Config Management

```bash
# Show effective config after layering
wa config

# Scaffold a fresh config file
wa config --init

# Scaffold at a custom path
wa config --init --path /path/to/config.toml
```

---

## Output Formats

All commands support `--format <fmt>` with five formats:

### Markdown (`--format markdown`) — default

Clean markdown with content, inline links, and optional metadata header.
Multi-result outputs separated by `---`.

**Note:** JSON-LD structured data is **not** appended by default. Use
`--include-structured-data` to append a `## Structured Data` block with
schema.org metadata.

### LLM (`--format llm`)

Token-optimised for LLM consumption, with `wa`-specific post-processing on top
of webclaw-core's `to_llm_text()`:

- **Deduplicated paragraphs**, collapsed whitespace, images stripped
- **Link text preserved in body** as `[label]` brackets — the LLM knows which
  text was originally a hyperlink without reading the full URL
- **Tracking parameters stripped** from `## Links` footer URLs (`utm_source`,
  `utm_medium`, `utm_campaign`, `utm_content`, `utm_term`, `ref`)
- **Structured data appendix** only included when `--include-structured-data` is passed

### Text (`--format text`)

Plain text `Title — URL` lines (search) or plain content (fetch). Falls back to
markdown when extraction provides no plain text.

### JSON (`--format json`)

Flat JSON schema with all extracted data: metadata, markdown, plain text, links,
images, code blocks, structured data, and domain type. Errors returned in-band
with `status: "error"`.

### Raw (`--format raw`)

Return the raw HTTP response body without extraction. For `wa fetch` this is the
raw HTML; for `wa search` this is the raw SearXNG JSON response. Useful when you
want the original source rather than processed content.

### Output Contract

| Stream | Purpose |
|--------|---------|
| **stdout** | Formatted result, including extraction-quality warnings when applicable |
| **stderr** | Operational progress and shell-input warnings |
| `--quiet` | Suppress stderr; result-quality warnings remain in stdout |
| `--output PATH` | Write result to file |
| Exit `0` | Success |
| Exit `1` | Error |

---

## Architecture

```
wa-cli  (CLI parsing, output formatting)
 ├── wa-core    (config, types, errors, URL rewriter — no I/O)
 ├── wa-search  (SearXNG HTTP client)
 ├── wa-extract (webclaw-fetch wrapper, raw HTML extraction)
 ├── wa-crawl   (BFS crawler, link extraction, sitemap parser)
 └── wa-git     (git clone + file tree walking)
```

Dependency layering:

```
wa-core ← wa-search  (reqwest)
wa-core ← wa-extract (webclaw-fetch, webclaw-core)
wa-core ← wa-crawl   (wa-extract, scraper, quick-xml)
wa-core ← wa-git     (walkdir, git CLI)
wa-core + wa-search + wa-extract + wa-crawl + wa-git → wa-cli
```

`wa-core` has zero I/O dependencies — portable to WASM.

## License

**AGPL-3.0** — required because this project depends on
[webclaw-fetch](https://github.com/0xMassi/webclaw) which is also AGPL-3.0
licensed.

Full development history, architecture decisions, and design rationale:
**[PLAN.md](PLAN.md)**.
