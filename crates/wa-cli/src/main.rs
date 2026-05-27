//! wa — Web Access CLI for AI Agents.
//!
//! # I/O Stream Contract
//! - **stdout**: formatted results (markdown, llm, text, json)
//! - **stderr**: progress messages, warnings, errors
//! - **--quiet**: suppress all stderr output (AI agent mode)
//! - Exit codes: 0 = success, 1 = runtime error, 2 = usage error

use clap::{Parser, Subcommand, crate_version};
use wa_core::types::OutputFormat;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "wa", version = crate_version!(), about = "Web Access CLI for AI Agents")]
struct Cli {
    /// Suppress all progress output (AI agent mode)
    #[arg(long, global = true)]
    quiet: bool,

    /// Output format: markdown, llm, text, or json
    #[arg(long, global = true, default_value = "markdown")]
    format: OutputFormatArg,

    /// Write result to FILE instead of stdout
    #[arg(short = 'o', long, global = true)]
    output: Option<String>,

    /// Path to config file (overrides auto-discovered ~/.config/wa/config.toml)
    #[arg(long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search the web via SearXNG
    Search {
        /// Search query (combines all arguments)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        query: Vec<String>,

        /// Fetch and extract content from each result URL
        #[arg(long)]
        fetch: bool,

        /// Max number of results to fetch when --fetch is active
        #[arg(long, default_value = "3")]
        fetch_limit: usize,

        /// Concurrency for parallel fetching
        #[arg(long)]
        concurrency: Option<usize>,

        /// Number of search results to return
        #[arg(long, default_value = "10")]
        limit: usize,

        /// SearXNG instance URL (overrides config/env)
        #[arg(long)]
        searxng_url: Option<String>,

        /// Browser profile: chrome, firefox, safari-ios, random
        #[arg(long)]
        browser: Option<String>,

        /// Proxy URL (SOCKS or HTTP)
        #[arg(long)]
        proxy: Option<String>,

        /// Cookies as "name=value" (can be repeated)
        #[arg(long = "cookie")]
        cookies: Vec<String>,

        /// Omit the compact metadata header from fetched results
        #[arg(long)]
        no_meta: bool,

        /// Include JSON-LD structured data appendix in markdown/llm output
        #[arg(long)]
        include_structured_data: bool,
    },

    /// Fetch and extract content from URLs
    Fetch {
        /// One or more URLs to fetch
        #[arg(required = true)]
        urls: Vec<String>,

        /// Omit the compact metadata header from output
        #[arg(long)]
        no_meta: bool,

        /// Browser profile: chrome, firefox, safari-ios, random
        #[arg(long)]
        browser: Option<String>,

        /// Proxy URL
        #[arg(long)]
        proxy: Option<String>,

        /// Cookies as "name=value" (can be repeated)
        #[arg(long = "cookie")]
        cookies: Vec<String>,

        /// Concurrency for multiple URLs
        #[arg(long)]
        concurrency: Option<usize>,

        /// CSS selectors to include (only these elements)
        #[arg(long)]
        include: Vec<String>,

        /// CSS selectors to exclude
        #[arg(long)]
        exclude: Vec<String>,

        /// Extract only the main content area
        #[arg(long)]
        only_main_content: bool,

        /// Include raw HTML in the result
        #[arg(long)]
        include_raw_html: bool,

        /// Include JSON-LD structured data appendix in markdown/llm output
        #[arg(long)]
        include_structured_data: bool,
    },

    /// Clone a git repository and list text files
    Git {
        /// Repository URL
        url: String,

        /// Only show file tree (paths + sizes), not contents
        #[arg(long = "tree-only")]
        tree_only: bool,

        /// Max file size in bytes
        #[arg(long)]
        max_file_size: Option<usize>,

        /// Max number of files to read
        #[arg(long)]
        max_files: Option<usize>,

        /// Output directory for the clone
        #[arg(long)]
        output_dir: Option<String>,
    },

    /// Show current configuration, or --init to scaffold a new config file
    Config {
        /// Scaffold a new config file with commented defaults
        #[arg(long)]
        init: bool,

        /// Config file path for --init (overrides default location)
        #[arg(long, requires = "init")]
        path: Option<String>,
    },

    /// Fetch HTML via a browser-backed rendering endpoint and extract content
    Browser {
        /// One or more URLs to fetch through the browser
        #[arg(required = true)]
        urls: Vec<String>,

        /// Browser endpoint URL (default: http://localhost:8000/html?url=)
        #[arg(long)]
        browser_endpoint: Option<String>,

        /// Omit the compact metadata header from output
        #[arg(long)]
        no_meta: bool,

        /// CSS selectors to include (only these elements)
        #[arg(long)]
        include: Vec<String>,

        /// CSS selectors to exclude
        #[arg(long)]
        exclude: Vec<String>,

        /// Extract only the main content area
        #[arg(long)]
        only_main_content: bool,

        /// Include raw HTML in the result
        #[arg(long)]
        include_raw_html: bool,

        /// Include JSON-LD structured data appendix in markdown/llm output
        #[arg(long)]
        include_structured_data: bool,
    },
}

/// CLI-level output format that converts to wa_core::OutputFormat.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormatArg {
    Markdown,
    Llm,
    Text,
    Json,
    Raw,
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(arg: OutputFormatArg) -> Self {
        match arg {
            OutputFormatArg::Markdown => OutputFormat::Markdown,
            OutputFormatArg::Llm => OutputFormat::Llm,
            OutputFormatArg::Text => OutputFormat::Text,
            OutputFormatArg::Json => OutputFormat::Json,
            OutputFormatArg::Raw => OutputFormat::Raw,
        }
    }
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

/// Format search results as markdown.
fn format_search_markdown(results: &[wa_core::types::SearchResult]) -> String {
    let mut out = String::new();
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!("{}. **{}**\n", i + 1, r.title));
        out.push_str(&format!("   {}\n", r.url));
        if let Some(ref img) = r.img_src {
            out.push_str(&format!("   Image: {}\n", img));
        }
        if !r.snippet.is_empty() {
            out.push_str(&format!("   > {}\n", r.snippet));
        }
        out.push('\n');
    }
    out
}

/// Format search results as JSON.
fn format_search_json(results: &[wa_core::types::SearchResult]) -> String {
    serde_json::to_string_pretty(results).unwrap_or_else(|_| "[]".into())
}

/// Format search + extracted results as JSON.
fn format_search_fetch_json(
    results: &[wa_core::types::SearchResult],
    extracted: &[wa_extract::BatchExtractResult],
) -> String {
    let combined: Vec<serde_json::Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let ext = extracted.get(i);
            let mut obj = serde_json::json!({
                "search_title": r.title,
                "url": r.url,
                "snippet": r.snippet,
            });
            if let Some(e) = ext {
                match &e.result {
                    Ok(er) => {
                        obj["status"] = serde_json::json!("ok");
                        obj["markdown"] = serde_json::json!(er.content.markdown);
                        obj["plain_text"] = serde_json::json!(er.content.plain_text);
                        obj["metadata"] = serde_json::json!({
                            "title": er.metadata.title,
                            "description": er.metadata.description,
                            "author": er.metadata.author,
                            "published_date": er.metadata.published_date,
                            "language": er.metadata.language,
                            "site_name": er.metadata.site_name,
                            "word_count": er.metadata.word_count,
                        });
                        obj["links"] = serde_json::json!(er.content.links.iter().map(|l| &l.href).collect::<Vec<_>>());
                        obj["images"] = serde_json::json!(er.content.images.iter().map(|i| &i.src).collect::<Vec<_>>());
                        obj["code_blocks"] = serde_json::json!(er.content.code_blocks);
                        obj["domain"] = serde_json::json!(er.domain_data.as_ref().map(|d| format!("{:?}", d.domain_type).to_lowercase()));
                        obj["structured_data"] = serde_json::json!(er.structured_data);
                    }
                    Err(err) => {
                        obj["status"] = serde_json::json!("error");
                        obj["error"] = serde_json::json!(format!("{}", err));
                    }
                }
            }
            obj
        })
        .collect();

    serde_json::to_string_pretty(&combined).unwrap_or_else(|_| "[]".into())
}

/// Format extraction result as markdown.
///
/// The body markdown from webclaw-core already has the H1 title and inline links.
/// A compact metadata header is prepended by default (opt-out with `--no-meta`),
/// giving AI agents instant context: domain, author, date, type, and word count.
fn format_extract_markdown(
    result: &wa_extract::ExtractionResult,
    url: &str,
    show_meta: bool,
    include_structured_data: bool,
) -> String {
    let mut out = String::new();

    if show_meta {
        out.push_str(&format_compact_meta(result, url));
        out.push('\n');
    }

    out.push_str(&result.content.markdown);

    if include_structured_data && !result.structured_data.is_empty() {
        out.push_str("\n\n## Structured Data\n\n```json\n");
        out.push_str(
            &serde_json::to_string_pretty(&result.structured_data).unwrap_or_default(),
        );
        out.push_str("\n```");
    }
    out.push('\n');
    out
}

/// Build a compact one-line metadata header for AI agent consumption.
///
/// Format: `> url:example.com · site:GitHub · author:Jane · date:2024-01-15 ·
/// type:article · 173 words`
///
/// Every field is labelled so even weak LLMs can parse it without guessing
/// position semantics. The URL always comes first (without `https://` prefix).
/// `site` is omitted when it duplicates the bare domain. Null/empty fields are
/// skipped. The line is always `> `-prefixed (blockquote) so it doesn't
/// interfere with the body markdown.
fn format_compact_meta(result: &wa_extract::ExtractionResult, url: &str) -> String {
    let mut parts: Vec<String> = Vec::new();

    // URL — always first, strip protocol for compactness
    let url_clean = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    parts.push(format!("url:{url_clean}"));

    // Site name — only if it adds information beyond the bare domain
    if let Some(ref site_name) = result.metadata.site_name {
        let site_clean = site_name
            .strip_prefix("https://")
            .or_else(|| site_name.strip_prefix("http://"))
            .unwrap_or(site_name);
        let url_domain = url_clean.split('/').next().unwrap_or(url_clean);
        if !site_clean.eq_ignore_ascii_case(url_domain) {
            parts.push(format!("site:{site_clean}"));
        }
    }

    // Author
    if let Some(ref author) = result.metadata.author {
        parts.push(format!("author:{author}"));
    }

    // Date (short form: YYYY-MM-DD)
    if let Some(ref date) = result.metadata.published_date {
        let short = if date.len() >= 10 { &date[..10] } else { date };
        parts.push(format!("date:{short}"));
    }

    // Domain type
    if let Some(ref dd) = result.domain_data {
        parts.push(format!("type:{}", format!("{:?}", dd.domain_type).to_lowercase()));
    }

    // Word count
    if result.metadata.word_count > 0 {
        parts.push(format!("{} words", result.metadata.word_count));
    }

    if parts.is_empty() {
        return String::new();
    }

    format!("> {}\n", parts.join(" · "))
}

/// Format extraction result as text, falling back to markdown (with images
/// converted to regular links) when `plain_text` is empty — vertical extractors
/// like Reddit only populate markdown, not plain_text.
///
/// Image syntax `![alt](url)` is converted to `[alt](url)` so URLs are preserved
/// as fetchable links. Everything else keeps its markdown syntax — headings,
/// bold, code fences, and lists are all readable as-is.
fn format_extract_text(result: &wa_extract::ExtractionResult) -> String {
    if !result.content.plain_text.is_empty() {
        return result.content.plain_text.clone();
    }
    // Fallback: remove image syntax from the markdown body.
    // Everything else — headings, bold, links, code — is still
    // meaningful as plain text.
    remove_markdown_images(&result.content.markdown)
}

/// Remove inline markdown images `![alt](url)`, replacing with `[alt](url)`
/// so the URL is preserved as a regular link in text mode.
fn remove_markdown_images(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // Found image: ![alt](url) → [alt](url)
            let start = i + 2; // after ![
            if let Some(alt_end) = text[start..].find(']') {
                let alt_end = start + alt_end;
                if alt_end + 1 < text.len()
                    && text.as_bytes()[alt_end + 1] == b'('
                {
                    if let Some(url_end) = text[alt_end + 2..].find(')') {
                        let alt_text = &text[start..alt_end];
                        let url = &text[alt_end + 2..alt_end + 2 + url_end];
                        result.push_str(&format!("[{alt_text}]({url})"));
                        let char_pos = text[..alt_end + 2 + url_end + 1].chars().count();
                        i = char_pos;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Post-process webclaw's LLM text to restore `[label]` brackets around link
/// text in the body. webclaw's `to_llm_text()` strips `[text](url)` to plain
/// `text` and appends a `## Links` footer; this keeps the semantic signal
/// that the text was originally a hyperlink.
///
/// This is a best-effort transform: the first occurrence of each link label
/// in the body is wrapped with brackets, matched as a whole word to avoid
/// partial replacements. Labels are processed longest-first so a shorter
/// label does not falsely match inside a longer one.
fn bracket_links_in_llm_body(llm_text: &str) -> String {
    // Split the output into body (before Links footer) and footer.
    let Some(links_start) = llm_text.find("\n\n## Links\n") else {
        return llm_text.to_string();
    };

    let body = &llm_text[..links_start];
    let footer = &llm_text[links_start..];

    // Parse each "- label: url" line from the Links footer.
    let mut labels: Vec<String> = Vec::new();
    for line in footer.lines().skip(1) {
        if let Some(rest) = line.strip_prefix("- ") {
            if let Some(colon) = rest.find(": ") {
                labels.push(rest[..colon].to_string());
            }
        }
    }

    if labels.is_empty() {
        return llm_text.to_string();
    }

    // Longest labels first: prevents a short label from matching inside
    // a longer one (e.g. "React" inside "Learn React").
    labels.sort_by(|a, b| b.len().cmp(&a.len()));

    let mut bracketed = body.to_string();
    let mut replaced_ranges: Vec<(usize, usize)> = Vec::new();

    'next_label: for label in &labels {
        let label_bytes = label.as_bytes();
        let text_bytes = bracketed.as_bytes();

        for (pos, window) in text_bytes.windows(label_bytes.len()).enumerate() {
            if window != label_bytes {
                continue;
            }
            let end = pos + label_bytes.len();

            // Word-boundary check: not preceded or followed by alphanumeric.
            let before_ok = pos == 0 || !text_bytes[pos - 1].is_ascii_alphanumeric();
            let after_ok = end >= text_bytes.len() || !text_bytes[end].is_ascii_alphanumeric();
            if !before_ok || !after_ok {
                continue;
            }

            // Skip if already bracketed: [label].
            let already_bracketed = pos > 0
                && text_bytes[pos - 1] == b'['
                && end < text_bytes.len()
                && text_bytes[end] == b']';
            if already_bracketed {
                continue;
            }

            // Skip matches inside the metadata header (lines starting with "> ").
            let line_start = text_bytes[..pos].iter().rposition(|&c| c == b'\n').map(|i| i + 1).unwrap_or(0);
            if text_bytes.get(line_start) == Some(&b'>') && text_bytes.get(line_start + 1) == Some(&b' ') {
                continue;
            }

            // No overlap with already-replaced ranges.
            let overlaps = replaced_ranges.iter().any(|(s, e)| pos < *e && end > *s);
            if overlaps {
                continue;
            }

            bracketed.replace_range(pos..end, &format!("[{label}]"));
            replaced_ranges.push((pos, end + 2)); // +2 for "[" and "]"
            continue 'next_label;
        }
    }

    bracketed + footer
}

/// Strip known tracking query parameters from a URL.
///
/// Removes `utm_source`, `utm_medium`, `utm_campaign`, `utm_content`, and
/// `utm_term` while preserving all other parameters, fragments, and the
/// base URL. If no query remains after stripping, the `?` is dropped.
fn clean_url(url: &str) -> String {
    let Some(query_start) = url.find('?') else {
        return url.to_string();
    };

    let base = &url[..query_start];
    let query = &url[query_start + 1..];

    // Preserve fragment identifier if present in the query portion.
    let (query, fragment) = match query.find('#') {
        Some(hash) => (&query[..hash], &query[hash..]),
        None => (query, ""),
    };

    const TRACKING: &[&str] = &[
        "utm_source", "utm_medium", "utm_campaign",
        "utm_content", "utm_term", "ref",
    ];

    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or("");
            !TRACKING.contains(&key)
        })
        .collect();

    if kept.is_empty() {
        format!("{}{}", base, fragment)
    } else {
        format!("{}?{}{}", base, kept.join("&"), fragment)
    }
}

/// Clean tracking parameters from URLs in the `## Links` footer of LLM text.
fn clean_links_footer_urls(llm_text: &str) -> String {
    let Some(links_start) = llm_text.find("\n\n## Links\n") else {
        return llm_text.to_string();
    };

    let before = &llm_text[..links_start];
    let footer = &llm_text[links_start..];

    // Find where Structured Data section starts (if any) so we don't touch it.
    let sd_start = footer.find("\n\n## Structured Data\n");
    let (links_section, after_links) = match sd_start {
        Some(pos) => (&footer[..pos], &footer[pos..]),
        None => (footer, ""),
    };

    let cleaned_links = links_section
        .lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("- ") {
                if let Some(colon_pos) = rest.find(": ") {
                    let label = &rest[..colon_pos];
                    let url = &rest[colon_pos + 2..];
                    return format!("- {}: {}", label, clean_url(url));
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{before}{cleaned_links}{after_links}")
}

/// Format extraction result as LLM-optimized text.
fn format_extract_llm(
    result: &wa_extract::ExtractionResult,
    url: &str,
    include_structured_data: bool,
) -> String {
    let text = wa_extract::to_llm_text(result, Some(url));
    let text = bracket_links_in_llm_body(&text);
    let text = clean_links_footer_urls(&text);
    if !include_structured_data {
        if let Some(idx) = text.rfind("\n\n## Structured Data\n\n```json\n") {
            return text[..idx].trim().to_string();
        }
    }
    text
}

/// Format git clone result as markdown (tree-only mode).
fn format_git_tree_markdown(repo: &wa_core::types::ClonedRepo) -> String {
    let mut out = format!("## Repository Cloned\n\n**Path:** `{}`\n\n", repo.local_path);

    if let Some(ref tree) = repo.tree {
        use std::collections::BTreeMap;
        use std::collections::BTreeSet;

        // Collect all directory paths that appear as parents
        let mut all_dirs = BTreeSet::new();
        let mut dirs: BTreeMap<String, Vec<(String, u64, bool)>> = BTreeMap::new();
        // (name, size, is_dir)

        for entry in tree {
            let parent = std::path::Path::new(&entry.path)
                .parent()
                .and_then(|p| {
                    let s = p.to_string_lossy();
                    if s.is_empty() { None } else { Some(s.to_string()) }
                })
                .unwrap_or_default();
            let name = std::path::Path::new(&entry.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| entry.path.clone());
            dirs.entry(parent.clone()).or_default().push((name, entry.size, false));

            // Collect all intermediate directories
            let mut current = parent.clone();
            while !current.is_empty() {
                all_dirs.insert(current.clone());
                current = std::path::Path::new(&current)
                    .parent()
                    .and_then(|p| {
                        let s = p.to_string_lossy();
                        if s.is_empty() { None } else { Some(s.to_string()) }
                    })
                    .unwrap_or_default();
            }
        }

        // Insert directory entries into their parent dirs
        for d in &all_dirs {
            let parent = std::path::Path::new(d)
                .parent()
                .and_then(|p| {
                    let s = p.to_string_lossy();
                    if s.is_empty() { None } else { Some(s.to_string()) }
                })
                .unwrap_or_default();
            let name = std::path::Path::new(d)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| d.clone());
            dirs.entry(parent).or_default().push((name, 0, true));
        }

        let root_name = std::path::Path::new(&repo.local_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".into());

        out.push_str(&format!("```\n{}/\n", root_name));

        // Render recursively
        fn render_dir(
            out: &mut String,
            dirs: &BTreeMap<String, Vec<(String, u64, bool)>>,
            path: &str,
            prefix: &str,
        ) {
            if let Some(children) = dirs.get(path) {
                // Sort: dirs first, then files; alphabetical within each
                let mut sorted = children.clone();
                sorted.sort_by(|a, b| {
                    b.2.cmp(&a.2) // dirs (true) before files (false)
                        .then_with(|| a.0.cmp(&b.0))
                });

                for (i, (name, size, is_dir)) in sorted.iter().enumerate() {
                    let last = i == sorted.len() - 1;
                    let branch = if last { "└── " } else { "├── " };

                    if *is_dir {
                        out.push_str(&format!("{}{}{}/\n", prefix, branch, name));
                        let new_prefix = format!(
                            "{}{}   ",
                            prefix,
                            if last { " " } else { "│" }
                        );
                        let subdir = if path.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", path, name)
                        };
                        render_dir(out, dirs, &subdir, &new_prefix);
                    } else {
                        out.push_str(&format!(
                            "{}{}{} ({})\n",
                            prefix, branch, name, format_size(*size)
                        ));
                    }
                }
            }
        }

        render_dir(&mut out, &dirs, "", "");

        let total_size: u64 = tree.iter().map(|e| e.size).sum();
        out.push_str(&format!(
            "```\n\n{} files, {} total\n",
            tree.len(),
            format_size(total_size)
        ));
    }

    out
}

/// Format git clone result as text (tree-only mode).
fn format_git_tree_text(repo: &wa_core::types::ClonedRepo) -> String {
    let mut out = format!("Cloned to: {}\n\n", repo.local_path);
    if let Some(ref tree) = repo.tree {
        for entry in tree {
            out.push_str(&format!(
                "{} ({})\n",
                entry.path,
                format_size(entry.size)
            ));
        }
    }
    out
}

/// Format file size in human-readable form (bytes, KiB, MiB).
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Format git clone result as markdown.
fn format_git_markdown(repo: &wa_core::types::ClonedRepo) -> String {
    if repo.tree.is_some() {
        return format_git_tree_markdown(repo);
    }
    let mut out = format!("# Git Clone: {}\n\n", repo.local_path);
    for file in &repo.files {
        out.push_str(&format!("## {}\n\n", file.path));
        out.push_str("```\n");
        out.push_str(&file.content);
        if !file.content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }
    out
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let fmt: OutputFormat = cli.format.into();
    let quiet = cli.quiet;
    let output_file = cli.output;
    let config_arg = cli.config.as_deref().map(std::path::Path::new);

    match cli.command {
        Commands::Config { init, path } => {
            if init {
                let init_path = path.as_deref().map(std::path::Path::new);
                let written = wa_core::config::Config::init_config_file(init_path)?;
                if !quiet {
                    eprintln!("Config file created: {}", written.display());
                }
                write_output(
                    &format!("Config file created: {}\n", written.display()),
                    output_file.as_deref(),
                )?;
            } else {
                let config = wa_core::config::Config::load(config_arg)?;
                let file_path = wa_core::config::Config::config_file_path(config_arg);
                let mut out = String::new();
                if let Some(ref fp) = file_path {
                    out.push_str(&format!(
                        "Config file: {} ({})\n\n",
                        fp.display(),
                        if fp.exists() { "exists" } else { "not found" }
                    ));
                } else {
                    out.push_str("Config file: not found (HOME not set)\n\n");
                }
                out.push_str(&serde_json::to_string_pretty(&config)?);
                out.push('\n');
                write_output(&out, output_file.as_deref())?;
            }
        }

        Commands::Search {
            query,
            fetch,
            fetch_limit,
            concurrency,
            limit,
            searxng_url,
            browser,
            proxy,
            cookies,
            no_meta,
            include_structured_data,
        } => {
            let query_str = query.join(" ");
            if query_str.trim().is_empty() {
                return Err("empty search query".into());
            }

            // Load config (file + env), then apply CLI overrides
            let cfg = wa_core::config::Config::load(config_arg)?;
            let searxng = searxng_url.unwrap_or(cfg.searxng_url);
            let browser_resolved = browser.unwrap_or(cfg.browser_profile);
            let proxy_resolved = proxy.or(cfg.proxy);
            let concurrency_resolved = concurrency.unwrap_or(4);

            if !quiet {
                eprintln!("Searching: \"{}\" via {}", query_str, searxng);
            }

            let client = wa_search::SearXNGClient::new(searxng);

            if fmt == OutputFormat::Raw {
                let raw_json = client.search_raw(&query_str).await?;
                write_output(&raw_json, output_file.as_deref())?;
                return Ok(());
            }

            let results = client.search(&query_str, limit).await?;

            if results.is_empty() {
                if !quiet {
                    eprintln!("No results found.");
                }
                return Ok(());
            }

            if fetch {
                // Fetch and extract each result
                let browser_profile = wa_extract::BrowserProfile::try_from_str(&browser_resolved)?;
                let cookies_opt = if cookies.is_empty() {
                    None
                } else {
                    Some(cookies.clone())
                };

                let extractor = wa_extract::Extractor::new(
                    browser_profile,
                    proxy_resolved.clone(),
                    cookies_opt,
                    12,
                );

                let urls: Vec<&str> = results
                    .iter()
                    .take(fetch_limit)
                    .map(|r| r.url.as_str())
                    .collect();

                if !quiet {
                    eprintln!(
                        "Fetching {} pages (concurrency: {})...",
                        urls.len(),
                        concurrency_resolved
                    );
                }

                let extracted = extractor
                    .fetch_batch(&urls, concurrency_resolved, &Default::default())
                    .await;

                let output = match fmt {
                    OutputFormat::Markdown => {
                        let mut out = String::new();
                        for (i, r) in results.iter().enumerate() {
                            out.push_str(&format!(
                                "## {}: {}\n\n",
                                i + 1,
                                r.title
                            ));
                            out.push_str(&format!("URL: {}\n\n", r.url));
                            if let Some(ext) = extracted.get(i) {
                                match &ext.result {
                                    Ok(er) => {
                                        out.push_str(&format_extract_markdown(er, &r.url, !no_meta, include_structured_data));
                                    }
                                    Err(e) => {
                                        out.push_str(&format!(
                                            "**Error**: {}\n\n",
                                            e
                                        ));
                                    }
                                }
                            }
                            out.push_str("---\n\n");
                        }
                        out
                    }
                    OutputFormat::Llm => {
                        let mut out = String::new();
                        for (i, r) in results.iter().enumerate() {
                            out.push_str(&format!("> Result {}\n", i + 1));
                            out.push_str(&format!("> Title: {}\n", r.title));
                            out.push_str(&format!("> URL: {}\n\n", r.url));
                            if let Some(ext) = extracted.get(i) {
                                match &ext.result {
                                    Ok(er) => {
                                        out.push_str(&format_extract_llm(er, &r.url, include_structured_data));
                                    }
                                    Err(e) => {
                                        out.push_str(&format!(
                                            "> Error: {}\n\n",
                                            e
                                        ));
                                    }
                                }
                            }
                        }
                        out
                    }
                    OutputFormat::Text => {
                        let mut out = String::new();
                        for (i, r) in results.iter().enumerate() {
                            out.push_str(&format!("{}. {}\n", i + 1, r.title));
                            out.push_str(&format!("   {}\n", r.url));
                            if let Some(ext) = extracted.get(i) {
                                match &ext.result {
                                    Ok(er) => {
                                        out.push_str(&format_extract_text(er));
                                        out.push('\n');
                                    }
                                    Err(e) => {
                                        out.push_str(&format!("   Error: {}\n", e));
                                    }
                                }
                            }
                            out.push('\n');
                        }
                        out
                    }
                    OutputFormat::Json => {
                        format_search_fetch_json(&results, &extracted)
                    }
                    OutputFormat::Raw => unreachable!(),
                };

                write_output(&output, output_file.as_deref())?;
            } else {
                // Just search results
                let output = match fmt {
                    OutputFormat::Markdown => format_search_markdown(&results),
                    OutputFormat::Text => {
                        results
                            .iter()
                            .map(|r| {
                                let mut line = format!("{} — {}", r.title, r.url);
                                if let Some(ref img) = r.img_src {
                                    line.push_str(&format!(" — Image: {}", img));
                                }
                                line
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                    OutputFormat::Json => format_search_json(&results),
                    OutputFormat::Raw => unreachable!(),
                    OutputFormat::Llm => {
                        format_search_markdown(&results) // LLM format for search without fetch = markdown
                    }
                };

                write_output(&output, output_file.as_deref())?;

                if !quiet {
                    eprintln!("Found {} results.", results.len());
                }
            }
        }

        Commands::Fetch {
            urls,
            no_meta,
            browser,
            proxy,
            cookies,
            concurrency,
            include,
            exclude,
            only_main_content,
            include_raw_html,
            include_structured_data,
        } => {
            let cfg = wa_core::config::Config::load(config_arg)?;
            let browser_resolved = browser.unwrap_or(cfg.browser_profile);
            let proxy_resolved = proxy.or(cfg.proxy);
            let concurrency_resolved = concurrency.unwrap_or(4);

            let browser_profile = wa_extract::BrowserProfile::try_from_str(&browser_resolved)?;
            let cookies_opt = if cookies.is_empty() {
                None
            } else {
                Some(cookies.clone())
            };
            let extractor =
                wa_extract::Extractor::new(browser_profile, proxy_resolved.clone(), cookies_opt, 12);

            let mut options = wa_extract::ExtractionOptions::default();
            if !include.is_empty() {
                options.include_selectors = include.clone();
            }
            if !exclude.is_empty() {
                options.exclude_selectors = exclude.clone();
            }
            if only_main_content {
                options.only_main_content = true;
            }
            if include_raw_html {
                options.include_raw_html = true;
            }

            if urls.len() == 1 {
                if !quiet {
                    eprintln!("Fetching: {}", urls[0]);
                }
                if fmt == OutputFormat::Raw {
                    let html = extractor.fetch_raw(&urls[0]).await.map_err(|e| format!("{}", e))?;
                    write_output(&html, output_file.as_deref())?;
                } else {
                    let result = extractor
                        .fetch_and_extract(&urls[0], &options)
                        .await
                        .map_err(|e| format!("{}", e))?;

                    let output = match fmt {
                        OutputFormat::Markdown => format_extract_markdown(&result, &urls[0], !no_meta, include_structured_data),
                        OutputFormat::Llm => format_extract_llm(&result, &urls[0], include_structured_data),
                        OutputFormat::Text => format_extract_text(&result),
                        OutputFormat::Json => {
                            let ext_result = wa_extract::BatchExtractResult {
                                url: urls[0].clone(),
                            result: Ok(result),
                        };
                        format_search_fetch_json(
                            &[wa_core::types::SearchResult {
                                title: String::new(),
                                url: urls[0].clone(),
                                snippet: String::new(),
                                img_src: None,
                            }],
                            &[ext_result],
                        )
                    }
                    OutputFormat::Raw => unreachable!(),
                };
                write_output(&output, output_file.as_deref())?;
                }
            } else {
                if fmt == OutputFormat::Raw {
                    let mut out = String::new();
                    for url in &urls {
                        match extractor.fetch_raw(url).await {
                            Ok(html) => out.push_str(&html),
                            Err(e) => {
                                out.push_str(&format!("<!-- Error for {}: {} -->\n", url, e));
                            }
                        }
                    }
                    write_output(&out, output_file.as_deref())?;
                } else {
                    if !quiet {
                        eprintln!(
                            "Fetching {} URLs (concurrency: {})...",
                            urls.len(),
                            concurrency_resolved
                        );
                    }

                    let url_refs: Vec<&str> = urls.iter().map(|u| u.as_str()).collect();
                    let results = extractor
                        .fetch_batch(&url_refs, concurrency_resolved, &options)
                        .await;

                    let output = match fmt {
                    OutputFormat::Markdown => {
                        let mut out = String::new();
                        for r in &results {
                            out.push_str(&format!("## {}\n\n", r.url));
                            match &r.result {
                                Ok(er) => {
                                    if let Some(ref title) = er.metadata.title {
                                        out.push_str(&format!("# {}\n\n", title));
                                    }
                                    out.push_str(&er.content.markdown);
                                }
                                Err(e) => {
                                    out.push_str(&format!("**Error**: {}\n", e));
                                }
                            }
                            out.push_str("\n---\n\n");
                        }
                        out
                    }
                    OutputFormat::Llm => {
                        let mut out = String::new();
                        for r in &results {
                            match &r.result {
                                Ok(er) => {
                                    out.push_str(&format_extract_llm(er, &r.url, include_structured_data));
                                }
                                Err(e) => {
                                    out.push_str(&format!("> Error: {}\n\n", e));
                                }
                            }
                        }
                        out
                    }
                    OutputFormat::Text => results
                        .iter()
                        .map(|r| match &r.result {
                            Ok(er) => format_extract_text(er),
                            Err(e) => format!("Error: {}", e),
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    OutputFormat::Json => {
                        // Build search-like results for the unified schema
                        let search_results: Vec<_> = results
                            .iter()
                            .map(|r| wa_core::types::SearchResult {
                                title: String::new(),
                                url: r.url.clone(),
                                snippet: String::new(),
                                img_src: None,
                            })
                            .collect();
                        format_search_fetch_json(&search_results, &results)
                    }
                    OutputFormat::Raw => unreachable!(),
                };
                write_output(&output, output_file.as_deref())?;
                }
            }

            if !quiet {
                eprintln!("Done.");
            }
        }

        Commands::Browser {
            urls,
            no_meta,
            browser_endpoint,
            include,
            exclude,
            only_main_content,
            include_raw_html,
            include_structured_data,
        } => {
            let cfg = wa_core::config::Config::load(config_arg)?;
            let endpoint = browser_endpoint.unwrap_or(cfg.browser_endpoint);

            let mut options = wa_extract::ExtractionOptions::default();
            if !include.is_empty() {
                options.include_selectors = include.clone();
            }
            if !exclude.is_empty() {
                options.exclude_selectors = exclude.clone();
            }
            if only_main_content {
                options.only_main_content = true;
            }
            if include_raw_html {
                options.include_raw_html = true;
            }

            let client = reqwest::Client::new();

            if urls.len() == 1 {
                if !quiet {
                    eprintln!("Browser fetching: {}", urls[0]);
                }
                let html = fetch_browser_html(&client, &endpoint, &urls[0]).await?;
                if fmt == OutputFormat::Raw {
                    write_output(&html, output_file.as_deref())?;
                } else {
                    let result = wa_extract::extract_with_options(&html, Some(&urls[0]), &options)
                        .map_err(|e| format!("extraction failed: {}", e))?;

                    let output = match fmt {
                        OutputFormat::Markdown => format_extract_markdown(&result, &urls[0], !no_meta, include_structured_data),
                        OutputFormat::Llm => format_extract_llm(&result, &urls[0], include_structured_data),
                        OutputFormat::Text => format_extract_text(&result),
                        OutputFormat::Json => {
                            let ext_result = wa_extract::BatchExtractResult {
                                url: urls[0].clone(),
                                result: Ok(result),
                            };
                            format_search_fetch_json(
                                &[wa_core::types::SearchResult {
                                    title: String::new(),
                                url: urls[0].clone(),
                                snippet: String::new(),
                                img_src: None,
                            }],
                            &[ext_result],
                        )
                    }
                    OutputFormat::Raw => unreachable!(),
                };
                write_output(&output, output_file.as_deref())?;
                }
            } else {
                if fmt == OutputFormat::Raw {
                    let mut out = String::new();
                    for url in &urls {
                        match fetch_browser_html(&client, &endpoint, url).await {
                            Ok(html) => out.push_str(&html),
                            Err(e) => {
                                out.push_str(&format!("<!-- Error for {}: {} -->\n", url, e));
                            }
                        }
                    }
                    write_output(&out, output_file.as_deref())?;
                } else {
                    if !quiet {
                        eprintln!("Browser fetching {} URLs...", urls.len());
                    }
                    let mut results: Vec<wa_extract::BatchExtractResult> = Vec::new();
                    for url in &urls {
                        let result = match fetch_browser_html(&client, &endpoint, url).await {
                            Ok(html) => wa_extract::extract_with_options(&html, Some(url), &options)
                                .map_err(|e| format!("extraction failed: {}", e)),
                            Err(e) => Err(format!("{}", e)),
                        };
                        results.push(wa_extract::BatchExtractResult {
                            url: url.clone(),
                            result: result.map_err(|e| wa_core::error::WaError::Fetch {
                                url: url.clone(),
                                detail: e,
                            }),
                        });
                    }

                let search_results: Vec<wa_core::types::SearchResult> = urls
                    .iter()
                    .map(|u| wa_core::types::SearchResult {
                        title: String::new(),
                        url: u.clone(),
                        snippet: String::new(),
                        img_src: None,
                    })
                    .collect();

                let output = match fmt {
                    OutputFormat::Markdown => {
                        let mut out = String::new();
                        for (i, r) in results.iter().enumerate() {
                            if i > 0 {
                                out.push_str("\n---\n\n");
                            }
                            match &r.result {
                                Ok(er) => {
                                    out.push_str(&format!("## {}\n\n", r.url));
                                    if !no_meta {
                                        out.push_str(&format_compact_meta(er, &r.url));
                                        out.push('\n');
                                    }
                                    out.push_str(&er.content.markdown);
                                    if include_structured_data && !er.structured_data.is_empty() {
                                        out.push_str("\n\n## Structured Data\n\n```json\n");
                                        out.push_str(
                                            &serde_json::to_string_pretty(&er.structured_data)
                                                .unwrap_or_default(),
                                        );
                                        out.push_str("\n```");
                                    }
                                    out.push('\n');
                                }
                                Err(e) => {
                                    out.push_str(&format!("## {} (error)\n\n{}", r.url, e));
                                }
                            }
                        }
                        out
                    }
                    OutputFormat::Llm => {
                        let mut out = String::new();
                        for r in &results {
                            match &r.result {
                                Ok(er) => {
                                    out.push_str(&format_extract_llm(er, &r.url, include_structured_data));
                                    out.push('\n');
                                }
                                Err(e) => {
                                    out.push_str(&format!("> URL: {}\n> Error: {}\n\n", r.url, e));
                                }
                            }
                        }
                        out
                    }
                    OutputFormat::Text => {
                        let mut parts: Vec<String> = Vec::new();
                        for r in &results {
                            match &r.result {
                                Ok(er) => parts.push(format_extract_text(er)),
                                Err(e) => parts.push(format!("{} (error: {})", r.url, e)),
                            }
                        }
                        parts.join("\n\n")
                    }
                    OutputFormat::Json => format_search_fetch_json(&search_results, &results),
                    OutputFormat::Raw => unreachable!(),
                };
                write_output(&output, output_file.as_deref())?;
                }
            }
        }

        Commands::Git {
            url,
            max_file_size,
            max_files,
            output_dir,
            tree_only,
        } => {
            let cfg = wa_core::config::Config::load(config_arg)?;
            let max_file_size_resolved = max_file_size.unwrap_or(cfg.max_file_size);
            let max_files_resolved = max_files.unwrap_or(cfg.max_files);

            let opts = wa_git::GitCloneOptions {
                max_file_size: max_file_size_resolved,
                max_files: max_files_resolved,
                output_dir: output_dir.map(std::path::PathBuf::from),
                tree_only,
            };

            let cloner = wa_git::GitCloner::new(opts);

            if !quiet {
                eprintln!("Cloning: {}", url);
            }

            let repo = cloner.clone_and_list(&url)?;

            let output = match fmt {
                OutputFormat::Markdown => format_git_markdown(&repo),
                OutputFormat::Text => {
                    if repo.tree.is_some() {
                        format_git_tree_text(&repo)
                    } else {
                        let mut out = format!("Cloned to: {}\n\n", repo.local_path);
                        for file in &repo.files {
                            out.push_str(&format!("{}\n", file.path));
                        }
                        out
                    }
                }
                OutputFormat::Json => {
                    serde_json::to_string_pretty(&repo).unwrap_or_else(|_| "{}".into())
                }
                OutputFormat::Llm => {
                    // Same as markdown for git
                    format_git_markdown(&repo)
                }
                OutputFormat::Raw => format_git_markdown(&repo),
            };

            write_output(&output, output_file.as_deref())?;

            if !quiet {
                eprintln!(
                    "Cloned to: {} ({} files)",
                    repo.local_path,
                    repo.tree.as_ref().map(|t| t.len()).unwrap_or(repo.files.len())
                );
            }
        }
    }

    Ok(())
}

/// Write output to stdout or a file.
///
/// When `--output FILE` is specified, the result is written to the file
/// and stdout remains empty (exit code communicates success/failure).
fn write_output(content: &str, file: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = file {
        std::fs::write(path, content)?;
    } else {
        print!("{}", content);
    }
    Ok(())
}

/// Fetch rendered HTML from a browser-backed endpoint.
///
/// Appends the URL-encoded target URL to the endpoint base. The endpoint is
/// expected to return the fully-rendered HTML of the page.
async fn fetch_browser_html(
    client: &reqwest::Client,
    endpoint: &str,
    url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let full_url = format!("{endpoint}{url}", endpoint = endpoint, url = url);
    // Later we may want URL encoding; for now the browser endpoint
    // accepts raw URLs (they are valid in query strings in practice).
    let resp = client
        .get(&full_url)
        .send()
        .await
        .map_err(|e| format!("browser endpoint request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "browser endpoint returned HTTP {} for {}",
            resp.status().as_u16(),
            url
        )
        .into());
    }

    resp.text()
        .await
        .map_err(|e| format!("failed to read browser response: {}", e).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracket_links_restores_body_brackets() {
        let input = "Check out Last week for more info.\n\n## Links\n- Last week: https://example.com/last-week\n";
        let expected = "Check out [Last week] for more info.\n\n## Links\n- Last week: https://example.com/last-week\n";
        assert_eq!(bracket_links_in_llm_body(input), expected);
    }

    #[test]
    fn bracket_links_no_links_section_unchanged() {
        let input = "No links here, just plain text.";
        assert_eq!(bracket_links_in_llm_body(input), input);
    }

    #[test]
    fn bracket_links_longest_first_prevents_partial() {
        let input = "Read Learn React today.\n\n## Links\n- Learn React: https://react.dev/learn\n- React: https://react.dev\n";
        let result = bracket_links_in_llm_body(input);
        // "Learn React" (the longer label) should be bracketed
        assert!(result.contains("[Learn React]"));
        // The shorter "React" label should not create a false match
        // because "Learn React" already occupies that range
        let learn_react_count = result.matches("[Learn React]").count();
        assert_eq!(learn_react_count, 1);
    }

    #[test]
    fn bracket_links_does_not_double_bracket() {
        let input = "See [Last week] for details.\n\n## Links\n- Last week: https://example.com\n";
        assert_eq!(bracket_links_in_llm_body(input), input);
    }

    #[test]
    fn bracket_links_preserves_structured_data() {
        let input = "Check out Example for details.\n\n## Links\n- Example: https://example.com\n\n## Structured Data\n```json\n{\"@type\":\"Article\"}\n```";
        let result = bracket_links_in_llm_body(input);
        assert!(result.contains("## Structured Data"));
        assert!(result.contains("[Example]"));
    }

    #[test]
    fn bracket_links_skips_word_boundary_violations() {
        let input = "The React framework and Preact library.\n\n## Links\n- React: https://react.dev\n";
        let result = bracket_links_in_llm_body(input);
        // "React" should be bracketed as a whole word, not inside "Preact"
        // The first standalone "React" is bracketed
        assert!(result.contains("[React] framework"));
    }

    #[test]
    fn bracket_links_multiple_occurrences_brackets_first_only() {
        let input = "Last week was good. Last week was busy.\n\n## Links\n- Last week: https://example.com\n";
        let result = bracket_links_in_llm_body(input);
        let count = result.matches("[Last week]").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn bracket_links_skips_metadata_header_lines() {
        let input = "> Title: Self-Host Weekly\n> URL: https://example.com\n\nRead Self-Host Weekly today.\n\n## Links\n- Self-Host Weekly: https://example.com\n";
        let result = bracket_links_in_llm_body(input);
        // Metadata line should NOT be bracketed
        assert!(result.contains("> Title: Self-Host Weekly"));
        // Body text SHOULD be bracketed
        assert!(result.contains("[Self-Host Weekly] today"));
    }

    // -----------------------------------------------------------------------
    // URL cleaning tests
    // -----------------------------------------------------------------------

    #[test]
    fn clean_url_strips_utm_params() {
        let dirty = "https://example.com/?utm_source=newsletter&utm_medium=email&utm_campaign=spring";
        assert_eq!(clean_url(dirty), "https://example.com/");
    }

    #[test]
    fn clean_url_preserves_non_tracking_params() {
        let dirty = "https://example.com/?page=2&utm_source=feed&id=42";
        assert_eq!(clean_url(dirty), "https://example.com/?page=2&id=42");
    }

    #[test]
    fn clean_url_no_query_unchanged() {
        let clean = "https://example.com/article";
        assert_eq!(clean_url(clean), clean);
    }

    #[test]
    fn clean_url_preserves_fragment() {
        let dirty = "https://example.com/?utm_source=x#section-3";
        assert_eq!(clean_url(dirty), "https://example.com/#section-3");
    }

    #[test]
    fn clean_url_empty_query_after_stripping() {
        let dirty = "https://example.com/?utm_source=x";
        assert_eq!(clean_url(dirty), "https://example.com/");
    }

    #[test]
    fn clean_links_footer_urls_cleans_all_links() {
        let input = "Body text.\n\n## Links\n- Example: https://a.com?utm_source=x\n- Other: https://b.com?utm_medium=y&page=1\n";
        let result = clean_links_footer_urls(input);
        assert!(result.contains("https://a.com"));
        assert!(!result.contains("utm_source"));
        assert!(result.contains("https://b.com?page=1"));
        assert!(!result.contains("utm_medium"));
    }

    #[test]
    fn clean_links_footer_preserves_structured_data() {
        let input = "## Links\n- Example: https://a.com?utm_source=x\n\n## Structured Data\n```json\n{}\n```";
        let result = clean_links_footer_urls(input);
        assert!(result.contains("## Structured Data"));
        assert!(result.contains("```json"));
    }

    #[test]
    fn clean_links_footer_no_links_section_unchanged() {
        let input = "Just body text with no footer.";
        assert_eq!(clean_links_footer_urls(input), input);
    }
}
