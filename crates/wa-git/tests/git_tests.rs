use std::fs;
use std::path::Path;
use std::process::Command;
use wa_git::{GitCloneOptions, GitCloner};

/// Create a local git repo in the given directory with some files.
fn init_test_repo(dir: &Path) {
    Command::new("git")
        .arg("init")
        .current_dir(dir)
        .output()
        .unwrap();

    // Configure a dummy user (required for commits)
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .unwrap();

    // Create some files
    fs::write(dir.join("README.md"), "# Test Repo\n\nHello!").unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src").join("lib.rs"), "pub fn hello() {}").unwrap();
    fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();

    // Create a noise dir with a file
    fs::create_dir_all(dir.join("node_modules").join("pkg")).unwrap();
    fs::write(
        dir.join("node_modules").join("pkg").join("index.js"),
        "module.exports = {};",
    )
    .unwrap();

    // Create a binary file
    fs::write(dir.join("logo.png"), &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x00][..]).unwrap();

    // Create a hidden file
    fs::write(dir.join(".env"), "SECRET=test").unwrap();

    // Create a lockfile
    fs::write(dir.join("Cargo.lock"), "lockfile content").unwrap();

    // Create a large file
    let large_content = "x".repeat(200_000);
    fs::write(dir.join("large.txt"), &large_content).unwrap();

    // Commit everything
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir)
        .output()
        .unwrap();
}

/// Helper: run clone and get result
fn clone_test_repo(repo_path: &Path, output_dir: &Path) -> wa_core::types::ClonedRepo {
    let opts = GitCloneOptions {
        max_file_size: 102_400,     // 100 KiB
        max_files: 100,
        output_dir: Some(output_dir.to_path_buf()),
        tree_only: false,
    };

    let cloner = GitCloner::new(opts);
    let file_url = format!("file://{}", repo_path.display());
    cloner.clone_and_list(&file_url).unwrap()
}

// ---- T50: clone_small_repo -------------------------------------------------

#[test]
fn clone_small_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");
    // Don't pre-create dest — git clone will create it

    let repo = clone_test_repo(&source, &dest);

    // Verify local_path is present
    assert!(repo.local_path.len() > 0);
    assert!(Path::new(&repo.local_path).exists());

    // Should have collected some text files
    assert!(!repo.files.is_empty());

    // README.md should be present
    let readme = repo.files.iter().find(|f| f.path == "README.md");
    assert!(readme.is_some(), "README.md should be in the cloned repo");
    assert!(readme.unwrap().content.contains("Hello"));

    // src/lib.rs should be present
    let lib = repo.files.iter().find(|f| f.path == "src/lib.rs");
    assert!(lib.is_some());
}

// ---- T51: clone_skip_binary -------------------------------------------------

#[test]
fn clone_skip_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");
    let repo = clone_test_repo(&source, &dest);

    // logo.png should NOT be in the file list
    let png = repo.files.iter().find(|f| f.path == "logo.png");
    assert!(png.is_none(), "binary files should be excluded");
}

// ---- T52: clone_skip_noise_dirs --------------------------------------------

#[test]
fn clone_skip_noise_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");
    let repo = clone_test_repo(&source, &dest);

    // node_modules files should NOT be in the file list
    let has_nm = repo.files.iter().any(|f| f.path.contains("node_modules"));
    assert!(!has_nm, "node_modules should be excluded");
}

// ---- T53: clone_skip_hidden_files ------------------------------------------

#[test]
fn clone_skip_hidden_files() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");
    let repo = clone_test_repo(&source, &dest);

    // .env should NOT be in the file list
    let has_env = repo.files.iter().any(|f| f.path == ".env");
    assert!(!has_env, ".env should be excluded");

    // .gitignore should be present if the repo has one (it doesn't in our test)
    // but .env should definitely be excluded
}

// ---- T54: clone_skip_lockfiles ---------------------------------------------

#[test]
fn clone_skip_lockfiles() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");
    let repo = clone_test_repo(&source, &dest);

    // Cargo.lock should NOT be in the file list
    let has_lock = repo.files.iter().any(|f| f.path == "Cargo.lock");
    assert!(!has_lock, "Cargo.lock should be excluded");
}

// ---- T55: clone_max_file_size ----------------------------------------------

#[test]
fn clone_max_file_size() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");

    let opts = GitCloneOptions {
        max_file_size: 102_400,
        max_files: 100,
        output_dir: Some(dest.to_path_buf()),
        tree_only: false,
    };
    let cloner = GitCloner::new(opts);
    let file_url = format!("file://{}", source.display());
    let repo = cloner.clone_and_list(&file_url).unwrap();

    // large.txt is 200KB, max is 100KB — should be excluded
    let large = repo.files.iter().find(|f| f.path == "large.txt");
    assert!(large.is_none(), "large.txt exceeds max_file_size");
}

// ---- T56: clone_max_files --------------------------------------------------

#[test]
fn clone_max_files() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");

    let opts = GitCloneOptions {
        max_file_size: 102_400,
        max_files: 2, // only collect 2 files
        output_dir: Some(dest.to_path_buf()),
        tree_only: false,
    };
    let cloner = GitCloner::new(opts);
    let file_url = format!("file://{}", source.display());
    let repo = cloner.clone_and_list(&file_url).unwrap();

    assert!(repo.files.len() <= 2, "should respect max_files limit");
}

// ---- T57: clone_invalid_url ------------------------------------------------

#[test]
fn clone_invalid_url() {
    let cloner = GitCloner::new(GitCloneOptions::default());
    let err = cloner.clone_and_list("not-a-url").unwrap_err();
    assert!(format!("{}", err).contains("invalid") || format!("{}", err).contains("unsupported"));
}

// ---- T58: clone_nonexistent_repo -------------------------------------------

#[test]
fn clone_nonexistent_repo() {
    if GitCloner::check_git_available().is_err() {
        eprintln!("skipping: git not available");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("dest");

    let opts = GitCloneOptions {
        max_file_size: 102_400,
        max_files: 100,
        output_dir: Some(dest),
        tree_only: false,
    };
    let cloner = GitCloner::new(opts);
    let result = cloner.clone_and_list("https://github.com/this/definitely-does-not-exist-999999");
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("git") || msg.contains("error") || msg.contains("failed"));
}

// ---- T59: git_clone_fresh_repo --------------------------------------------

#[test]
fn git_clone_fresh_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir(&source).unwrap();
    init_test_repo(&source);

    let dest = tmp.path().join("dest");
    let repo = clone_test_repo(&source, &dest);

    // At minimum, we should have README.md
    assert!(!repo.files.is_empty());
    assert!(!repo.local_path.is_empty());
}
