//! Git repository cloning and text file listing.
//!
//! Clones a git repository to `/tmp`, then walks the filesystem tree
//! collecting text files while skipping binaries, noise directories,
//! and files exceeding size limits.  The clone persists on disk so
//! AI agents can reference it for further operations.

use std::path::{Path, PathBuf};
use std::process::Command;
use wa_core::error::WaError;
use wa_core::types::{ClonedRepo, GitFile, TreeEntry};

/// Options for cloning a git repository.
#[derive(Debug, Clone)]
pub struct GitCloneOptions {
    /// Maximum file size in bytes to read (default: 100 KiB).
    pub max_file_size: usize,

    /// Maximum number of text files to collect (default: 100).
    pub max_files: usize,

    /// Optional output directory. If `None`, a temp dir under `/tmp` is used.
    pub output_dir: Option<PathBuf>,

    /// Only collect paths and sizes — skip reading file contents.
    pub tree_only: bool,
}

impl Default for GitCloneOptions {
    fn default() -> Self {
        Self {
            max_file_size: 102_400,
            max_files: 100,
            output_dir: None,
            tree_only: false,
        }
    }
}

/// Binary file extensions that are skipped.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "ico", "svg", "webp", "bmp", "tiff",
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    "exe", "dll", "so", "dylib", "wasm",
    "mp3", "mp4", "avi", "mov", "wav", "flac", "ogg",
    "ttf", "otf", "woff", "woff2", "eot",
    "class", "pyc", "o", "a", "lib", "rlib",
    "db", "sqlite", "sqlite3", "mdb",
];

/// Directory names that are skipped entirely.
const NOISE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "vendor",
    "dist",
    "build",
    "target",
    "__pycache__",
    ".tox",
    ".venv",
    "venv",
    ".next",
    ".nuxt",
    ".cache",
    "bower_components",
];

/// Hidden files (starting with `.`) that are NOT text-files-of-interest.
const SKIP_HIDDEN: &[&str] = &[".env", ".DS_Store", ".gitkeep", ".editorconfig"];

/// Lockfiles that are skipped.
const SKIP_LOCKFILES: &[&str] = &["Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml"];

/// A git repository cloner.
pub struct GitCloner {
    options: GitCloneOptions,
}

impl GitCloner {
    /// Create a new cloner with the given options.
    pub fn new(options: GitCloneOptions) -> Self {
        Self { options }
    }

    /// Verify that `git` is available on the system PATH.
    pub fn check_git_available() -> Result<(), WaError> {
        let output = Command::new("git")
            .arg("--version")
            .output()
            .map_err(|_| WaError::GitNotFound)?;
        if !output.status.success() {
            return Err(WaError::GitNotFound);
        }
        Ok(())
    }

    /// Clone a repository and collect its text files.
    ///
    /// Supported URL formats:
    /// - `https://github.com/owner/repo`
    /// - `https://github.com/owner/repo/tree/branch`
    /// - `https://github.com/owner/repo/blob/branch/path`
    /// - `https://gitlab.com/owner/repo`
    /// - `https://codeberg.org/owner/repo`
    /// - Generic `https://` or `git@` URLs
    ///
    /// Gist URLs (`gist.github.com`) are rejected.
    pub fn clone_and_list(&self, url: &str) -> Result<ClonedRepo, WaError> {
        // Pre-flight: git must be available
        Self::check_git_available()?;

        // Reject gist URLs
        if url.contains("gist.github.com") {
            return Err(WaError::Git(
                "gist URLs are not supported — use fetch to read gist content".into(),
            ));
        }

        // Determine clone URL and optional branch
        let (clone_url, branch) = parse_git_url(url)?;

        // Determine clone destination
        let dest = match &self.options.output_dir {
            Some(dir) => dir.clone(),
            None => {
                let dir_name = repo_dir_name(url);
                // Use /tmp like pi-searxng
                let tmp = std::env::temp_dir().join(format!("wa-git-{}", random_suffix()));
                // Create the parent, then append the repo name
                std::fs::create_dir_all(&tmp).map_err(|e| {
                    WaError::Git(format!("failed to create temp dir: {}", e))
                })?;
                tmp.join(&dir_name)
            }
        };

        if dest.exists() {
            // If the directory already exists, git clone will fail.
            // Remove it so git can recreate it.
            if dest.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
                return Err(WaError::Git(format!(
                    "destination '{}' is not empty",
                    dest.display()
                )));
            }
            std::fs::remove_dir(&dest).map_err(|e| {
                WaError::Git(format!("failed to remove existing dir: {}", e))
            })?;
        }

        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    WaError::Git(format!("failed to create parent dir: {}", e))
                })?;
            }
        }

        // Build the git clone command
        let mut cmd = Command::new("git");
        cmd.arg("clone")
            .arg("--depth")
            .arg("1");

        if let Some(ref b) = branch {
            cmd.arg("--branch").arg(b);
        }

        cmd.arg(&clone_url).arg(&dest);

        let output = cmd.output().map_err(|e| {
            WaError::Git(format!("failed to run git: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WaError::Git(format!(
                "git clone failed: {}",
                stderr.trim()
            )));
        }

        let local_path = dest.to_string_lossy().to_string();

        if self.options.tree_only {
            let tree = walk_tree_only(&dest, self.options.max_files)?;
            Ok(ClonedRepo {
                local_path,
                files: Vec::new(),
                tree: Some(tree),
            })
        } else {
            let files = walk_and_collect(
                &dest,
                self.options.max_file_size,
                self.options.max_files,
            )?;
            Ok(ClonedRepo {
                local_path,
                files,
                tree: None,
            })
        }
    }
}

/// Walk a directory and collect text files, respecting limits.
fn walk_and_collect(
    root: &Path,
    max_file_size: usize,
    max_files: usize,
) -> Result<Vec<GitFile>, WaError> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_noise_dir(e))
    {
        let entry = entry.map_err(|e| WaError::Io(e.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // Skip binary extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                continue;
            }
        }

        // Skip hidden files that aren't useful
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if SKIP_HIDDEN.contains(&name) {
                continue;
            }
            if SKIP_LOCKFILES.contains(&name) {
                continue;
            }
        }

        // Check file size
        let metadata = entry.metadata().map_err(|e| WaError::Io(e.into()))?;
        let size = metadata.len() as usize;
        if size > max_file_size {
            continue;
        }

        // Read content
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| {
            // If we can't read as UTF-8, skip
            String::new()
        });

        if content.is_empty() && size > 0 {
            // Binary or non-UTF8 — skip
            continue;
        }

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        files.push(GitFile {
            path: rel_path,
            content,
            size,
        });

        if files.len() >= max_files {
            break;
        }
    }

    Ok(files)
}

/// Check if a directory entry should be skipped.
fn is_noise_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|n| NOISE_DIRS.contains(&n))
            .unwrap_or(false)
}

/// Walk a cloned repo and collect only file paths and sizes (no content).
fn walk_tree_only(
    root: &Path,
    max_files: usize,
) -> Result<Vec<TreeEntry>, WaError> {
    let mut entries = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_noise_dir(e))
    {
        let entry = entry.map_err(|e| WaError::Io(e.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // Skip binary extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                continue;
            }
        }

        // Skip hidden files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if SKIP_HIDDEN.contains(&name) || SKIP_LOCKFILES.contains(&name) {
                continue;
            }
        }

        let metadata = entry.metadata().map_err(|e| WaError::Io(e.into()))?;
        let size = metadata.len();

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        entries.push(TreeEntry {
            path: rel_path,
            size,
        });

        if entries.len() >= max_files {
            break;
        }
    }

    Ok(entries)
}

/// Generate a short random suffix for temp dirs.
fn random_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

/// Get a human-readable directory name from a URL.
fn repo_dir_name(url: &str) -> String {
    // Extract the last path segment before any trailing slashes, /tree/, /blob/
    let trimmed = url.trim_end_matches('/');
    let segments: Vec<&str> = trimmed.split('/').collect();

    // Walk backwards to find the repo name
    for i in (1..segments.len()).rev() {
        if segments[i] == "tree" || segments[i] == "blob" {
            // The segment after tree/blob is the branch, before it is the repo
            if i > 0 {
                return segments[i - 1].to_string();
            }
        }
    }

    // Last segment that's not empty
    segments.last().map(|s| s.to_string()).unwrap_or_else(|| "repo".into())
}

/// Parse a git URL into a clone URL and optional branch.
///
/// For GitHub blob/tree URLs, extracts the branch.
fn parse_git_url(url: &str) -> Result<(String, Option<String>), WaError> {
    let url_lower = url.to_lowercase();

    // Check for supported hosts
    let is_supported = url_lower.contains("github.com")
        || url_lower.contains("gitlab.com")
        || url_lower.contains("codeberg.org")
        || url.starts_with("git@")
        || url.starts_with("file://");

    if !is_supported {
        return Err(WaError::Git(format!(
            "unsupported git host: {}. Supported: github.com, gitlab.com, codeberg.org, or any git@ URL",
            url
        )));
    }

    // file:// URLs pass through unchanged
    if url.starts_with("file://") {
        return Ok((url.to_string(), None));
    }

    // Parse the URL parts
    let url_no_proto = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let parts: Vec<&str> = url_no_proto.split('/').collect();
    // Expected: host/owner/repo[/tree/branch|/blob/branch/path...]

    if parts.len() < 3 {
        return Err(WaError::Git(format!(
            "invalid git URL: expected host/owner/repo, got {}",
            url
        )));
    }

    let host = parts[0];
    let owner = parts[1];
    let repo = parts[2].trim_end_matches(".git");

    let mut branch: Option<String> = None;
    let clone_url = format!("https://{}/{}/{}.git", host, owner, repo);

    // Check for /tree/ or /blob/ patterns
    if parts.len() >= 5 && (parts[3] == "tree" || parts[3] == "blob") {
        branch = Some(parts[4].to_string());
    }

    Ok((clone_url, branch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_root_url() {
        let (clone_url, branch) =
            parse_git_url("https://github.com/serde-rs/serde").unwrap();
        assert_eq!(clone_url, "https://github.com/serde-rs/serde.git");
        assert_eq!(branch, None);
    }

    #[test]
    fn parse_github_tree_url() {
        let (clone_url, branch) =
            parse_git_url("https://github.com/serde-rs/serde/tree/dev").unwrap();
        assert_eq!(clone_url, "https://github.com/serde-rs/serde.git");
        assert_eq!(branch, Some("dev".into()));
    }

    #[test]
    fn parse_github_blob_url() {
        let (clone_url, branch) =
            parse_git_url("https://github.com/serde-rs/serde/blob/main/src/lib.rs").unwrap();
        assert_eq!(clone_url, "https://github.com/serde-rs/serde.git");
        assert_eq!(branch, Some("main".into()));
    }

    #[test]
    fn parse_gitlab_url() {
        let (clone_url, branch) =
            parse_git_url("https://gitlab.com/gitlab-org/gitlab").unwrap();
        assert_eq!(clone_url, "https://gitlab.com/gitlab-org/gitlab.git");
        assert_eq!(branch, None);
    }

    #[test]
    fn parse_codeberg_url() {
        let (clone_url, branch) =
            parse_git_url("https://codeberg.org/forgejo/forgejo").unwrap();
        assert!(clone_url.contains("codeberg.org/forgejo/forgejo.git"));
        assert_eq!(branch, None);
    }

    #[test]
    fn parse_unsupported_host() {
        let err = parse_git_url("https://bitbucket.org/user/repo").unwrap_err();
        assert!(format!("{}", err).contains("unsupported"));
    }

    #[test]
    fn parse_not_a_repo_url() {
        let err = parse_git_url("https://github.com/serde-rs").unwrap_err();
        assert!(format!("{}", err).contains("invalid"));
    }

    #[test]
    fn parse_gist_url_excluded() {
        let cloner = GitCloner::new(GitCloneOptions::default());
        let err = cloner.clone_and_list("https://gist.github.com/user/abc123").unwrap_err();
        assert!(format!("{}", err).contains("gist"));
    }
}
