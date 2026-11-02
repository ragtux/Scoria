//! Every ported pattern compiles.
//!
//! A pattern that does not is silently inert at runtime — the engine skips
//! it rather than panicking — so without this test a mistranslated regex
//! would show up only as a language that quietly stops highlighting one
//! kind of token. JavaScript's syntax is close to Rust's but not the same:
//! `/…/i` has to become `(?i)…`, `\/` is not an escape, and a `[` inside a
//! character class has to be escaped.

use scoria_highlight::compiled::compile;
use scoria_highlight::languages::LANGUAGES;

#[test]
fn every_pattern_compiles() {
    let mut bad = Vec::new();
    let mut seen = 0;
    let mut stack: Vec<&'static scoria_highlight::grammar::Grammar> =
        LANGUAGES.iter().map(|(_, _, g)| *g).collect();
    let mut done: Vec<usize> = Vec::new();
    while let Some(g) = stack.pop() {
        let key = g as *const _ as usize;
        if done.contains(&key) {
            continue;
        }
        done.push(key);
        for (kind, patterns) in &compile(g).entries {
            for (ix, pat) in patterns.iter().enumerate() {
                seen += 1;
                if pat.re.is_none() {
                    bad.push(format!("{kind}[{ix}]"));
                }
                if let Some(inside) = pat.inside {
                    stack.push(inside);
                }
            }
        }
    }
    assert!(seen > 0, "no patterns were reached");
    assert!(
        bad.is_empty(),
        "{} of {seen} patterns did not compile: {bad:#?}",
        bad.len()
    );
}

/// Every name and alias resolves, and nothing is registered twice.
#[test]
fn the_language_table_is_unambiguous() {
    let mut names = Vec::new();
    for (id, aliases, _) in LANGUAGES {
        names.push(*id);
        names.extend(aliases.iter().copied());
    }
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "a name is registered twice");
    for name in names {
        assert!(
            scoria_highlight::languages::grammar(name).is_some(),
            "{name} does not resolve"
        );
        assert_eq!(name, name.to_ascii_lowercase(), "{name} is not lowercase");
    }
}
