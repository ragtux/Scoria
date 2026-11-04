//! Prints the first difference between Scoria's HTML and the corpus's, per
//! language and per corpus (Obsidian's, then Prism's), which is how a
//! ported grammar is debugged.
//!
//!     cargo run -p scoria-highlight --example diffcorpus [language]
use scoria_highlight::{to_html, tokenize_language};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Sample {
    source: String,
    html: String,
}

fn main() {
    let corpora = [
        (
            "obsidian",
            include_str!("../tests/corpus/obsidian-1.14.5.json"),
        ),
        ("prism", include_str!("../tests/corpus/prism-1.29.0.json")),
    ];
    let only = std::env::args().nth(1);
    for (name, text) in corpora {
        let corpus: BTreeMap<String, Sample> = serde_json::from_str(text).unwrap();
        for (lang, sample) in &corpus {
            if only.as_deref().is_some_and(|o| o != lang) {
                continue;
            }
            let Some(nodes) = tokenize_language(&sample.source, lang) else {
                continue;
            };
            let got = to_html(&sample.source, &nodes);
            if got == sample.html {
                println!("{name} {lang}: OK");
                continue;
            }
            let at = got
                .char_indices()
                .zip(sample.html.char_indices())
                .find(|((_, a), (_, b))| a != b)
                .map(|((i, _), _)| i)
                .unwrap_or(got.len().min(sample.html.len()));
            let from =
                floor(&got, at.saturating_sub(70)).min(floor(&sample.html, at.saturating_sub(70)));
            println!("{name} {lang}: differ at byte {at}");
            println!(
                "  want ...{}",
                &sample.html[from..floor(&sample.html, at + 150)]
            );
            println!("  got  ...{}", &got[from..floor(&got, at + 150)]);
            println!();
        }
    }
}

/// `ix` moved back to a character boundary of `s`, and no further than its end.
fn floor(s: &str, ix: usize) -> usize {
    let mut ix = ix.min(s.len());
    while !s.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}
