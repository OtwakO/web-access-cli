#[cfg(test)]
mod tests {
    use wa_core::config::Config;

    // ---- T1: config_defaults ------------------------------------------------

    #[test]
    fn config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.searxng_url, "http://localhost:8080");
        assert_eq!(cfg.fetch_timeout_secs, 12);
        assert_eq!(cfg.browser_profile, "chrome");
        assert_eq!(cfg.proxy, None);
        assert_eq!(cfg.max_file_size, 102_400);
        assert_eq!(cfg.max_files, 100);
        assert_eq!(cfg.retries, 3);
        assert_eq!(cfg.retry_delay_ms, 500);
    }

    // ---- T2: config_from_toml_file ------------------------------------------

    #[test]
    fn config_from_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
searxng_url = "https://searx.example.com"
browser_profile = "firefox"
max_files = 50
"#,
        )
        .unwrap();

        temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
            let cfg = Config::load(Some(&path)).unwrap();
            assert_eq!(cfg.searxng_url, "https://searx.example.com");
            assert_eq!(cfg.browser_profile, "firefox");
            assert_eq!(cfg.max_files, 50);
            // these should still be defaults
            assert_eq!(cfg.fetch_timeout_secs, 12);
            assert_eq!(cfg.max_file_size, 102_400);
        });
    }

    // ---- T3: config_from_toml_partial ---------------------------------------

    #[test]
    fn config_from_toml_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial.toml");
        std::fs::write(&path, r#"max_files = 10"#).unwrap();

        temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
            let cfg = Config::load(Some(&path)).unwrap();
            assert_eq!(cfg.max_files, 10);
            // everything else should be default
            assert_eq!(cfg.searxng_url, "http://localhost:8080");
            assert_eq!(cfg.retries, 3);
        });
    }

    // ---- T4: config_from_invalid_toml ---------------------------------------

    #[test]
    fn config_from_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "not valid toml {{{").unwrap();

        temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
            let err = Config::load(Some(&path)).unwrap_err();
            let msg = format!("{}", err);
            assert!(msg.contains("invalid TOML") || msg.contains("config error"));
        });
    }

    // ---- T5: config_env_override --------------------------------------------

    #[test]
    fn config_env_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        std::fs::write(&path, r#"searxng_url = "https://from-file.example.com""#).unwrap();

        temp_env::with_var("WA_SEARXNG_URL", Some("https://from-env.example.com"), || {
            let cfg = Config::load(Some(&path)).unwrap();
            assert_eq!(cfg.searxng_url, "https://from-env.example.com");
        });
    }

    // ---- T6: config_cli_override --------------------------------------------

    #[test]
    fn config_cli_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        std::fs::write(&path, r#"max_files = 30"#).unwrap();

        temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
            let mut cfg = Config::load(Some(&path)).unwrap();
            assert_eq!(cfg.max_files, 30);

            // Simulate CLI override
            cfg.max_files = 5;
            assert_eq!(cfg.max_files, 5);
        });
    }

    // ---- T7: config_explicit_path_not_found — error -------------------------

    #[test]
    fn config_explicit_path_not_found() {
        let path = std::path::Path::new("/tmp/does-not-exist-923847.toml");
        temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
            let err = Config::load(Some(path)).unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("not found"),
                "expected 'not found' error, got: {}",
                msg
            );
        });
    }

    // ---- T8: config_auto_discovery_silent — no file → defaults ---------------

    #[test]
    fn config_auto_discovery_silent() {
        // When no explicit path and no config file exists at the auto path,
        // we should get defaults + env, not an error.
        // We set HOME to a temp dir with no config file to verify this.
        let dir = tempfile::tempdir().unwrap();
        temp_env::with_var("HOME", Some(dir.path().to_str().unwrap()), || {
            temp_env::with_var("XDG_CONFIG_HOME", None::<&str>, || {
                temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
                    let cfg = Config::load(None).unwrap();
                    assert_eq!(cfg.searxng_url, "http://localhost:8080");
                    assert_eq!(cfg.retries, 3);
                });
            });
        });
    }

    // ---- T9: config_init — scaffolds file -----------------------------------

    #[test]
    fn config_init_scaffolds_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let result = Config::init_config_file(Some(&path)).unwrap();
        assert_eq!(result, path);
        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# wa configuration"));
        assert!(contents.contains("searxng_url"));
        assert!(contents.contains("browser_profile"));
        assert!(contents.contains("proxy"));
    }

    // ---- T10: config_init_already_exists — error ----------------------------

    #[test]
    fn config_init_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.toml");
        std::fs::write(&path, "searxng_url = \"https://example.com\"").unwrap();

        let err = Config::init_config_file(Some(&path)).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("already exists"));
    }

    // ---- T11: config_deny_unknown_fields ------------------------------------

    #[test]
    fn config_deny_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unknown.toml");
        std::fs::write(
            &path,
            r#"
searxng_url = "https://example.com"
typo_field = "oops"
"#,
        )
        .unwrap();

        temp_env::with_var("WA_SEARXNG_URL", None::<&str>, || {
            let err = Config::load(Some(&path)).unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("unknown field") || msg.contains("invalid TOML"),
                "expected unknown field error, got: {}",
                msg
            );
        });
    }

    // ---- T12: output_format_serialization -----------------------------------

    #[test]
    fn output_format_serialization() {
        // Lowercase round-trip as defined by #[serde(rename_all = "lowercase")]
        let json = serde_json::to_string(&wa_core::types::OutputFormat::Markdown).unwrap();
        assert_eq!(json, r#""markdown""#);

        let parsed: wa_core::types::OutputFormat =
            serde_json::from_str(r#""llm""#).unwrap();
        assert_eq!(parsed, wa_core::types::OutputFormat::Llm);

        let parsed: wa_core::types::OutputFormat =
            serde_json::from_str(r#""text""#).unwrap();
        assert_eq!(parsed, wa_core::types::OutputFormat::Text);

        let parsed: wa_core::types::OutputFormat =
            serde_json::from_str(r#""json""#).unwrap();
        assert_eq!(parsed, wa_core::types::OutputFormat::Json);
    }
}
