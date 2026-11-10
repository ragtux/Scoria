//! The Unique note creator's names: a moment format rendered from the
//! clock, moved forward a minute at a time until no note has it.
//!
//! Observed on Obsidian 1.15.0 on 2026-11-09 (`tools/obsidian-probe/results/
//! 2026-11-09-note-composer-unique.json`, runs `zk`, `zk:^s(00|04|...)` and
//! `zk:^s(00|01|02|04d)`). Three runs of *Create new unique note* at
//! 20:19:35, :39 and :43 made `202611092019.md`, `202611092020.md` and
//! `202611092021.md`: a taken name does not get a counter, the time moves on.
//! A name is taken when a note of that basename is anywhere in the vault --
//! with `Zettel/202611092021.md` there, a root note made at 20:21 was
//! `202611092022.md`. *Add unique internal link* picks its name the same way
//! but creates nothing, and with text selected the name it checks is the
//! whole link text, `<name> <selection>`: at 20:21:38, with 20:21 and 20:22
//! both taken and a bare link at that minute reading `202611092023`, a link
//! around `insert` was `[[202611092021 insert]]`.
//!
//! The clock is passed in, so every rule here is a test on a fixed minute.

use crate::Vault;
use scoria_formats::{config::ZK_PREFIXER_DEFAULT_FORMAT, validate_file_name, FileKind};
use scoria_templates::{moment, Stamp};
use std::collections::HashSet;

/// How many minutes forward a name is looked for before giving up: past a
/// leap year's worth, so a format that names only the year still finds the
/// next one. A format with no date token in it at all never changes, and
/// Obsidian's own loop would not end; this one does.
const MAX_MINUTES: i64 = 1 << 20;

/// Why no unique name could be picked or no note made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UniqueError {
    /// The format holds tokens Scoria does not render, so the name it
    /// would pick is not the name Obsidian would.
    Unsupported(Vec<String>),
    /// The rendered name is not a legal file name.
    BadName(String),
    /// The configured folder is not in the vault: Obsidian says
    /// "Failed to create unique note. Folder “X” not found." and makes
    /// nothing.
    NoFolder(String),
    /// Every minute in reach rendered a taken name.
    Exhausted,
}

impl std::fmt::Display for UniqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniqueError::Unsupported(tokens) => write!(
                f,
                "Scoria cannot render {} in a unique note format",
                tokens.join(", ")
            ),
            UniqueError::BadName(name) => write!(f, "A unique note cannot be named {name}"),
            UniqueError::NoFolder(folder) => write!(
                f,
                "Failed to create unique note. Folder \u{201c}{folder}\u{201d} not found."
            ),
            UniqueError::Exhausted => {
                write!(f, "Scoria found no free unique name for this format")
            }
        }
    }
}

/// The format a setting names notes by: the setting, or
/// [`ZK_PREFIXER_DEFAULT_FORMAT`] when it is absent or empty (an emptied
/// field went back to the default in the probe).
pub fn effective_format(setting: Option<&str>) -> &str {
    setting
        .filter(|f| !f.is_empty())
        .unwrap_or(ZK_PREFIXER_DEFAULT_FORMAT)
}

/// The first name `format` renders from `at` onwards, a minute at a time,
/// that `taken` refuses; with a `selection`, the name is `<rendered>
/// <selection>` and that whole text is what has to be free. The returned
/// name is the whole text.
pub fn pick_name(
    format: &str,
    at: &Stamp,
    selection: &str,
    taken: impl Fn(&str) -> bool,
) -> Result<String, UniqueError> {
    let unsupported = moment::unsupported_tokens(format);
    if !unsupported.is_empty() {
        return Err(UniqueError::Unsupported(unsupported));
    }
    for minute in 0..MAX_MINUTES {
        let mut name = moment::format(format, &at.plus_minutes(minute));
        if !selection.is_empty() {
            name.push(' ');
            name.push_str(selection);
        }
        if !taken(&name) {
            return Ok(name);
        }
    }
    Err(UniqueError::Exhausted)
}

impl Vault {
    /// The basenames of every note, lowered: what a unique name has to
    /// miss. Only Markdown files count, since a `[[name]]` with no
    /// extension resolves to nothing else; the case is folded the way link
    /// resolution folds it. **Unprobed**: both of those -- the probe's
    /// vaults held no canvas or differently-cased note of a candidate name.
    fn note_basenames(&self) -> HashSet<String> {
        self.files()
            .iter()
            .filter(|f| f.kind == FileKind::Markdown)
            .map(|f| {
                let name = f.path.rsplit('/').next().unwrap_or(&f.path);
                name.strip_suffix(".md").unwrap_or(name).to_lowercase()
            })
            .collect()
    }

    /// The name *Add unique internal link* writes between `[[` and `]]` at
    /// `at`, `selection` included.
    pub fn unique_link_name(
        &self,
        format: &str,
        at: &Stamp,
        selection: &str,
    ) -> Result<String, UniqueError> {
        let names = self.note_basenames();
        pick_name(format, at, selection, |n| names.contains(&n.to_lowercase()))
    }

    /// The vault path of the note *Create new unique note* makes at `at`:
    /// `<folder>/<name>.md`, the folder having to exist already, as for a
    /// daily note.
    pub fn unique_note_path(
        &self,
        format: &str,
        folder: &str,
        at: &Stamp,
    ) -> Result<String, UniqueError> {
        let folder = folder.trim_matches('/');
        if !folder.is_empty() && !self.absolute(folder).is_dir() {
            return Err(UniqueError::NoFolder(folder.to_string()));
        }
        let name = self.unique_link_name(format, at, "")?;
        if validate_file_name(&name).is_err() {
            return Err(UniqueError::BadName(name));
        }
        Ok(if folder.is_empty() {
            format!("{name}.md")
        } else {
            format!("{folder}/{name}.md")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn at(hour: u32, minute: u32, second: u32) -> Stamp {
        Stamp {
            year: 2026,
            month: 11,
            day: 9,
            hour,
            minute,
            second,
        }
    }

    fn scratch(folders: &[&str], files: &[&str]) -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".obsidian")).unwrap();
        for folder in folders {
            fs::create_dir_all(dir.path().join(folder)).unwrap();
        }
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let vault = Vault::open(dir.path()).unwrap();
        (dir, vault)
    }

    #[test]
    fn an_empty_or_absent_format_is_the_default() {
        assert_eq!(effective_format(None), "YYYYMMDDHHmm");
        assert_eq!(effective_format(Some("")), "YYYYMMDDHHmm");
        assert_eq!(
            effective_format(Some("YYYY-MM-DD-HHmmss")),
            "YYYY-MM-DD-HHmmss"
        );
    }

    /// The probe's three runs in one minute: each takes the next minute.
    #[test]
    fn a_taken_name_moves_the_time_forward_a_minute_until_it_is_free() {
        let (_dir, vault) = scratch(&[], &["Home.md"]);
        assert_eq!(
            vault.unique_note_path("YYYYMMDDHHmm", "", &at(20, 19, 35)),
            Ok("202611092019.md".to_string())
        );
        let (_dir, vault) = scratch(&[], &["Home.md", "202611092019.md"]);
        assert_eq!(
            vault.unique_note_path("YYYYMMDDHHmm", "", &at(20, 19, 39)),
            Ok("202611092020.md".to_string())
        );
        let (_dir, vault) = scratch(&[], &["202611092019.md", "202611092020.md"]);
        assert_eq!(
            vault.unique_note_path("YYYYMMDDHHmm", "", &at(20, 19, 43)),
            Ok("202611092021.md".to_string())
        );
    }

    /// `Zettel/202611092021.md` made a root note at 20:21 `...2022`.
    #[test]
    fn a_name_is_taken_by_a_note_of_that_basename_in_any_folder() {
        let (_dir, vault) = scratch(&[], &["Zettel/202611092021.md"]);
        assert_eq!(
            vault.unique_note_path("YYYYMMDDHHmm", "", &at(20, 21, 24)),
            Ok("202611092022.md".to_string())
        );
    }

    #[test]
    fn the_folder_setting_places_the_note_and_has_to_exist() {
        let (_dir, vault) = scratch(&["Zettel"], &["Home.md"]);
        assert_eq!(
            vault.unique_note_path("YYYYMMDDHHmm", "Zettel", &at(20, 21, 7)),
            Ok("Zettel/202611092021.md".to_string())
        );
        let err = vault
            .unique_note_path("YYYYMMDDHHmm", "NoSuchFolder", &at(20, 21, 12))
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Failed to create unique note. Folder \u{201c}NoSuchFolder\u{201d} not found."
        );
    }

    /// `YYYY-MM-DD [note] HHmmss`, run at 20:21:16 and again at 20:21:20.
    #[test]
    fn a_format_with_seconds_and_a_literal_names_the_note_by_it() {
        let (_dir, vault) = scratch(&[], &[]);
        assert_eq!(
            vault.unique_note_path("YYYY-MM-DD [note] HHmmss", "", &at(20, 21, 16)),
            Ok("2026-11-09 note 202116.md".to_string())
        );
    }

    /// 20:21 and 20:22 taken: a bare link is `...2023`, but a link around
    /// `insert` checks `202611092021 insert`, which is free.
    #[test]
    fn a_link_name_is_checked_with_the_selection_joined_to_it() {
        let (_dir, vault) = scratch(&[], &["Zettel/202611092021.md", "202611092022.md"]);
        assert_eq!(
            vault.unique_link_name("YYYYMMDDHHmm", &at(20, 21, 29), ""),
            Ok("202611092023".to_string())
        );
        assert_eq!(
            vault.unique_link_name("YYYYMMDDHHmm", &at(20, 21, 38), "insert"),
            Ok("202611092021 insert".to_string())
        );
    }

    #[test]
    fn the_name_carries_over_the_hour_and_a_format_that_never_changes_gives_up() {
        let taken = |n: &str| n == "202611092059";
        assert_eq!(
            pick_name("YYYYMMDDHHmm", &at(20, 59, 0), "", taken),
            Ok("202611092100".to_string())
        );
        assert_eq!(
            pick_name("[fixed]", &at(20, 59, 0), "", |_| true),
            Err(UniqueError::Exhausted)
        );
    }
}
