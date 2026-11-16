//! What every host shares, included with `#[path]` so each host stays one
//! crate with no dependency but its runtime: the tempdir vault, the
//! vault-relative path rule, and the timing summaries.

#![allow(dead_code)]

use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

pub const NOTE: &str = "Notes/Spike.md";
pub const NOTE_TEXT: &str = "# Spike\n\nThe quick brown fox jumps over the lazy dog.\n";

/// A throwaway vault under the system temp dir with one note in it.
pub struct Vault {
    pub root: PathBuf,
}

impl Vault {
    pub fn new(tag: &str) -> Vault {
        let root =
            std::env::temp_dir().join(format!("scoria-plugin-spike-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Notes")).unwrap();
        std::fs::write(root.join(NOTE), NOTE_TEXT).unwrap();
        Vault { root }
    }

    /// The vault-relative path as a file under the root, refusing anything
    /// that could leave it (absolute paths, `..`, a drive prefix).
    pub fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let rel = Path::new(path);
        if rel.components().all(|c| matches!(c, Component::Normal(_))) {
            Ok(self.root.join(rel))
        } else {
            Err(format!("{path}: not a vault path"))
        }
    }

    pub fn read(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(self.resolve(path)?).map_err(|e| format!("{path}: {e}"))
    }

    pub fn modify(&self, path: &str, text: &str) -> Result<(), String> {
        let file = self.resolve(path)?;
        if !file.is_file() {
            return Err(format!("{path}: no such note"));
        }
        std::fs::write(file, text).map_err(|e| format!("{path}: {e}"))
    }

    /// Asserts the plugin's `append-line` reached the disk.
    pub fn check_appended(&self) {
        let text = self.read(NOTE).unwrap();
        assert_eq!(
            text,
            format!("{NOTE_TEXT}Appended by the spike plugin.\n"),
            "the note after append-line"
        );
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn report(key: &str, d: Duration) {
    println!("{key}_us={:.1}", d.as_secs_f64() * 1e6);
}

/// Times `f` `n` times one call at a time and prints the median, p99 and max.
pub fn sample(key: &str, n: usize, mut f: impl FnMut()) {
    for _ in 0..n.min(1000) {
        f();
    }
    let mut times: Vec<Duration> = (0..n)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    times.sort();
    let us = |d: Duration| d.as_secs_f64() * 1e6;
    println!(
        "{key}_median_us={:.3} {key}_p99_us={:.3} {key}_max_us={:.1} {key}_n={n}",
        us(times[n / 2]),
        us(times[n * 99 / 100]),
        us(times[n - 1]),
    );
}
