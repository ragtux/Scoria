//! The spellchecker in the app: which dictionaries are loaded, the custom
//! words, and the squiggle the editor paints under a misspelled word.
//!
//! What is checked, how words break and the dictionaries themselves are
//! `scoria-spell`'s. This module holds the session's [`Spelling`] (a gpui
//! global every editor watches, so a change of languages, of the custom
//! words or of the `spellcheck` setting rechecks every open note), loads
//! the dictionaries off the main thread, and paints.
//!
//! Observed in Obsidian 1.15.0 on 2026-11-15
//! ([[Specification#Observed spellcheck, Obsidian 1.15.0 (installer build 1.13.7)]]): the squiggle
//! is Chromium's, `#ff0000` in both themes, a wave of period 6 px whose
//! stroke spans 5 px down to the bottom of the font's box, the same under
//! a heading, bold text or plain; the languages are kept in local storage
//! (`spellcheck-languages`), the added words in Chromium's
//! `Custom Dictionary.txt` in the data folder, both outside the vault.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{point, px, App, Bounds, Global, Hsla, PathBuilder, Pixels, Point, Window, WrappedLine};
use scoria_spell::{CustomDictionary, Speller};

use crate::preview::{DisplayLine, Piece};

/// The session's spellchecker.
pub struct Spelling {
    /// `spellcheck` in `app.json` (on when absent).
    pub enabled: bool,
    /// The chosen languages, Obsidian's codes.
    pub languages: Vec<String>,
    /// The loaded dictionaries and custom words; `None` until the first
    /// load finishes.
    pub speller: Option<Speller>,
    /// Languages chosen that no dictionary was found for.
    pub missing: Vec<String>,
    /// Every language a dictionary was found for, the bundled one too.
    pub available: Vec<String>,
    /// A load in flight: a second request waits for it.
    loading: bool,
    reload: bool,
}

impl Global for Spelling {}

/// Scoria's configuration folder, where the custom dictionary and the
/// `dictionaries` folder live.
fn config_dir() -> Option<PathBuf> {
    scoria_vault::registry::config_dir()
}

/// Where the custom words are kept: `Custom Dictionary.txt` under the
/// configuration folder, Chromium's name and bytes.
pub fn custom_path() -> Option<PathBuf> {
    config_dir().map(|d| CustomDictionary::path_in(&d))
}

/// The languages kept in `scoria.json`, Obsidian's default when none are.
fn stored_languages() -> Vec<String> {
    scoria_vault::registry::VaultRegistry::open()
        .ok()
        .flatten()
        .and_then(|r| r.spellcheck_languages().map(<[String]>::to_vec))
        .filter(|l| !l.is_empty())
        .unwrap_or_else(scoria_spell::default_languages)
}

/// The speller the editors check with: the loaded one while the setting is
/// on, nothing while it is off or still loading.
pub fn active(cx: &App) -> Option<Speller> {
    let s = cx.try_global::<Spelling>()?;
    if s.enabled {
        s.speller.clone()
    } else {
        None
    }
}

/// Follows `app.json`'s `spellcheck`. The first call loads the
/// dictionaries.
pub fn set_enabled(enabled: bool, cx: &mut App) {
    if !cx.has_global::<Spelling>() {
        cx.set_global(Spelling {
            enabled,
            languages: stored_languages(),
            speller: None,
            missing: Vec::new(),
            available: Vec::new(),
            loading: false,
            reload: false,
        });
        load(cx);
        return;
    }
    if cx.global::<Spelling>().enabled != enabled {
        cx.global_mut::<Spelling>().enabled = enabled;
    }
}

/// The chosen languages.
pub fn languages(cx: &App) -> Vec<String> {
    cx.try_global::<Spelling>()
        .map(|s| s.languages.clone())
        .unwrap_or_else(stored_languages)
}

/// The languages a dictionary was found for.
pub fn available(cx: &App) -> Vec<String> {
    cx.try_global::<Spelling>()
        .map(|s| s.available.clone())
        .filter(|a| !a.is_empty())
        .unwrap_or_else(scoria_spell::default_languages)
}

/// The languages chosen that no dictionary was found for.
pub fn missing(cx: &App) -> Vec<String> {
    cx.try_global::<Spelling>()
        .map(|s| s.missing.clone())
        .unwrap_or_default()
}

/// Sets the languages, writes them to `scoria.json` (dropping the key at
/// the default, as Obsidian's restore does), and reloads.
pub fn set_languages(languages: Vec<String>, cx: &mut App) {
    let stored = (languages != scoria_spell::default_languages()).then(|| languages.clone());
    if let Ok(Some(mut reg)) = scoria_vault::registry::VaultRegistry::open() {
        reg.set_spellcheck_languages(stored);
        let _ = reg.save();
    }
    if !cx.has_global::<Spelling>() {
        set_enabled(true, cx);
    }
    cx.global_mut::<Spelling>().languages = languages;
    load(cx);
}

/// Loads the dictionaries of the chosen languages and the custom words on
/// the background executor, then hands them to every editor.
fn load(cx: &mut App) {
    {
        let s = cx.global_mut::<Spelling>();
        if s.loading {
            s.reload = true;
            return;
        }
        s.loading = true;
        s.reload = false;
    }
    let languages = cx.global::<Spelling>().languages.clone();
    let task = cx.background_executor().spawn(async move {
        let config = config_dir();
        let (dicts, missing) = scoria_spell::load_languages(&languages, config.as_deref());
        let found =
            scoria_spell::discover::find(&scoria_spell::discover::search_dirs(config.as_deref()));
        let available = scoria_spell::discover::available(&found);
        let custom = custom_path()
            .map(|p| CustomDictionary::load(&p))
            .unwrap_or_default();
        (Speller::new(dicts, custom), missing, available)
    });
    cx.spawn(async move |cx| {
        let (speller, missing, available) = task.await;
        cx.update(|cx| {
            let again = {
                let s = cx.global_mut::<Spelling>();
                s.speller = Some(speller);
                s.missing = missing;
                s.available = available;
                s.loading = false;
                s.reload
            };
            if again {
                load(cx);
            }
        });
    })
    .detach();
}

/// The custom words, sorted.
pub fn custom_words(cx: &App) -> Vec<String> {
    match cx.try_global::<Spelling>().and_then(|s| s.speller.as_ref()) {
        Some(s) => s.custom().words().map(str::to_string).collect(),
        None => custom_path()
            .map(|p| CustomDictionary::load(&p))
            .unwrap_or_default()
            .words()
            .map(str::to_string)
            .collect(),
    }
}

/// Changes the custom words: re-reads the file (another window may have
/// written it), applies `change`, writes it back and rechecks.
fn edit_custom(change: impl FnOnce(&mut CustomDictionary) -> bool, cx: &mut App) {
    let Some(path) = custom_path() else {
        return;
    };
    let mut custom = CustomDictionary::load(&path);
    if !change(&mut custom) {
        return;
    }
    let _ = custom.save(&path);
    if let Some(s) = cx.try_global::<Spelling>().and_then(|s| s.speller.clone()) {
        cx.global_mut::<Spelling>().speller = Some(s.with_custom(custom));
    }
}

/// "Add to dictionary".
pub fn add_word(word: &str, cx: &mut App) {
    edit_custom(|c| c.add(word), cx);
}

/// The × beside a word in the "Spellcheck dictionary" modal.
pub fn remove_word(word: &str, cx: &mut App) {
    edit_custom(|c| c.remove(word), cx);
}

/// A language's name in the Settings row, as Obsidian lists it.
pub fn display_name(code: &str) -> String {
    LANGUAGE_NAMES
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(&code.replace('_', "-")))
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| code.to_string())
}

/// The 57 languages Obsidian 1.15.0's "+" dropdown offered on Linux, in
/// its order, with its names (Chromium's own list).
pub const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("af", "Afrikaans"),
    ("sq", "Albanian"),
    ("hy", "Armenian"),
    ("bg", "Bulgarian"),
    ("ca", "Catalan"),
    ("hr", "Croatian"),
    ("cs", "Czech"),
    ("da", "Danish"),
    ("nl", "Dutch"),
    ("en", "English"),
    ("en-AU", "English (Australia)"),
    ("en-CA", "English (Canada)"),
    ("en-GB", "English (United Kingdom)"),
    ("en-GB-oxendict", "English (United Kingdom)(oxendict)"),
    ("en-US", "English (United States)"),
    ("et", "Estonian"),
    ("fo", "Faroese"),
    ("fr", "French"),
    ("fr-FR", "French (France)"),
    ("gl", "Galician"),
    ("de", "German"),
    ("de-DE", "German (Germany)"),
    ("el", "Greek"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("it-IT", "Italian (Italy)"),
    ("ko", "Korean"),
    ("lv", "Latvian"),
    ("lt", "Lithuanian"),
    ("nb", "Norwegian Bokmål"),
    ("fa", "Persian"),
    ("pl", "Polish"),
    ("pt", "Portuguese"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sr", "Serbian"),
    ("sh", "Serbo-Croatian"),
    ("sk", "Slovak"),
    ("sl", "Slovenian"),
    ("es", "Spanish"),
    ("es-419", "Spanish (419)"),
    ("es-AR", "Spanish (Argentina)"),
    ("es-MX", "Spanish (Mexico)"),
    ("es-ES", "Spanish (Spain)"),
    ("es-US", "Spanish (United States)"),
    ("sv", "Swedish"),
    ("tg", "Tajik"),
    ("ta", "Tamil"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("vi", "Vietnamese"),
    ("cy", "Welsh"),
];

// ----- the editor's side ------------------------------------------------------

/// One note's misspellings, from the parse that drew its decorations.
#[derive(Debug, Clone, Default)]
pub struct NoteSpelling {
    /// Byte ranges of the whole text, in order.
    pub ranges: Arc<Vec<Range<usize>>>,
    /// The same, per line, as columns of that line.
    pub lines: Arc<Vec<Vec<Range<usize>>>>,
    /// How long the last check took, for the stress timings.
    pub check_ms: f64,
}

impl NoteSpelling {
    /// Checks a parsed note: the misspellings as whole-text ranges and per
    /// line. `typing` is the caret while a word is being typed there.
    pub fn check(
        speller: &Speller,
        doc: &scoria_markdown::Document<'_>,
        typing: Option<usize>,
    ) -> NoteSpelling {
        let started = std::time::Instant::now();
        let ranges = speller.misspellings(doc, typing);
        let lines = per_line(doc.source, &ranges);
        NoteSpelling {
            ranges: Arc::new(ranges),
            lines: Arc::new(lines),
            check_ms: started.elapsed().as_secs_f64() * 1000.0,
        }
    }

    /// The misspelled word at a byte of the text, if any.
    pub fn at(&self, offset: usize) -> Option<Range<usize>> {
        Speller::word_at(&self.ranges, offset)
    }
}

/// Whole-text ranges split per line, as columns.
fn per_line(src: &str, ranges: &[Range<usize>]) -> Vec<Vec<Range<usize>>> {
    let index = scoria_markdown::LineIndex::new(src);
    let mut out = vec![Vec::new(); index.line_count()];
    for r in ranges {
        let line = index.pos(r.start).line as usize;
        let start = index.line_start(line);
        // A word never spans a line break.
        if let Some(l) = out.get_mut(line) {
            l.push(r.start - start..r.end - start);
        }
    }
    out
}

/// What a right click on a misspelled word hands the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellHit {
    /// The word's bytes in the text when the menu opened.
    pub range: Range<usize>,
    pub word: String,
    /// Chromium's three at most; empty draws "No suggestions...".
    pub suggestions: Vec<String>,
}

/// The squiggle's colour: Chromium's spelling marker red, the same in the
/// light and dark themes.
pub fn squiggle_color() -> Hsla {
    gpui::rgb(0xff0000).into()
}

/// Paints the squiggles of one shaped line. `ranges` are source columns of
/// the line; only the parts shown as typed (a [`Piece::Copy`]) are marked,
/// so hidden markup and rendered widgets never carry one.
pub fn paint_line(
    ranges: &[Range<usize>],
    display: &DisplayLine,
    line: &WrappedLine,
    origin: Point<Pixels>,
    row_height: Pixels,
    window: &mut Window,
) {
    if ranges.is_empty() {
        return;
    }
    let layout = &line.unwrapped_layout;
    // The bottom of the font's box on a row: where the text sits in it,
    // centred as the editor centres it, plus the descent.
    let box_bottom =
        (row_height - layout.ascent - layout.descent) / 2. + layout.ascent + layout.descent;
    for r in ranges {
        for piece in &display.pieces {
            let Piece::Copy {
                display: d,
                source: s,
                len,
            } = piece
            else {
                continue;
            };
            let a = r.start.max(*s);
            let b = r.end.min(s + len);
            if a >= b {
                continue;
            }
            for (y, x0, x1) in segments(display, line, d + (a - s), d + (b - s), row_height) {
                paint_wave(
                    origin.x + x0,
                    origin.x + x1,
                    origin.y + y + box_bottom,
                    window,
                );
            }
        }
    }
}

/// The rows a display range covers: each row's top and the x span on it.
fn segments(
    display: &DisplayLine,
    line: &WrappedLine,
    from: usize,
    to: usize,
    row_height: Pixels,
) -> Vec<(Pixels, Pixels, Pixels)> {
    let text = &display.text;
    let mut out: Vec<(Pixels, Pixels, Pixels)> = Vec::new();
    let mut bounds: Vec<usize> = (from..=to.min(text.len()))
        .filter(|&i| text.is_char_boundary(i))
        .collect();
    bounds.dedup();
    let mut current: Option<(Pixels, Pixels, Pixels)> = None;
    for i in bounds {
        let Some(p) = line.position_for_index(i, row_height) else {
            continue;
        };
        match current.as_mut() {
            Some((y, _, x1)) if *y == p.y => *x1 = p.x,
            Some(_) => {
                // A wrap: gpui puts the boundary at the end of the row
                // before, so the next row starts at its left edge.
                out.extend(current.take());
                current = Some((p.y, px(0.), p.x));
            }
            None => current = Some((p.y, p.x, p.x)),
        }
    }
    out.extend(current);
    out.retain(|(_, a, b)| b > a);
    out
}

/// Chromium's wave between two x's with its lowest point on `bottom`: a
/// 1 px stroke, period 6 px, 2 px either side of its centre line,
/// starting on the centre line and going down.
fn paint_wave(x0: Pixels, x1: Pixels, bottom: Pixels, window: &mut Window) {
    const STROKE: f32 = 1.;
    const HALF_PERIOD: f32 = 3.;
    const AMPLITUDE: f32 = 2.;
    let centre = bottom - px(AMPLITUDE + STROKE / 2.);
    let mut builder = PathBuilder::stroke(px(STROKE));
    builder.move_to(point(x0, centre));
    let mut x = x0;
    let mut down = true;
    while x < x1 {
        let next = x + px(HALF_PERIOD);
        // A quadratic's middle reaches half way to its control point.
        let ctrl_y = if down {
            centre + px(2. * AMPLITUDE)
        } else {
            centre - px(2. * AMPLITUDE)
        };
        builder.curve_to(point(next, centre), point(x + px(HALF_PERIOD / 2.), ctrl_y));
        x = next;
        down = !down;
    }
    let Ok(path) = builder.build() else {
        return;
    };
    let mask = Bounds::from_corners(
        point(x0, centre - px(AMPLITUDE + STROKE)),
        point(x1, centre + px(AMPLITUDE + STROKE)),
    );
    window.with_content_mask(Some(gpui::ContentMask { bounds: mask }), |window| {
        window.paint_path(path, squiggle_color());
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_spell::Dictionary;

    fn speller() -> Speller {
        Speller::new(
            vec![Arc::new(Dictionary::bundled())],
            CustomDictionary::default(),
        )
    }

    #[test]
    fn misspellings_are_split_per_line_as_columns() {
        let src = "Hello wrold\n\n# A teh heading\n";
        let doc = scoria_markdown::parse(src);
        let n = NoteSpelling::check(&speller(), &doc, None);
        assert_eq!(*n.ranges, vec![6..11, 17..20]);
        assert_eq!(n.lines[0].first(), Some(&(6..11)));
        assert_eq!(n.lines[0].len(), 1);
        assert!(n.lines[1].is_empty());
        assert_eq!(n.lines[2], vec![4..7]);
        assert_eq!(n.at(8), Some(6..11));
        assert_eq!(n.at(12), None);
    }

    #[test]
    fn every_listed_language_has_a_name() {
        assert_eq!(LANGUAGE_NAMES.len(), 57);
        assert_eq!(display_name("en-US"), "English (United States)");
        assert_eq!(display_name("de"), "German");
        assert_eq!(display_name("xx-YY"), "xx-YY");
    }

    /// The editor rechecks on every keystroke, on the parse's background
    /// thread: a 20,000-word note must stay well inside a frame's worth of
    /// work once its words have been seen.
    #[test]
    fn a_long_note_rechecks_quickly() {
        let s = speller();
        let para = "The quick brown fox jumps over the lazy dog, and teh wrold keeps turning. ";
        let src: String = (0..1500).map(|i| format!("{para}{i}\n\n")).collect();
        let doc = scoria_markdown::parse(&src);
        let first = NoteSpelling::check(&s, &doc, None);
        assert_eq!(first.ranges.len(), 3000);
        let again = NoteSpelling::check(&s, &doc, None);
        assert!(
            again.check_ms < 250.,
            "a warm recheck of {} bytes took {:.1} ms",
            src.len(),
            again.check_ms
        );
    }
}
