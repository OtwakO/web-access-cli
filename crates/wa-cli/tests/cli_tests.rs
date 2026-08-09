use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

/// Helper: get the wa binary path
fn wa_bin() -> Command {
    let config_home = tempfile::tempdir().unwrap().keep();
    let mut command = Command::cargo_bin("wa").unwrap();
    command.env("XDG_CONFIG_HOME", config_home);
    command
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
        .stdout(predicate::str::contains("\"provider\": \"searxng\""));
}

#[test]
fn search_help_uses_generic_provider_flags_and_removes_legacy_flag() {
    let mut cmd = wa_bin();
    cmd.args(["search", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--search-provider"))
        .stdout(predicate::str::contains("--search-url"))
        .stdout(predicate::str::contains("--search-api-key"))
        .stdout(predicate::str::contains("--searxng-url").not());
}

#[test]
fn removed_searxng_flag_fails_loudly() {
    let mut cmd = wa_bin();
    cmd.args([
        "search",
        "--searxng-url",
        "https://legacy.example.com",
        "rust",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--searxng-url has been removed"));

    let mut equals_cmd = wa_bin();
    equals_cmd.args([
        "search",
        "--searxng-url=https://legacy.example.com",
        "rust",
    ]);
    equals_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("--searxng-url has been removed"));
}

#[test]
fn cli_config_redacts_degoog_api_key() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[search]\nprovider = \"degoog\"\n[search.degoog]\napi_key = \"secret-token\"\n",
    )
    .unwrap();

    let mut cmd = wa_bin();
    cmd.args(["--config", config_path.to_str().unwrap(), "config"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("<redacted>"))
        .stdout(predicate::str::contains("secret-token").not());
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

// ---- T67: fetch --endpoints rejects multiple URLs ----------------------------

#[test]
fn fetch_endpoints_rejects_multiple_urls() {
    let mut cmd = wa_bin();
    cmd.args([
        "fetch",
        "--endpoints",
        "https://example.com/",
        "https://example.org/",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("supports only a single URL"));
}
