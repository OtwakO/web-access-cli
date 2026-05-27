# wa — Web Access CLI for AI Agents

Rust CLI giving AI agents four web capabilities:

| Command | Description |
|---------|-------------|
| `wa search` | Web search via SearXNG, optional per-result extraction |
| `wa fetch` | Fetch URL → extract clean content via webclaw |
| `wa browser` | Fetch via browser-backed rendering endpoint → extract |
| `wa git` | Clone repo → file listing or tree |

All extraction uses webclaw-core (95.1% extraction accuracy, 29+ vertical extractors)
rather than Readability. Output formats: **markdown** (default), `--format llm`
(token-optimised for LLM consumption), `text`, or `json`.

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

## Commands

All commands support global flags: `--quiet`, `--format <fmt>`, `--output PATH`,
`--config PATH`.

### `wa search` — Web Search

```bash
wa search "rust async programming"

# With auto-fetch and extraction of result pages
wa search "rust async" --fetch --fetch-limit 5

# Control result count
wa search "rust async" --limit 20
```

| Flag | Default | Description |
|------|---------|-------------|
| `--fetch` | off | Fetch and extract each result URL |
| `--fetch-limit <n>` | `3` | Max results to fetch (with `--fetch`) |
| `--limit <n>` | `10` | Search results to return |
| `--concurrency <n>` | `4` | Parallel fetches (with `--fetch`) |
| `--searxng-url <url>` | config | Override SearXNG instance |
| `--browser <profile>` | config | chrome, firefox, safari-ios, random |
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

# With raw HTML included in result
wa fetch https://example.com --include-raw-html
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
| `--include-raw-html` | off | Attach raw HTML to result (JSON format) |
| `--include-structured-data` | off | Append JSON-LD structured data appendix |

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
| `--include-raw-html` | off | Attach raw HTML to result (JSON format) |
| `--include-structured-data` | off | Append JSON-LD structured data appendix |

*`wa browser` and `wa fetch` share the same extraction pipeline — only the
HTML source differs.*

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

All commands support `--format <fmt>` with four formats:

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

### Output Contract

| Stream | Purpose |
|--------|---------|
| **stdout** | Clean formatted result |
| **stderr** | Progress messages, warnings |
| `--quiet` | Suppress all stderr |
| `--output PATH` | Write result to file |
| Exit `0` | Success |
| Exit `1` | Error |

---

## Architecture

```
wa-cli  (CLI parsing, output formatting)
 ├── wa-core    (config, types, errors — no I/O)
 ├── wa-search  (SearXNG HTTP client)
 ├── wa-extract (webclaw-fetch wrapper, raw HTML extraction)
 └── wa-git     (git clone + file tree walking)
```

Dependency layering:

```
wa-core ← wa-search  (reqwest)
wa-core ← wa-extract (webclaw-fetch, webclaw-core)
wa-core ← wa-git     (walkdir, git CLI)
wa-core + wa-search + wa-extract + wa-git → wa-cli
```

`wa-core` has zero I/O dependencies — portable to WASM.

## License

**AGPL-3.0** — required because this project depends on
[webclaw-fetch](https://github.com/0xMassi/webclaw) which is also AGPL-3.0
licensed.

Full development history, architecture decisions, and design rationale:
**[PLAN.md](PLAN.md)**.
