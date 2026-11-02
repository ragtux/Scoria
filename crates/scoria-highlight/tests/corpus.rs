//! The ratchet: Obsidian's own output, token for token.
//!
//! `tests/corpus/obsidian-1.14.5.json` holds, per language, a sample of
//! source and the exact `innerHTML` Obsidian 1.14.5 rendered it to — read
//! off a real Obsidian by `tools/obsidian-probe/code-highlight.mjs` on
//! 2026-11-02. A ported grammar is correct when its HTML is identical, and
//! that is a strong test: it compares every token's name, its aliases, its
//! nesting and its boundaries at once.
//!
//! A language with no grammar yet is listed as unported rather than failed,
//! so the corpus can hold more samples than the crate has languages and the
//! next one ported is a line in `LANGUAGES`, not a change here.

use scoria_highlight::{languages, to_html, tokenize};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Sample {
    source: String,
    #[serde(rename = "codeClass")]
    code_class: String,
    html: String,
    #[serde(rename = "tokenCount")]
    token_count: usize,
}

fn corpus() -> BTreeMap<String, Sample> {
    let text = include_str!("corpus/obsidian-1.14.5.json");
    serde_json::from_str(text).expect("the corpus parses")
}

/// Every ported language renders exactly as Obsidian rendered it.
#[test]
fn the_ported_languages_match_obsidian() {
    let corpus = corpus();
    let mut unported = Vec::new();
    let mut checked = Vec::new();
    let mut wrong = Vec::new();
    for (lang, sample) in &corpus {
        let Some(grammar) = languages::grammar(lang) else {
            unported.push(lang.clone());
            continue;
        };
        let nodes = tokenize(&sample.source, grammar);
        let got = to_html(&sample.source, &nodes);
        if got == sample.html {
            checked.push(lang.clone());
        } else {
            wrong.push((lang.clone(), sample.html.clone(), got));
        }
    }
    if !wrong.is_empty() {
        let mut report = String::new();
        for (lang, want, got) in &wrong {
            report.push_str(&format!(
                "\n=== {lang} ===\nObsidian: {want}\nScoria:   {got}\n"
            ));
        }
        panic!(
            "{} of {} ported languages differ from Obsidian:{report}",
            wrong.len(),
            wrong.len() + checked.len()
        );
    }
    assert!(
        !checked.is_empty(),
        "no ported language was in the corpus at all"
    );
    eprintln!("matched: {checked:?}");
    eprintln!("not ported yet: {unported:?}");
}

/// The corpus's own bookkeeping: a sample's HTML is the source with markup
/// around it, so stripping the markup gives the source back. A sample that
/// fails this was recorded wrong and would make the test above a lie.
#[test]
fn every_sample_s_html_carries_its_own_source() {
    for (lang, sample) in corpus() {
        let text = strip_tags(&sample.html);
        assert_eq!(
            text, sample.source,
            "{lang}'s recorded HTML does not spell out its source"
        );
        assert!(
            sample.code_class.contains(&format!("language-{lang}"))
                || sample.code_class.contains("language-none"),
            "{lang} has code class {}",
            sample.code_class
        );
        if sample.token_count == 0 {
            assert!(
                !sample.html.contains("<span"),
                "{lang} has no tokens but has spans"
            );
        }
    }
}

/// `<span …>`/`</span>` out, `&amp;`/`&lt;`/`&gt;` back in.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(ix) = rest.find('<') {
        out.push_str(&rest[..ix]);
        let Some(end) = rest[ix..].find('>') else {
            break;
        };
        rest = &rest[ix + end + 1..];
    }
    out.push_str(rest);
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}
