//! Excluded files: the `userIgnoreFilters` list of `app.json`, as
//! Obsidian 1.15.1's `metadataCache.isUserIgnored` reads it (observed
//! 2026-11-16, `tools/obsidian-probe/excluded-files.mjs`).
//!
//! Each entry is trimmed first. One that then starts and ends with `/` and
//! has something between is a regular expression, searched for anywhere in
//! the vault path; anything else is a path prefix. Both compare without
//! case: `archive/` excludes `Archive/Old.md`, `/^Archive/` excludes
//! `archive lower/x.md`. A prefix is plain text, so `Archive` excludes
//! `Archived note.md` as well, `*.png` is a literal star, and `/Archive`
//! (no closing slash) is the prefix `/Archive`, which no vault path has. An
//! empty entry, `/`-wrapped nothing (`//`), and a regex that will not
//! compile exclude nothing; the other entries still apply. No flags are
//! read: `/arch/i` is a prefix too.
//!
//! The patterns are JavaScript's in Obsidian and the `regex` crate's here.
//! The two agree on everything a path filter is likely to say; lookaround
//! and backreferences, which the crate does not have, count as a regex that
//! will not compile.

use regex::{Regex, RegexBuilder};

/// A compiled `userIgnoreFilters` list.
#[derive(Debug, Clone, Default)]
pub struct ExcludeFilters {
    rules: Vec<Regex>,
}

impl ExcludeFilters {
    /// Compiles the list as it stands in `app.json`.
    pub fn new<S: AsRef<str>>(filters: &[S]) -> ExcludeFilters {
        ExcludeFilters {
            rules: filters.iter().filter_map(|f| compile(f.as_ref())).collect(),
        }
    }

    /// Whether nothing is excluded, which lets a caller skip the check.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Whether a vault path (a file or a folder, `/`-separated, no leading
    /// slash) is excluded.
    pub fn is_excluded(&self, path: &str) -> bool {
        self.rules.iter().any(|r| r.is_match(path))
    }
}

/// One entry's pattern: `None` for the entries that exclude nothing.
fn compile(filter: &str) -> Option<Regex> {
    let filter = filter.trim();
    if filter.is_empty() {
        return None;
    }
    let pattern = match filter
        .strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
    {
        // `//` is a prefix in Obsidian (its pattern is `^\/\/`), not an
        // empty regex that would match every path.
        Some(body) if !body.is_empty() => body.to_string(),
        _ => format!("^{}", regex::escape(filter)),
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .ok()
}

/// What an *Add exclusion* modal adds: the text as typed, untrimmed, unless
/// it is blank. Obsidian keeps `"  Padded/  "` with its spaces and adds a
/// rule already in the list a second time.
pub fn added_rule(typed: &str) -> Option<String> {
    (!typed.trim().is_empty()).then(|| typed.to_string())
}

/// What a picked folder adds: its path and a slash, so the rule is the
/// folder and not every name it begins (`Other/`, not `Other`).
pub fn folder_rule(folder: &str) -> String {
    format!("{}/", folder.trim_end_matches('/'))
}

/// A row of the *Add exclusion* field's suggestions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// A vault folder; picking it adds [`folder_rule`] of it.
    Folder {
        path: String,
        /// Byte ranges of `path` the typed text matched.
        ranges: Vec<std::ops::Range<usize>>,
    },
    /// The text as typed, drawn `+ text`; picking it adds [`added_rule`].
    Typed(String),
}

/// The suggestions under the *Add exclusion* field: the vault's folders
/// (never files) the typed text fuzzy matches, best first, then the text
/// itself as the last row once anything is typed. With nothing typed it is
/// every folder, shallowest first and then in code-point order, which put
/// `Archive`, `Data`, `Other` and `archive lower` ahead of `Archive/Deep`
/// in the probe (Obsidian 1.15.1, step s09).
pub fn offers(folders: &[String], typed: &str) -> Vec<Offer> {
    let depth = |p: &str| p.matches('/').count();
    let mut found: Vec<(f64, &String, Vec<std::ops::Range<usize>>)> = folders
        .iter()
        .filter(|f| !f.is_empty())
        .filter_map(|f| {
            if typed.trim().is_empty() {
                return typed.is_empty().then(|| (0.0, f, Vec::new()));
            }
            crate::fuzzy_match(typed, f).map(|m| (m.score, f, m.ranges))
        })
        .collect();
    found.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| depth(a.1).cmp(&depth(b.1)))
            .then_with(|| a.1.cmp(b.1))
    });
    let mut out: Vec<Offer> = found
        .into_iter()
        .map(|(_, path, ranges)| Offer::Folder {
            path: path.clone(),
            ranges,
        })
        .collect();
    if !typed.is_empty() {
        out.push(Offer::Typed(typed.to_string()));
    }
    out
}

impl Offer {
    /// The rule picking this row adds, if any: a blank `+` row adds nothing.
    pub fn rule(&self) -> Option<String> {
        match self {
            Offer::Folder { path, .. } => Some(folder_rule(path)),
            Offer::Typed(text) => added_rule(text),
        }
    }
}

/// The value the Files tab shows on the Excluded files row: nothing for no
/// rules, the rule itself for one, and the count otherwise.
pub fn summary(filters: &[String]) -> Option<String> {
    match filters {
        [] => None,
        [one] => Some(one.clone()),
        many => Some(format!("{} rules", many.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The paths of the probe's matcher step.
    const PATHS: &[&str] = &[
        "Archive/Old.md",
        "Archive/Deep/Deepest.md",
        "Archive",
        "Archive.md",
        "Archived note.md",
        "archive lower/lower.md",
        "archive/x.md",
        "Other/Archive/Nested.md",
        "Keep.md",
        "Old twin.md",
        "Archive/pic.png",
        "Data/Table.base",
        "a.png",
        "Notes/Sub/x.md",
        "Notes/Subway.md",
        "Notes/sub/x.md",
        "x.md",
        "Archive Old.md",
        "Drafts/2024/a.md",
        "Folder.with.dots/a.md",
        "Daily/2024-01-01.md",
        "Daily/notes.md",
        ".hidden/a.md",
    ];

    fn excluded(filters: &[&str]) -> Vec<&'static str> {
        let f = ExcludeFilters::new(filters);
        PATHS.iter().copied().filter(|p| f.is_excluded(p)).collect()
    }

    /// Every row of `results/2026-11-16-excluded-files.json` step s01: the
    /// filter list and the paths Obsidian 1.15.1 called ignored.
    #[test]
    fn matches_obsidians_table() {
        let all_archive = [
            "Archive/Old.md",
            "Archive/Deep/Deepest.md",
            "Archive",
            "Archive.md",
            "Archived note.md",
            "archive lower/lower.md",
            "archive/x.md",
            "Archive/pic.png",
            "Archive Old.md",
        ];
        let folder = [
            "Archive/Old.md",
            "Archive/Deep/Deepest.md",
            "archive/x.md",
            "Archive/pic.png",
        ];
        let anywhere = [
            "Archive/Old.md",
            "Archive/Deep/Deepest.md",
            "Archive",
            "Archive.md",
            "Archived note.md",
            "archive lower/lower.md",
            "archive/x.md",
            "Other/Archive/Nested.md",
            "Archive/pic.png",
            "Archive Old.md",
        ];
        let rows: &[(&[&str], &[&str])] = &[
            (&[], &[]),
            (&["Archive/"], &folder),
            (&["Archive"], &all_archive),
            (&["/Archive"], &[]),
            (&["archive/"], &folder),
            (&["Archive/Old.md"], &["Archive/Old.md"]),
            (&["Archive/Old"], &["Archive/Old.md"]),
            (&["/arch/"], &anywhere),
            (&["/Arch/"], &anywhere),
            (&["/arch/i"], &[]),
            (&["/^Archive/"], &all_archive),
            (&["/Old\\.md$/"], &["Archive/Old.md", "Archive Old.md"]),
            (&["/[/"], &[]),
            (&["*.png"], &[]),
            (&["/\\.png$/"], &["Archive/pic.png", "a.png"]),
            (
                &["Notes/Sub"],
                &["Notes/Sub/x.md", "Notes/Subway.md", "Notes/sub/x.md"],
            ),
            (&["Notes/Sub/"], &["Notes/Sub/x.md", "Notes/sub/x.md"]),
            (&[" Archive/"], &folder),
            (&[""], &[]),
            (&["/"], &[]),
            (&["/Daily\\/\\d{4}/"], &["Daily/2024-01-01.md"]),
            (
                &["Archive/", "Keep.md"],
                &[
                    "Archive/Old.md",
                    "Archive/Deep/Deepest.md",
                    "archive/x.md",
                    "Keep.md",
                    "Archive/pic.png",
                ],
            ),
            (&["Folder.with.dots"], &["Folder.with.dots/a.md"]),
            (&["Archive/ "], &folder),
            (&["ARCHIVE/"], &folder),
            (&["/^archive\\/old/"], &["Archive/Old.md"]),
            (&["\tArchive/"], &folder),
            (&["/ Old/"], &["Archive Old.md"]),
            (&["./Archive"], &[]),
            (&["Archive\\"], &[]),
            (&["//"], &[]),
            (&["/.*/"], PATHS),
        ];
        for (filters, want) in rows {
            assert_eq!(excluded(filters), *want, "filters {filters:?}");
        }
    }

    #[test]
    fn a_broken_regex_leaves_the_other_rules_working() {
        let f = ExcludeFilters::new(&["/[/", "Archive/"]);
        assert!(f.is_excluded("Archive/Old.md"));
        assert!(!f.is_excluded("Keep.md"));
        assert!(ExcludeFilters::new(&["/[/", "", "  "]).is_empty());
    }

    #[test]
    fn the_modal_adds_what_was_typed_and_a_folder_with_its_slash() {
        assert_eq!(added_rule("  Padded/  ").as_deref(), Some("  Padded/  "));
        assert_eq!(added_rule("   "), None);
        assert_eq!(added_rule(""), None);
        assert_eq!(added_rule("/\\.png$/").as_deref(), Some("/\\.png$/"));
        assert_eq!(folder_rule("Other"), "Other/");
        assert_eq!(folder_rule("Archive/Deep"), "Archive/Deep/");
    }

    fn shown(offers: &[Offer]) -> Vec<String> {
        offers
            .iter()
            .map(|o| match o {
                Offer::Folder { path, .. } => path.clone(),
                Offer::Typed(t) => format!("+ {t}"),
            })
            .collect()
    }

    /// The probe's folders and what Obsidian listed for each query.
    #[test]
    fn the_field_offers_folders_then_what_was_typed() {
        let folders: Vec<String> = [
            "Archive",
            "Archive/Deep",
            "Data",
            "Other",
            "Other/Archive",
            "archive lower",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            shown(&offers(&folders, "")),
            [
                "Archive",
                "Data",
                "Other",
                "archive lower",
                "Archive/Deep",
                "Other/Archive"
            ]
        );
        let a = shown(&offers(&folders, "A"));
        assert_eq!(a[0], "Archive");
        assert_eq!(a.last().unwrap(), "+ A");
        assert_eq!(a.len(), 6, "{a:?}");
        let o = shown(&offers(&folders, "o"));
        assert_eq!(o[0], "Other");
        assert_eq!(o.last().unwrap(), "+ o");
        assert_eq!(o.len(), 4, "{o:?}");
        assert_eq!(
            shown(&offers(&folders, "Archive/")),
            ["Archive/Deep", "+ Archive/"]
        );
        assert_eq!(
            shown(&offers(&folders, "Other/Ar")),
            ["Other/Archive", "+ Other/Ar"]
        );
        assert_eq!(shown(&offers(&folders, "Keep")), ["+ Keep"]);
        assert_eq!(shown(&offers(&folders, "/x/")), ["+ /x/"]);
        // A blank `+` row is offered and adds nothing.
        let blank = offers(&folders, "   ");
        assert_eq!(shown(&blank), ["+    "]);
        assert_eq!(blank[0].rule(), None);
        assert_eq!(offers(&folders, "Oth")[0].rule().as_deref(), Some("Other/"));
    }

    #[test]
    fn the_row_shows_one_rule_or_the_count() {
        assert_eq!(summary(&[]), None);
        assert_eq!(summary(&["Archive/".into()]).as_deref(), Some("Archive/"));
        assert_eq!(
            summary(&["Archive/".into(), "Data".into(), "Other/".into()]).as_deref(),
            Some("3 rules")
        );
    }
}
