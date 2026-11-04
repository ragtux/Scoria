//! The ratchets: Obsidian's own output, and Prism's, token for token.
//!
//! `tests/corpus/obsidian-1.14.5.json` holds, per language, a sample of
//! source and the exact `innerHTML` Obsidian 1.14.5 rendered it to — read
//! off a real Obsidian by `tools/obsidian-probe/code-highlight.mjs` on
//! 2026-11-02. A ported grammar is correct when its HTML is identical, and
//! that is a strong test: it compares every token's name, its aliases, its
//! nesting and its boundaries at once.
//!
//! `tests/corpus/prism-1.29.0.json` is the same thing with Prism itself as
//! the renderer: `tools/prism-port/port.js` loads Prism the way Obsidian's
//! copy is loaded — with that order it reproduces every Obsidian sample
//! above byte for byte — and renders the longer samples in
//! `tools/prism-port/samples/`, one per ported language. It is what holds
//! the languages the Obsidian probe has no sample for.
//!
//! A language with no grammar yet is listed as unported rather than failed,
//! so a corpus can hold more samples than the crate has languages.

use scoria_highlight::{languages, to_html, tokenize_language};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Sample {
    source: String,
    html: String,
}

fn corpus(text: &str) -> BTreeMap<String, Sample> {
    serde_json::from_str(text).expect("the corpus parses")
}

fn obsidian() -> BTreeMap<String, Sample> {
    corpus(include_str!("corpus/obsidian-1.14.5.json"))
}

fn prism() -> BTreeMap<String, Sample> {
    corpus(include_str!("corpus/prism-1.29.0.json"))
}

/// Renders every sample whose language is ported and panics with each one
/// that differs; returns the languages that matched.
fn check(name: &str, corpus: &BTreeMap<String, Sample>) -> Vec<String> {
    let mut unported = Vec::new();
    let mut checked = Vec::new();
    let mut wrong = Vec::new();
    for (lang, sample) in corpus {
        let Some(nodes) = tokenize_language(&sample.source, lang) else {
            unported.push(lang.clone());
            continue;
        };
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
                "\n=== {lang} ===\n{name}: {want}\nScoria:   {got}\n"
            ));
        }
        panic!(
            "{} of {} ported languages differ from {name}:{report}",
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
    checked
}

/// Every ported language renders exactly as Obsidian rendered it.
#[test]
fn the_ported_languages_match_obsidian() {
    check("Obsidian", &obsidian());
}

/// Every ported language renders exactly as Prism renders it.
#[test]
fn the_ported_languages_match_prism() {
    check("Prism", &prism());
}

/// No language is ported without a sample to hold it to: each one in the
/// table has a Prism sample under its own id.
#[test]
fn every_ported_language_has_a_prism_sample() {
    let prism = prism();
    let missing: Vec<_> = languages::LANGUAGES
        .iter()
        .map(|(id, _, _)| *id)
        .filter(|id| !prism.contains_key(*id))
        .collect();
    assert!(missing.is_empty(), "no Prism sample for {missing:?}");
}

/// The corpora's own bookkeeping: a sample's HTML is the source with markup
/// around it, so stripping the markup gives the source back. A sample that
/// fails this was recorded wrong and would make the tests above a lie.
#[test]
fn every_sample_s_html_carries_its_own_source() {
    for (lang, sample) in obsidian().into_iter().chain(prism()) {
        let text = strip_tags(&sample.html);
        assert_eq!(
            text, sample.source,
            "{lang}'s recorded HTML does not spell out its source"
        );
    }
}

/// The Obsidian probe's own record of each block: the class Obsidian gave
/// the `code` element, and how many tokens it counted.
#[test]
fn the_obsidian_samples_are_what_the_probe_saw() {
    #[derive(serde::Deserialize)]
    struct Probe {
        html: String,
        #[serde(rename = "codeClass")]
        code_class: String,
        #[serde(rename = "tokenCount")]
        token_count: usize,
    }
    let probes: BTreeMap<String, Probe> =
        serde_json::from_str(include_str!("corpus/obsidian-1.14.5.json")).unwrap();
    for (lang, sample) in probes {
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
