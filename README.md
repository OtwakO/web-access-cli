# wa — Web Access CLI for AI Agents

A Rust CLI that gives AI agents three capabilities:

- **`wa search`** — Web search via SearXNG (self-hosted, privacy-respecting)
- **`wa fetch`** — Fetch URLs and extract clean content via webclaw (95.1% extraction accuracy)
- **`wa git`** — Clone GitHub/GitLab/Codeberg repos and list text files

Every page is extracted through webclaw-core's multi-signal scoring engine, not Readability.
Output formats: markdown, llm (token-optimized), plain text, JSON.

## Quick Start

```bash
# Prerequisites: Rust 1.85+, git
cargo build --release
./target/release/wa --help
```

## License

AGPL-3.0 — required because this project depends on webclaw-fetch (AGPL-3.0).
See [PLAN.md](docs/PLAN.md) for architecture and development guide.
