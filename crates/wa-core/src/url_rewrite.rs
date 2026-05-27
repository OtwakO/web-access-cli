//! URL rewrite engine — transparent regex-based URL transformations.
//!
//! Rules are ordered; first match wins. Applied before every HTTP fetch
//! so the agent surface is unchanged but the underlying request hits a
//! more cooperative host (e.g. old.reddit.com instead of www.reddit.com).

use serde::{Deserialize, Serialize};

/// A single rewrite rule: if `match_regex` matches the input URL, replace
/// with `replace`, supporting `$1`..`$N` capture-group expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlRewriteRule {
    /// Rust regex pattern (e.g. `r"^https?://www\.reddit\.com/(.*)$"`).
    pub match_regex: String,
    /// Replacement string with capture references (e.g. `"https://old.reddit.com/$1"`).
    pub replace: String,
}

/// Compiled, ordered set of rewrite rules.
///
/// First match wins. If no rule matches, the original URL is used unchanged.
#[derive(Debug, Clone)]
pub struct UrlRewriter {
    compiled: Vec<(regex::Regex, String)>,
}

impl UrlRewriter {
    /// Compile a list of rules into a rewriter.
    ///
    /// Returns an error if any rule contains an invalid regex. The error
    /// message includes the rule index for easy debugging.
    pub fn new(rules: &[UrlRewriteRule]) -> Result<Self, crate::error::WaError> {
        let mut compiled = Vec::with_capacity(rules.len());
        for (i, rule) in rules.iter().enumerate() {
            let re = regex::Regex::new(&rule.match_regex).map_err(|e| {
                crate::error::WaError::Config(format!(
                    "url_rewrites[{i}]: invalid regex '{}': {e}",
                    rule.match_regex
                ))
            })?;
            compiled.push((re, rule.replace.clone()));
        }
        Ok(Self { compiled })
    }

    /// Apply the first matching rule to `url`.
    ///
    /// Returns `Some(rewritten)` if a rule matched, `None` if no rule
    /// matched (caller should use the original URL).
    pub fn apply(&self, url: &str) -> Option<String> {
        for (re, replacement) in &self.compiled {
            if re.is_match(url) {
                return Some(re.replace(url, replacement).into_owned());
            }
        }
        None
    }

    /// Returns true if no rules are compiled (identity pass-through).
    pub fn is_empty(&self) -> bool {
        self.compiled.is_empty()
    }
}

impl Default for UrlRewriter {
    fn default() -> Self {
        Self { compiled: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rules() -> Vec<UrlRewriteRule> {
        vec![
            UrlRewriteRule {
                match_regex: r"^https?://www\.reddit\.com/(.*)$".into(),
                replace: "https://old.reddit.com/$1".into(),
            },
            UrlRewriteRule {
                match_regex: r"^https?://(www\.)?medium\.com/(.*)$".into(),
                replace: "https://scribe.rip/$2".into(),
            },
            UrlRewriteRule {
                match_regex: r"^https?://twitter\.com/".into(),
                replace: "https://nitter.net/".into(),
            },
        ]
    }

    #[test]
    fn rewriter_applies_first_match() {
        let rw = UrlRewriter::new(&make_rules()).unwrap();
        assert_eq!(
            rw.apply("https://www.reddit.com/r/rust/comments/abc"),
            Some("https://old.reddit.com/r/rust/comments/abc".into())
        );
    }

    #[test]
    fn rewriter_no_match_returns_none() {
        let rw = UrlRewriter::new(&make_rules()).unwrap();
        assert_eq!(
            rw.apply("https://github.com/octocat/hello-world"),
            None
        );
    }

    #[test]
    fn rewriter_first_match_wins() {
        let rules = vec![
            UrlRewriteRule {
                match_regex: r"^https?://.*\.reddit\.com/.*$".into(),
                replace: "https://old.reddit.com/".into(),
            },
            UrlRewriteRule {
                match_regex: r"^https?://www\.reddit\.com/.*$".into(),
                replace: "https://new.reddit.com/".into(),
            },
        ];
        let rw = UrlRewriter::new(&rules).unwrap();
        // First (broader) rule should win, not the second (more specific)
        assert_eq!(
            rw.apply("https://www.reddit.com/r/rust"),
            Some("https://old.reddit.com/".into())
        );
    }

    #[test]
    fn rewriter_capture_groups() {
        let rules = vec![UrlRewriteRule {
            match_regex: r"^https?://(www\.)?medium\.com/(.*)$".into(),
            replace: "https://scribe.rip/$2".into(),
        }];
        let rw = UrlRewriter::new(&rules).unwrap();
        assert_eq!(
            rw.apply("https://medium.com/@author/article-slug"),
            Some("https://scribe.rip/@author/article-slug".into())
        );
        assert_eq!(
            rw.apply("https://www.medium.com/@author/article-slug"),
            Some("https://scribe.rip/@author/article-slug".into())
        );
    }

    #[test]
    fn rewriter_invalid_regex_error() {
        let rules = vec![UrlRewriteRule {
            match_regex: r"^(unclosed".into(),
            replace: "x".into(),
        }];
        let err = UrlRewriter::new(&rules).unwrap_err().to_string();
        assert!(err.contains("url_rewrites[0]"));
        assert!(err.contains("invalid regex"));
    }

    #[test]
    fn rewriter_empty_rules_is_identity() {
        let rw = UrlRewriter::new(&[]).unwrap();
        assert!(rw.is_empty());
        assert_eq!(rw.apply("https://example.com"), None);
    }

    #[test]
    fn rewriter_preserves_fragment_and_query() {
        let rules = vec![UrlRewriteRule {
            match_regex: r"^https?://old\.site\.com/(.*)$".into(),
            replace: "https://new.site.com/$1".into(),
        }];
        let rw = UrlRewriter::new(&rules).unwrap();
        assert_eq!(
            rw.apply("https://old.site.com/page?foo=1#section"),
            Some("https://new.site.com/page?foo=1#section".into())
        );
    }
}
