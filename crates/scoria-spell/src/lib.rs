//! Spellchecking, without gpui.
//!
//! Obsidian's editor is spellchecked by Chromium's Hunspell-based checker;
//! Scoria checks with [spellbook](https://crates.io/crates/spellbook), a pure
//! Rust reader of the same Hunspell `.aff`/`.dic` dictionaries. This crate
//! owns:
//!
//! - [`dict`]: the bundled `en_US` (SCOWL's, the source of Chromium's own
//!   `en-US`) and dictionaries read from disk;
//! - [`discover`]: where those are looked for (the desktop's Hunspell
//!   folders and Scoria's configuration folder) and how a language code
//!   picks one;
//! - [`custom`]: the "Add to dictionary" words, in Chromium's
//!   `Custom Dictionary.txt` format;
//! - [`text`]: which bytes of a note are checked (from `scoria-markdown`'s
//!   tree) and how they split into words;
//! - [`Speller`]: all of it together, and [`Speller::misspellings`], what
//!   the editor squiggles.

pub mod custom;
pub mod dict;
pub mod discover;
pub mod text;

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub use custom::CustomDictionary;
pub use dict::Dictionary;

/// The most corrections the editor menu offers: Chromium's three.
pub const MAX_SUGGESTIONS: usize = 3;

/// The dictionaries of the chosen languages and the custom words. Cheap to
/// clone: the dictionaries are shared. Remembers each word's verdict, so a
/// note rechecked on every keystroke looks up each distinct word once.
#[derive(Clone)]
pub struct Speller {
    dictionaries: Arc<Vec<Arc<Dictionary>>>,
    custom: Arc<CustomDictionary>,
    verdicts: Arc<Mutex<HashMap<String, bool>>>,
}

impl std::fmt::Debug for Speller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speller")
            .field("languages", &self.languages())
            .field("custom", &self.custom.len())
            .finish()
    }
}

impl Speller {
    pub fn new(dictionaries: Vec<Arc<Dictionary>>, custom: CustomDictionary) -> Speller {
        Speller {
            dictionaries: Arc::new(dictionaries),
            custom: Arc::new(custom),
            verdicts: Arc::default(),
        }
    }

    /// The languages checked, in order.
    pub fn languages(&self) -> Vec<&str> {
        self.dictionaries
            .iter()
            .map(|d| d.language.as_str())
            .collect()
    }

    pub fn custom(&self) -> &CustomDictionary {
        &self.custom
    }

    /// The same dictionaries with another custom word list (after "Add to
    /// dictionary"); the verdicts start over.
    pub fn with_custom(&self, custom: CustomDictionary) -> Speller {
        Speller {
            dictionaries: self.dictionaries.clone(),
            custom: Arc::new(custom),
            verdicts: Arc::default(),
        }
    }

    /// Whether one word (as [`text::words`] cuts it) is spelled right: a
    /// custom word, a word of any chosen language, or a joined word whose
    /// every part is one. With no dictionary at all nothing is wrong.
    pub fn check(&self, word: &str) -> bool {
        match self.verdicts.lock() {
            Ok(mut m) => self.check_with(&mut m, word),
            Err(_) => self.dictionaries.is_empty() || self.check_uncached(word),
        }
    }

    /// [`Speller::check`] with the verdicts already locked, so a whole
    /// note takes the lock once.
    fn check_with(&self, verdicts: &mut HashMap<String, bool>, word: &str) -> bool {
        if self.dictionaries.is_empty() || !text::is_checked(word) {
            return true;
        }
        if let Some(&v) = verdicts.get(word) {
            return v;
        }
        let v = self.check_uncached(word);
        if verdicts.len() > 200_000 {
            verdicts.clear();
        }
        verdicts.insert(word.to_string(), v);
        v
    }

    fn check_one(&self, word: &str) -> bool {
        !text::is_checked(word)
            || self.custom.contains(word)
            || self.dictionaries.iter().any(|d| d.check(word))
    }

    fn check_uncached(&self, word: &str) -> bool {
        if self.check_one(word) {
            return true;
        }
        let parts = text::parts(word);
        parts.len() > 1 && parts.iter().all(|(_, p)| self.check_one(p))
    }

    /// Corrections for a misspelled word, at most [`MAX_SUGGESTIONS`], the
    /// chosen languages' interleaved in order, without repeats.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let lists: Vec<Vec<String>> = self.dictionaries.iter().map(|d| d.suggest(word)).collect();
        let mut out: Vec<String> = Vec::new();
        let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
        for i in 0..longest {
            for l in &lists {
                if let Some(s) = l.get(i) {
                    if !out.contains(s) {
                        out.push(s.clone());
                    }
                }
            }
        }
        out.truncate(MAX_SUGGESTIONS);
        out
    }

    /// The misspelled words of a parsed note, as byte ranges of its source,
    /// in order. `typing` is the caret while the word before it is being
    /// typed: a word that ends there is left alone until a space or a move
    /// ends it, as Chromium leaves it (observed 2026-11-15).
    pub fn misspellings(
        &self,
        doc: &scoria_markdown::Document<'_>,
        typing: Option<usize>,
    ) -> Vec<Range<usize>> {
        let src = doc.source;
        let mut out = Vec::new();
        if self.dictionaries.is_empty() {
            return out;
        }
        let Ok(mut verdicts) = self.verdicts.lock() else {
            return out;
        };
        for range in text::checked_ranges(doc) {
            let base = range.start;
            text::for_each_word(&src[range], |at, word| {
                let start = base + at;
                let end = start + word.len();
                if typing != Some(end) && !self.check_with(&mut verdicts, word) {
                    out.push(start..end);
                }
            });
        }
        out
    }

    /// The misspelled word at byte `offset` of `src`, if any: its range in
    /// `ranges` (as [`Speller::misspellings`] returned them).
    pub fn word_at(ranges: &[Range<usize>], offset: usize) -> Option<Range<usize>> {
        let i = ranges.partition_point(|r| r.end < offset);
        ranges
            .get(i)
            .filter(|r| r.start <= offset && offset <= r.end)
            .cloned()
    }
}

/// Loads the dictionaries for `languages` (Obsidian's codes): each from the
/// first folder that has it, the bundled `en-US` when none does. A language
/// with no dictionary anywhere is skipped and named in the second list.
pub fn load_languages(
    languages: &[String],
    config_dir: Option<&Path>,
) -> (Vec<Arc<Dictionary>>, Vec<String>) {
    let found = discover::find(&discover::search_dirs(config_dir));
    let mut dicts: Vec<Arc<Dictionary>> = Vec::new();
    let mut missing = Vec::new();
    for lang in languages {
        if dicts.iter().any(|d| d.language.eq_ignore_ascii_case(lang)) {
            continue;
        }
        let loaded = match discover::resolve(lang, &found) {
            Some(f) => Dictionary::load(&f.language, &f.aff, &f.dic).ok(),
            None => None,
        };
        match loaded {
            Some(d) => dicts.push(Arc::new(d)),
            None if is_bundled(lang) => dicts.push(Arc::new(Dictionary::bundled())),
            None => missing.push(lang.clone()),
        }
    }
    (dicts, missing)
}

/// Whether a code names the bundled dictionary: `en-US`, or a bare `en`.
pub fn is_bundled(language: &str) -> bool {
    let l = discover::language_code(language);
    l.eq_ignore_ascii_case(dict::BUNDLED_LANGUAGE) || l.eq_ignore_ascii_case("en")
}

/// Obsidian's default languages: `en-US` (the `spellcheck-languages` it
/// keeps holds `["en-US"]` until one is added).
pub fn default_languages() -> Vec<String> {
    vec![dict::BUNDLED_LANGUAGE.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speller() -> Speller {
        Speller::new(
            vec![Arc::new(Dictionary::bundled())],
            CustomDictionary::default(),
        )
    }

    fn flagged<'a>(s: &Speller, src: &'a str) -> Vec<&'a str> {
        let doc = scoria_markdown::parse(src);
        s.misspellings(&doc, None)
            .into_iter()
            .map(|r| &src[r])
            .collect()
    }

    #[test]
    fn the_probes_words_are_flagged_as_chromium_flagged_them() {
        let s = speller();
        let src = "zorp42 abc123xyz 2ndd 42nd 1st 3rd MISPELED Mispeled mIspeled camelCaseWrod dont dont't don't don’t recieve's self-awarness snake_case_wrod teh Imaage.png e.g. naive x I i Zorpville ZORPVILLE wrold. it’s recieve’s v2 ABCs URLs mp3 COVID-19 web3 hello_world foo.bar a.b.c o'clock 'quoted' rock'n'roll zorp·ville Dr. Mr iPhone macOS GitHub JavaScript e-mail co-operate end\n";
        assert_eq!(
            flagged(&s, src),
            [
                "zorp42",
                "abc123xyz",
                "2ndd",
                "MISPELED",
                "Mispeled",
                "mIspeled",
                "camelCaseWrod",
                "dont",
                "dont't",
                "recieve's",
                "awarness",
                "wrod",
                "teh",
                "Imaage.png",
                "Zorpville",
                "ZORPVILLE",
                "wrold",
                "recieve’s",
                "v2",
                "mp3",
                "web3",
                "zorp·ville",
                "macOS"
            ]
        );
    }

    #[test]
    fn the_word_being_typed_waits() {
        let s = speller();
        let src = "Hello wrold";
        let doc = scoria_markdown::parse(src);
        assert!(s.misspellings(&doc, Some(src.len())).is_empty());
        let flagged = s.misspellings(&doc, Some(5));
        assert_eq!((flagged.len(), flagged[0].clone()), (1, 6..11));
    }

    #[test]
    fn custom_words_and_suggestions() {
        let s = speller();
        assert!(!s.check("qzxqzxqzvv"));
        assert!(s.suggest("qzxqzxqzvv").is_empty());
        let mut c = CustomDictionary::default();
        c.add("qzxqzxqzvv");
        let s2 = s.with_custom(c);
        assert!(s2.check("qzxqzxqzvv"));
        assert!(s2.check("QZXQZXQZVV"));
        assert_eq!(s.suggest("wrold")[0], "world");
        assert!(s.suggest("recieve").len() <= MAX_SUGGESTIONS);
    }

    #[test]
    fn word_at_finds_the_range_under_an_offset() {
        let r = vec![2..5, 9..12];
        assert_eq!(Speller::word_at(&r, 2), Some(2..5));
        assert_eq!(Speller::word_at(&r, 5), Some(2..5));
        assert_eq!(Speller::word_at(&r, 7), None);
        assert_eq!(Speller::word_at(&r, 11), Some(9..12));
    }

    #[test]
    fn no_dictionary_flags_nothing() {
        let s = Speller::new(Vec::new(), CustomDictionary::default());
        assert!(flagged(&s, "wrold teh").is_empty());
    }

    #[test]
    fn an_unknown_language_is_reported_missing_and_en_falls_back_to_the_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let (d, missing) = load_languages(&["en".into(), "xx-YY".into()], Some(dir.path()));
        assert_eq!(d.len(), 1);
        assert_eq!(missing, ["xx-YY"]);
    }
}
