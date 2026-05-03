use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use std::io::Write;

/// Helper: get the wa binary path
fn wa_bin() -> Command {
    Command::cargo_bin("wa").unwrap()
}

// ---- T60: cli_help ---------------------------------------------------------

#[test]
fn cli_help() {
    let mut cmd = wa_bin();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Web Access CLI"));
}

// ---- T61: cli_version ------------------------------------------------------

#[test]
fn cli_version() {
    let mut cmd = wa_bin();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("wa ").or(predicate::str::contains("0.1")));
}

// ---- T68: cli_config_command -----------------------------------------------

#[test]
fn cli_config_command() {
    let mut cmd = wa_bin();
    cmd.arg("config");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("searxng_url"));
}

// ---- T69: cli_missing_command ----------------------------------------------

#[test]
fn cli_missing_command() {
    let mut cmd = wa_bin();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("COMMAND")));
}

// ---- T70: cli_bad_format ---------------------------------------------------

#[test]
fn cli_bad_format() {
    let mut cmd = wa_bin();
    cmd.args(["search", "--format", "pdf", "test"]);
    cmd.assert().failure();
}

// ---- T71: cli_quiet_mode ---------------------------------------------------

#[test]
fn cli_quiet_mode() {
    let mut cmd = wa_bin();
    // --quiet with config (which doesn't need network) should work
    cmd.args(["--quiet", "config"]);
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    // stderr should be empty in quiet mode (no progress messages)
    // Note: config command doesn't produce progress, so stderr is empty regardless
}

// ---- Tests requiring infrastructure ----------------------------------------
// T62-T67 require either SearXNG or network access.
// These are marked as skipped when infrastructure is not available.
// Use `cargo test -- --ignored` to run them with real services.

// ---- T65: cli_fetch_output_file --------------------------------------------

#[test]
#[ignore = "requires network access"]
fn cli_fetch_output_file() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("output.md");

    let mut cmd = wa_bin();
    cmd.args([
        "fetch",
        "--output",
        out.to_str().unwrap(),
        "https://example.com",
    ]);
    cmd.assert().success();

    let content = std::fs::read_to_string(&out).unwrap();
    assert!(!content.is_empty());
}

// ---- T66: cli_fetch_stdout -------------------------------------------------

#[test]
#[ignore = "requires network access"]
fn cli_fetch_stdout() {
    let mut cmd = wa_bin();
    cmd.args(["fetch", "https://example.com"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Example"));
}

// ---- T67: cli_git_output_file ----------------------------------------------

#[test]
fn cli_git_output_file() {
    // Create a local repo to clone from
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    std::fs::create_dir(&source).unwrap();

    // init git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&source)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&source)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&source)
        .output()
        .unwrap();

    std::fs::write(source.join("README.md"), "# Test").unwrap();
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&source)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&source)
        .output()
        .unwrap();

    let out = tmp.path().join("repo.md");
    let mut cmd = wa_bin();
    cmd.args([
        "git",
        "--output",
        out.to_str().unwrap(),
        &format!("file://{}", source.display()),
    ]);
    cmd.assert().success();

    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("README.md"));
    assert!(content.contains("# Test"));
}
