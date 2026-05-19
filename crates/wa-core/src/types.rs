use serde::{Deserialize, Serialize};

/// A single result from a SearXNG search query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// Image URL, only populated when the result category is "images".
    pub img_src: Option<String>,
}

/// A text file extracted from a cloned git repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitFile {
    /// Relative path within the repo, e.g. "src/main.rs"
    pub path: String,
    /// File content as a string
    pub content: String,
    /// File size in bytes
    pub size: usize,
}

/// A single entry in a repository file tree (no content — path + size only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// Relative path within the repo, e.g. "src/main.rs"
    pub path: String,
    /// File size in bytes
    pub size: u64,
}

/// The result of cloning a git repository.
/// The on-disk clone persists — AI agents can reference `local_path`
/// for further operations like running `cargo build` or grepping files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClonedRepo {
    /// Absolute path to the cloned repository on disk
    pub local_path: String,
    /// Text files extracted from the repository (empty when `tree_only`)
    pub files: Vec<GitFile>,
    /// File tree with paths and sizes (populated when `tree_only`, otherwise `None`)
    pub tree: Option<Vec<TreeEntry>>,
}

/// Output format for fetch and search commands.
/// The CLI crate provides its own clap::ValueEnum and converts to/from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Llm,
    Text,
    Json,
    Raw,
}
