//! Conservative extraction-quality diagnostics for fetched HTML documents.

use regex::Regex;
use scraper::{Html, Selector};
use std::sync::OnceLock;

use crate::ExtractionResult;

const SPARSE_EXTRACTION_CHARS: usize = 200;
const MIN_CANDIDATE_CHARS: usize = 500;
const MIN_CANDIDATE_MULTIPLIER: usize = 4;

/// Strong evidence that the extractor omitted a richer semantic content region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteExtraction {
    pub extracted_text_chars: usize,
    pub candidate_text_chars: usize,
}

/// Whether an extraction is sparse enough to justify obtaining the full HTML
/// for a quality check.
pub fn needs_quality_probe(extraction: &ExtractionResult) -> bool {
    extracted_text_chars(extraction) < SPARSE_EXTRACTION_CHARS
}

/// Analyze a full HTML document against its extracted result.
///
/// Returns a diagnostic only when extraction is sparse and a semantic content
/// region contains substantially more text. This intentionally favors avoiding
/// false positives over detecting every incomplete page.
pub fn detect_incomplete_extraction(
    html: &str,
    extraction: &ExtractionResult,
) -> Option<IncompleteExtraction> {
    let extracted_text_chars = extracted_text_chars(extraction);
    if !needs_quality_probe(extraction) {
        return None;
    }

    let document = Html::parse_document(html);
    let selector = Selector::parse("article, main, [role='main'], [itemprop='articleBody']")
        .expect("static semantic-content selector is valid");
    let mut candidate_text_chars = richest_semantic_region(&document, &selector);

    // HTML parsers treat <noscript> contents as raw text when scripting is
    // enabled. Parse those standard fallback payloads as fragments, then apply
    // the same semantic-region rule; no site or framework knowledge is needed.
    let noscript_selector = Selector::parse("noscript").expect("static noscript selector is valid");
    for noscript in document.select(&noscript_selector) {
        let payload = noscript.text().collect::<String>();
        let fragment = Html::parse_fragment(&payload);
        candidate_text_chars =
            candidate_text_chars.max(richest_semantic_region(&fragment, &selector));
    }

    if candidate_text_chars >= MIN_CANDIDATE_CHARS
        && candidate_text_chars
            >= extracted_text_chars
                .max(1)
                .saturating_mul(MIN_CANDIDATE_MULTIPLIER)
    {
        Some(IncompleteExtraction {
            extracted_text_chars,
            candidate_text_chars,
        })
    } else {
        None
    }
}

fn extracted_text_chars(extraction: &ExtractionResult) -> usize {
    if !extraction.content.plain_text.trim().is_empty() {
        return text_chars(&extraction.content.plain_text);
    }
    text_chars(&visible_markdown_text(&extraction.content.markdown))
}

fn visible_markdown_text(markdown: &str) -> String {
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    let without_images = IMAGE
        .get_or_init(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").expect("static image regex is valid"))
        .replace_all(markdown, "");
    LINK.get_or_init(|| Regex::new(r"\[([^\]]+)\]\([^)]*\)").expect("static link regex is valid"))
        .replace_all(&without_images, "$1")
        .into_owned()
}

fn richest_semantic_region(document: &Html, selector: &Selector) -> usize {
    let hidden = Selector::parse("script, style, template, svg")
        .expect("static hidden-content selector is valid");
    document
        .select(selector)
        .map(|element| {
            let mut fragment = Html::parse_fragment(&element.inner_html());
            for node in fragment
                .select(&hidden)
                .map(|node| node.id())
                .collect::<Vec<_>>()
            {
                if let Some(mut node) = fragment.tree.get_mut(node) {
                    node.detach();
                }
            }
            text_chars(&fragment.root_element().text().collect::<String>())
        })
        .max()
        .unwrap_or(0)
}

fn text_chars(text: &str) -> usize {
    text.chars().filter(|ch| !ch.is_whitespace()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Content, Metadata};

    fn extraction(plain_text: &str) -> ExtractionResult {
        extraction_with_markdown(plain_text, plain_text)
    }

    fn extraction_with_markdown(plain_text: &str, markdown: &str) -> ExtractionResult {
        ExtractionResult {
            metadata: Metadata {
                title: None,
                description: None,
                author: None,
                published_date: None,
                language: None,
                url: None,
                site_name: None,
                image: None,
                favicon: None,
                word_count: 0,
            },
            content: Content {
                markdown: markdown.into(),
                plain_text: plain_text.into(),
                links: Vec::new(),
                images: Vec::new(),
                code_blocks: Vec::new(),
                raw_html: None,
            },
            domain_data: None,
            structured_data: Vec::new(),
        }
    }

    #[test]
    fn quality_probe_is_limited_to_sparse_extractions() {
        assert!(needs_quality_probe(&extraction("Short page")));
        assert!(!needs_quality_probe(&extraction(
            &"Substantial text. ".repeat(30)
        )));
        assert!(!needs_quality_probe(&extraction_with_markdown(
            "",
            &"Rich vertical-extractor markdown. ".repeat(20),
        )));
        assert!(needs_quality_probe(&extraction_with_markdown(
            "",
            &format!("[short title](https://example.com/{})", "x".repeat(300)),
        )));
    }

    #[test]
    fn warns_when_sparse_extraction_omits_rich_semantic_content() {
        let article = "Substantial article sentence with useful detail. ".repeat(20);
        let html = format!(
            "<html><body><div id='app'></div><noscript><article>{article}</article></noscript></body></html>"
        );

        let diagnostic = detect_incomplete_extraction(&html, &extraction("Only a title"))
            .expect("rich omitted article should be detected");

        assert!(diagnostic.candidate_text_chars >= 500);
        assert!(diagnostic.candidate_text_chars > diagnostic.extracted_text_chars * 4);
    }

    #[test]
    fn ignores_complete_article_extraction() {
        let article = "Complete article sentence with useful detail. ".repeat(20);
        let html = format!("<html><body><article>{article}</article></body></html>");

        assert!(detect_incomplete_extraction(&html, &extraction(&article)).is_none());
    }

    #[test]
    fn ignores_hidden_payload_inside_semantic_content() {
        let payload = "window.__DATA__ = 'x';".repeat(50);
        let html = format!(
            "<html><body><article><h1>Short page</h1><script>{payload}</script></article></body></html>"
        );

        assert!(detect_incomplete_extraction(&html, &extraction("Short page")).is_none());
    }

    #[test]
    fn ignores_large_document_without_semantic_content_evidence() {
        let navigation = "Navigation and interface text. ".repeat(40);
        let html = format!("<html><body><div>{navigation}</div></body></html>");

        assert!(detect_incomplete_extraction(&html, &extraction("Short page")).is_none());
    }
}
