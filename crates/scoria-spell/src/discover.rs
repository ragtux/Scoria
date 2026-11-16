//! Where dictionaries come from.
//!
//! Obsidian's checker is Chromium's, which downloads its own `.bdic`
//! dictionaries into the data folder. Scoria reads ordinary Hunspell
//! `.aff`/`.dic` pairs instead, from the places the desktop already keeps
//! them, plus a `dictionaries` folder under Scoria's configuration folder;
//! `en-US` is bundled, so a machine with none installed still checks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::dict::BUNDLED_LANGUAGE;

/// One dictionary found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The code Obsidian would write: `en-US`, from `en_US.dic`.
    pub language: String,
    pub aff: PathBuf,
    pub dic: PathBuf,
}

/// The folders searched, in order; the first pair found for a language wins.
/// `config_dir` is Scoria's configuration folder, whose `dictionaries`
/// folder comes first, so a person can add or override a language there.
pub fn search_dirs(config_dir: Option<&Path>) -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let home = env("HOME")
        .or_else(|| env("USERPROFILE"))
        .map(PathBuf::from);
    let mut dirs = Vec::new();
    if let Some(c) = config_dir {
        dirs.push(c.join("dictionaries"));
    }
    // Hunspell's own variable, a path list.
    if let Some(p) = env("DICPATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    if cfg!(target_os = "macos") {
        if let Some(h) = &home {
            dirs.push(h.join("Library/Spelling"));
        }
        dirs.push(PathBuf::from("/Library/Spelling"));
        dirs.push(PathBuf::from("/opt/homebrew/share/hunspell"));
        dirs.push(PathBuf::from("/usr/local/share/hunspell"));
    } else if cfg!(target_os = "windows") {
        // Windows 8 and later take Hunspell dictionaries in a folder per
        // language tag here; `subfolders` below looks one level down.
        if let Some(a) = env("APPDATA") {
            dirs.push(PathBuf::from(a).join("Microsoft").join("Spelling"));
        }
    } else {
        let data_home = env("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
        if let Some(d) = data_home {
            dirs.push(d.join("hunspell"));
        }
        // NixOS puts profiles' share folders here, Flatpak and Snap theirs.
        let data_dirs = env("XDG_DATA_DIRS")
            .map(|v| std::env::split_paths(&v).collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]);
        for d in data_dirs {
            dirs.push(d.join("hunspell"));
            dirs.push(d.join("myspell"));
        }
        if let Some(h) = &home {
            dirs.push(h.join(".nix-profile/share/hunspell"));
        }
        for d in [
            "/run/current-system/sw/share/hunspell",
            "/usr/share/hunspell",
            "/usr/share/myspell",
            "/usr/share/myspell/dicts",
            "/usr/local/share/hunspell",
        ] {
            dirs.push(PathBuf::from(d));
        }
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// Every `.aff`/`.dic` pair in `dirs` (and, on Windows, their language
/// subfolders), one per language, the first folder's winning.
pub fn find(dirs: &[PathBuf]) -> Vec<Found> {
    let mut out: BTreeMap<String, Found> = BTreeMap::new();
    for dir in dirs {
        let mut folders = vec![dir.clone()];
        if cfg!(target_os = "windows") {
            folders.extend(subfolders(dir));
        }
        for folder in folders {
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            let mut names: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            names.sort();
            for dic in names {
                if dic.extension().and_then(|e| e.to_str()) != Some("dic") {
                    continue;
                }
                let aff = dic.with_extension("aff");
                if !aff.is_file() {
                    continue;
                }
                let Some(stem) = dic.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                // Hyphenation and thesaurus files share the folder.
                if stem.starts_with("hyph_") || stem.starts_with("th_") {
                    continue;
                }
                let language = language_code(stem);
                out.entry(language.to_ascii_lowercase())
                    .or_insert(Found { language, aff, dic });
            }
        }
    }
    out.into_values().collect()
}

fn subfolders(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

/// `en_US` → `en-US`: a dictionary file's stem in Obsidian's spelling.
pub fn language_code(stem: &str) -> String {
    stem.replace('_', "-")
}

/// The languages a person can pick: the bundled one and every one found,
/// sorted, without repeats.
pub fn available(found: &[Found]) -> Vec<String> {
    let mut v: Vec<String> = found.iter().map(|f| f.language.clone()).collect();
    if !v.iter().any(|l| l.eq_ignore_ascii_case(BUNDLED_LANGUAGE)) {
        v.push(BUNDLED_LANGUAGE.to_string());
    }
    v.sort_by_key(|l| l.to_ascii_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// The dictionary for a language code: the exact code first (case and
/// `_`/`-` ignored), then, for a bare language (`de`), the first regional
/// one (`de-AT`, `de-DE`, ...). `None` for the bundled language when no
/// file overrides it, and for a language with no dictionary at all.
pub fn resolve<'a>(language: &str, found: &'a [Found]) -> Option<&'a Found> {
    let want = language_code(language).to_ascii_lowercase();
    if let Some(f) = found
        .iter()
        .find(|f| f.language.to_ascii_lowercase() == want)
    {
        return Some(f);
    }
    if !want.contains('-') {
        return found.iter().find(|f| {
            f.language
                .to_ascii_lowercase()
                .strip_prefix(&want)
                .is_some_and(|rest| rest.starts_with('-'))
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(dir: &Path, stem: &str) {
        std::fs::write(dir.join(format!("{stem}.aff")), "SET UTF-8\n").unwrap();
        std::fs::write(dir.join(format!("{stem}.dic")), "1\nword\n").unwrap();
    }

    #[test]
    fn pairs_are_found_and_named_as_obsidian_names_languages() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        pair(a.path(), "de_DE");
        pair(a.path(), "hyph_de_DE");
        pair(b.path(), "de_DE");
        pair(b.path(), "fr");
        std::fs::write(b.path().join("en_GB.dic"), "1\nx\n").unwrap();
        let found = find(&[a.path().to_path_buf(), b.path().to_path_buf()]);
        let langs: Vec<_> = found.iter().map(|f| f.language.as_str()).collect();
        assert_eq!(
            langs,
            ["de-DE", "fr"],
            "no hyphenation file, no .dic without .aff"
        );
        assert!(found[0].dic.starts_with(a.path()), "the first folder wins");
        assert_eq!(available(&found), ["de-DE", "en-US", "fr"]);
    }

    #[test]
    fn a_bare_language_resolves_to_a_regional_dictionary() {
        let a = tempfile::tempdir().unwrap();
        pair(a.path(), "de_AT");
        pair(a.path(), "de_DE");
        pair(a.path(), "pt_BR");
        let found = find(&[a.path().to_path_buf()]);
        assert_eq!(resolve("de", &found).unwrap().language, "de-AT");
        assert_eq!(resolve("de-DE", &found).unwrap().language, "de-DE");
        assert_eq!(resolve("pt_br", &found).unwrap().language, "pt-BR");
        assert!(resolve("pt-PT", &found).is_none());
        assert!(resolve("en-US", &found).is_none());
    }

    #[test]
    fn the_configuration_folder_comes_first() {
        let c = Path::new("/cfg");
        let dirs = search_dirs(Some(c));
        assert_eq!(dirs[0], c.join("dictionaries"));
    }
}
