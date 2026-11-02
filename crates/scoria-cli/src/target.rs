//! What a path argument names: a vault, or a file in one.
//!
//! `scoria <folder>` has always meant "open this vault". A file argument is
//! what a file manager hands over — the desktop entry claims `text/markdown`
//! — and what shell completion produces, so `scoria Rocks/Basalt.md` opens
//! the vault that holds the note and then the note. The vault of a file is
//! the nearest folder above it with a `.obsidian` in it, which is
//! Obsidian's own definition of one; failing that, a vault on the machine's
//! list that contains the file; failing that, the file's own folder, since
//! any folder is a vault and that is the answer a person opening a loose
//! note wants.

use scoria_formats::config::DEFAULT_CONFIG_DIR;
use scoria_vault::VaultRegistry;
use std::path::{Path, PathBuf};

/// A resolved path argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The folder itself.
    Vault(PathBuf),
    /// A file, with the vault that holds it and its vault-relative path.
    File { vault: PathBuf, path: String },
}

/// Resolves one path argument, or says why it cannot be opened.
pub fn resolve(given: &Path) -> Result<Target, String> {
    resolve_with(given, &known_vaults())
}

/// [`resolve`], against a given list of the machine's vaults, so a test
/// never reads the developer's own.
pub fn resolve_with(given: &Path, known: &[PathBuf]) -> Result<Target, String> {
    let path = absolute(given);
    let meta = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", given.display()))?;
    if meta.is_dir() {
        return Ok(Target::Vault(path));
    }
    let vault = vault_of(&path, known)
        .ok_or_else(|| format!("{}: no folder above it to open as a vault", given.display()))?;
    let relative = path
        .strip_prefix(&vault)
        .map_err(|_| format!("{} is not inside {}", given.display(), vault.display()))?;
    Ok(Target::File {
        vault,
        path: slashed(relative),
    })
}

/// The vaults `scoria.json` remembers, or nothing when there is no list.
pub fn known_vaults() -> Vec<PathBuf> {
    match VaultRegistry::open() {
        Ok(Some(registry)) => registry
            .vaults()
            .iter()
            .map(|(_, entry)| entry.path.clone())
            .collect(),
        _ => Vec::new(),
    }
}

/// The vault a file belongs to.
fn vault_of(file: &Path, known: &[PathBuf]) -> Option<PathBuf> {
    let parents = || file.ancestors().skip(1);
    if let Some(root) = parents().find(|dir| dir.join(DEFAULT_CONFIG_DIR).is_dir()) {
        return Some(root.to_path_buf());
    }
    // A vault that nothing has configured yet has no `.obsidian` — Scoria
    // does not write one just to read a vault — so the machine's own list
    // is the second answer, deepest entry first so that a vault inside a
    // vault takes the file.
    if let Some(root) = parents().find(|dir| known.iter().any(|entry| same_folder(entry, dir))) {
        return Some(root.to_path_buf());
    }
    parents().next().map(Path::to_path_buf)
}

/// `path` as an absolute path, the way the vault list stores one: the
/// working directory joined on, with no symbolic link resolved, since a
/// vault reached through a link is that vault by the path the person used.
pub fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// A vault-relative path as Scoria spells one: `/` separators, original case.
fn slashed(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn same_folder(a: &Path, b: &Path) -> bool {
    if cfg!(target_os = "windows") {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_a_vault() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert_eq!(
            resolve_with(dir.path(), &[]).expect("resolves"),
            Target::Vault(dir.path().to_path_buf())
        );
    }

    #[test]
    fn a_file_is_opened_in_the_vault_that_holds_it() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let vault = dir.path().join("rocks");
        std::fs::create_dir_all(vault.join(".obsidian")).expect("the config dir");
        std::fs::create_dir_all(vault.join("Igneous")).expect("a folder");
        let note = vault.join("Igneous/Basalt.md");
        std::fs::write(&note, "# Basalt\n").expect("a note");

        assert_eq!(
            resolve_with(&note, &[]).expect("resolves"),
            Target::File {
                vault: vault.clone(),
                path: "Igneous/Basalt.md".into(),
            }
        );
        // A deeper folder with no `.obsidian` of its own does not become the
        // vault: the search goes up to the one that has it.
        let deep = vault.join("Igneous/Flows");
        std::fs::create_dir_all(&deep).expect("a folder");
        std::fs::write(deep.join("Pahoehoe.canvas"), "{}").expect("a canvas");
        assert_eq!(
            resolve_with(&deep.join("Pahoehoe.canvas"), &[]).expect("resolves"),
            Target::File {
                vault,
                path: "Igneous/Flows/Pahoehoe.canvas".into(),
            }
        );
    }

    #[test]
    fn a_remembered_vault_takes_a_file_with_no_config_folder() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let vault = dir.path().join("notes");
        std::fs::create_dir_all(vault.join("Sub")).expect("a folder");
        let note = vault.join("Sub/Alpha.md");
        std::fs::write(&note, "alpha\n").expect("a note");
        assert_eq!(
            resolve_with(&note, std::slice::from_ref(&vault)).expect("resolves"),
            Target::File {
                vault,
                path: "Sub/Alpha.md".into(),
            }
        );
    }

    #[test]
    fn a_loose_note_opens_its_own_folder() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let note = dir.path().join("Scratch.md");
        std::fs::write(&note, "scratch\n").expect("a note");
        assert_eq!(
            resolve_with(&note, &[]).expect("resolves"),
            Target::File {
                vault: dir.path().to_path_buf(),
                path: "Scratch.md".into(),
            }
        );
    }

    #[test]
    fn a_path_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let e = resolve_with(&dir.path().join("nowhere"), &[]).expect_err("refused");
        assert!(e.contains("nowhere"), "{e}");
    }
}
