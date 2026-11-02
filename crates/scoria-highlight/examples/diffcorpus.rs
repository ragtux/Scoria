//! Prints the first difference between Scoria's HTML and Obsidian's, per
//! language, which is how a ported grammar is debugged.
//!
//!     cargo run -p scoria-highlight --example diffcorpus [language]
use scoria_highlight::{languages, to_html, tokenize};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Sample {
    source: String,
    html: String,
}

fn main() {
    let text = include_str!("../tests/corpus/obsidian-1.14.5.json");
    let corpus: BTreeMap<String, Sample> = serde_json::from_str(text).unwrap();
    let only = std::env::args().nth(1);
    for (lang, sample) in &corpus {
        if only.as_deref().is_some_and(|o| o != lang) {
            continue;
        }
        let Some(g) = languages::grammar(lang) else {
            continue;
        };
        let got = to_html(&sample.source, &tokenize(&sample.source, g));
        if got == sample.html {
            println!("{lang}: OK");
            continue;
        }
        let at = got
            .char_indices()
            .zip(sample.html.char_indices())
            .find(|((_, a), (_, b))| a != b)
            .map(|((i, _), _)| i)
            .unwrap_or(got.len().min(sample.html.len()));
        let from = at.saturating_sub(70);
        println!("{lang}: differ at byte {at}");
        println!(
            "  want ...{}",
            &sample.html[from..(at + 150).min(sample.html.len())]
        );
        println!("  got  ...{}", &got[from..(at + 150).min(got.len())]);
        println!();
    }
}
