use wa_core::error::WaError;
use wa_core::types::{ClonedRepo, GitFile, SearchResult};
use std::io;

// ---- T9: search_result_json_roundtrip -----------------------------------

#[test]
fn search_result_json_roundtrip() {
    let sr = SearchResult {
        title: "Rust async book".into(),
        url: "https://rust-lang.org/async-book".into(),
        snippet: "An introduction to async programming in Rust.".into(),
    };
    let json = serde_json::to_string(&sr).unwrap();
    let back: SearchResult = serde_json::from_str(&json).unwrap();
    assert_eq!(sr, back);
}

// ---- T10: git_file_json_roundtrip ---------------------------------------

#[test]
fn git_file_json_roundtrip() {
    let gf = GitFile {
        path: "src/main.rs".into(),
        content: "fn main() {}".into(),
        size: 13,
    };
    let json = serde_json::to_string(&gf).unwrap();
    let back: GitFile = serde_json::from_str(&json).unwrap();
    assert_eq!(gf, back);
}

// ---- T11: cloned_repo_json_roundtrip ------------------------------------

#[test]
fn cloned_repo_json_roundtrip() {
    let repo = ClonedRepo {
        local_path: "/tmp/wa-git-abc123/serde".into(),
        tree: None,
        files: vec![
            GitFile {
                path: "Cargo.toml".into(),
                content: "[package]\nname = \"serde\"".into(),
                size: 28,
            },
            GitFile {
                path: "src/lib.rs".into(),
                content: "pub fn serialize() {}".into(),
                size: 23,
            },
        ],
    };
    let json = serde_json::to_string(&repo).unwrap();
    let back: ClonedRepo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.local_path, repo.local_path);
    assert_eq!(back.files.len(), 2);
    assert_eq!(back.files[0].path, "Cargo.toml");
}

// ---- T12: error_display_formatting --------------------------------------

#[test]
fn error_display_formatting() {
    let err = WaError::fetch("https://example.com", "connection refused");
    let msg = format!("{}", err);
    assert!(msg.contains("https://example.com"));
    assert!(msg.contains("connection refused"));
    assert!(msg.contains("fetch error"));
}

// ---- T13: error_io_from -------------------------------------------------

#[test]
fn error_io_from() {
    // Simulate using `?` operator with io::Error
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
    let wa_err: WaError = io_err.into();
    let msg = format!("{}", wa_err);
    assert!(msg.contains("i/o error"));
    assert!(msg.contains("file missing"));
}
