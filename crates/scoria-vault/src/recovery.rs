//! File recovery's snapshots, kept by Scoria itself.
//!
//! Obsidian keeps them in its own IndexedDB (`<vault id>-backup`, store
//! `backups`, one `{path, ts, data}` record per snapshot), which nothing
//! outside Obsidian can read, so Scoria keeps its own: the same records, one
//! JSON object per line, in `file-recovery/<vault id>.jsonl` under Scoria's
//! configuration folder ([`crate::registry::config_dir`]) — **never in the
//! vault**, which must not gain a file Obsidian did not write. Obsidian never
//! sees Scoria's snapshots and Scoria never sees Obsidian's
//! ([[Specification#Observed File recovery, Obsidian 1.15.0 (installer build 1.13.7)]]).
//!
//! A snapshot is taken of a file's whole text when the last one of that file
//! is older than `intervalMinutes` (or there is none), and one identical to
//! the file's last snapshot is not taken again. Snapshots older than
//! `keepDays` are dropped by [`SnapshotStore::prune`].

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// One snapshot: the vault path, the time in Unix milliseconds and the
/// file's whole text, Obsidian's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub path: String,
    pub ts: i64,
    pub data: String,
}

/// The snapshots of one vault.
#[derive(Debug, Default)]
pub struct SnapshotStore {
    /// The `.jsonl` file, or `None` for a store kept in memory only (no
    /// configuration folder, or a test).
    file: Option<PathBuf>,
    /// Every snapshot, for a store in memory only.
    memory: Vec<Snapshot>,
    /// Each path's last snapshot: its time and a hash of its text. Read
    /// from the file the first time it is needed.
    last: Option<HashMap<String, (i64, u64)>>,
}

impl SnapshotStore {
    /// The store for the vault at `root`, under Scoria's configuration
    /// folder; in memory when there is none.
    pub fn for_vault(root: &Path) -> SnapshotStore {
        SnapshotStore::in_config(crate::registry::config_dir(), root)
    }

    /// The store for the vault at `root` under the configuration folder
    /// `config`.
    pub fn in_config(config: Option<PathBuf>, root: &Path) -> SnapshotStore {
        SnapshotStore::at(config.map(|dir| {
            dir.join("file-recovery")
                .join(format!("{}.jsonl", vault_id(root)))
        }))
    }

    /// A store in `file`, or in memory with `None`.
    pub fn at(file: Option<PathBuf>) -> SnapshotStore {
        SnapshotStore {
            file,
            memory: Vec::new(),
            last: None,
        }
    }

    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// Every snapshot, oldest first. A line that does not parse is skipped.
    pub fn all(&self) -> Vec<Snapshot> {
        let Some(file) = &self.file else {
            return self.memory.clone();
        };
        let Ok(f) = std::fs::File::open(file) else {
            return Vec::new();
        };
        std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect()
    }

    /// `path`'s snapshots, newest first.
    pub fn of(&self, path: &str) -> Vec<Snapshot> {
        let mut out: Vec<Snapshot> = self.all().into_iter().filter(|s| s.path == path).collect();
        out.sort_by_key(|s| std::cmp::Reverse(s.ts));
        out
    }

    /// The paths that have snapshots, sorted.
    pub fn paths(&self) -> Vec<String> {
        let mut out: Vec<String> = self.all().into_iter().map(|s| s.path).collect();
        out.sort();
        out.dedup();
        out
    }

    fn last(&mut self) -> &mut HashMap<String, (i64, u64)> {
        if self.last.is_none() {
            let mut map = HashMap::new();
            for s in self.all() {
                let entry = map.entry(s.path.clone()).or_insert((i64::MIN, 0));
                if s.ts >= entry.0 {
                    *entry = (s.ts, hash(&s.data));
                }
            }
            self.last = Some(map);
        }
        self.last.as_mut().expect("just filled")
    }

    /// Takes a snapshot of `path` holding `text` at `now` (Unix ms) when it
    /// is due: none yet, or the last one at least `interval_ms` old and of
    /// a different text. Returns whether one was taken.
    pub fn snapshot(
        &mut self,
        path: &str,
        text: &str,
        now: i64,
        interval_ms: i64,
    ) -> std::io::Result<bool> {
        let h = hash(text);
        if let Some(&(ts, last_hash)) = self.last().get(path) {
            if last_hash == h || now - ts < interval_ms {
                return Ok(false);
            }
        }
        let record = Snapshot {
            path: path.to_string(),
            ts: now,
            data: text.to_string(),
        };
        match &self.file {
            Some(file) => {
                if let Some(dir) = file.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file)?;
                let mut line = serde_json::to_string(&record).map_err(std::io::Error::other)?;
                line.push('\n');
                f.write_all(line.as_bytes())?;
            }
            None => self.memory.push(record),
        }
        self.last().insert(path.to_string(), (now, h));
        Ok(true)
    }

    /// Drops every snapshot older than `keep_ms` before `now`. The file is
    /// rewritten only when something goes.
    pub fn prune(&mut self, now: i64, keep_ms: i64) -> std::io::Result<usize> {
        let all = self.all();
        let kept: Vec<Snapshot> = all
            .iter()
            .filter(|s| now - s.ts <= keep_ms)
            .cloned()
            .collect();
        let dropped = all.len() - kept.len();
        if dropped > 0 {
            self.replace(kept)?;
        }
        Ok(dropped)
    }

    /// "Clear history": every snapshot gone.
    pub fn clear(&mut self) -> std::io::Result<()> {
        self.replace(Vec::new())
    }

    fn replace(&mut self, records: Vec<Snapshot>) -> std::io::Result<()> {
        self.last = None;
        let Some(file) = &self.file else {
            self.memory = records;
            return Ok(());
        };
        if records.is_empty() {
            return match std::fs::remove_file(file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let mut text = String::new();
        for r in &records {
            text.push_str(&serde_json::to_string(r).map_err(std::io::Error::other)?);
            text.push('\n');
        }
        let tmp = file.with_extension("jsonl.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, file)
    }
}

/// Sixteen hex digits for a vault folder, as the fold records name theirs.
pub fn vault_id(root: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// One line of a snapshot compared with the file now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Same(String),
    /// In the snapshot, not in the file.
    Removed(String),
    /// In the file, not in the snapshot.
    Added(String),
}

/// The lines of `old` against `new`, by their longest common subsequence;
/// "Show changes". Past 4,000 lines a side the two are shown whole, one
/// removed and one added, rather than taking quadratic time.
pub fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len() > 4000 || b.len() > 4000 {
        let mut out: Vec<DiffLine> = a.iter().map(|l| DiffLine::Removed(l.to_string())).collect();
        out.extend(b.iter().map(|l| DiffLine::Added(l.to_string())));
        return out;
    }
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffLine::Same(a[i].to_string()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(DiffLine::Removed(a[i].to_string()));
            i += 1;
        } else {
            out.push(DiffLine::Added(b[j].to_string()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| DiffLine::Removed(l.to_string())));
    out.extend(b[j..].iter().map(|l| DiffLine::Added(l.to_string())));
    out
}

/// How long ago `then` was at `now` (both Unix ms), in the words of
/// moment.js's `fromNow` as its documentation tables them: "a few seconds
/// ago", "a minute ago", "5 minutes ago", "an hour ago", "a day ago", …
/// The restore notice ends with it.
pub fn time_ago(then: i64, now: i64) -> String {
    let s = ((now - then).max(0) as f64) / 1000.0;
    let m = s / 60.0;
    let h = m / 60.0;
    let d = h / 24.0;
    let round = |x: f64| x.round() as i64;
    if s < 45.0 {
        "a few seconds ago".into()
    } else if s < 90.0 {
        "a minute ago".into()
    } else if m < 45.0 {
        format!("{} minutes ago", round(m))
    } else if m < 90.0 {
        "an hour ago".into()
    } else if h < 22.0 {
        format!("{} hours ago", round(h))
    } else if h < 36.0 {
        "a day ago".into()
    } else if d < 26.0 {
        format!("{} days ago", round(d))
    } else if d < 45.0 {
        "a month ago".into()
    } else if d < 320.0 {
        format!("{} months ago", round(d / 30.4))
    } else if d < 548.0 {
        "a year ago".into()
    } else {
        format!("{} years ago", round(d / 365.25))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn a_snapshot_is_taken_once_per_interval_and_not_twice_the_same() {
        let mut store = SnapshotStore::at(None);
        assert!(store.snapshot("Note.md", "one", 0, 5 * MIN).unwrap());
        // Within the interval: none, as Obsidian took none for two
        // modifies and typing within five minutes.
        assert!(!store.snapshot("Note.md", "two", 4 * MIN, 5 * MIN).unwrap());
        // Another file is its own clock.
        assert!(store.snapshot("Other.md", "x", 4 * MIN, 5 * MIN).unwrap());
        assert!(store.snapshot("Note.md", "two", 5 * MIN, 5 * MIN).unwrap());
        // The same text again is not a new snapshot.
        assert!(!store.snapshot("Note.md", "two", 20 * MIN, 5 * MIN).unwrap());
        let of: Vec<_> = store.of("Note.md").into_iter().map(|s| s.data).collect();
        assert_eq!(of, ["two", "one"]);
        assert_eq!(store.paths(), ["Note.md", "Other.md"]);
    }

    #[test]
    fn the_file_holds_obsidians_records_and_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("deep/v.jsonl");
        let mut store = SnapshotStore::at(Some(file.clone()));
        store.snapshot("A.md", "a\n", 1000, 0).unwrap();
        store.snapshot("B.md", "b \"q\"", 2000, 0).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "{\"path\":\"A.md\",\"ts\":1000,\"data\":\"a\\n\"}\n{\"path\":\"B.md\",\"ts\":2000,\"data\":\"b \\\"q\\\"\"}\n"
        );
        let mut again = SnapshotStore::at(Some(file.clone()));
        // The reopened store knows A.md's last text.
        assert!(!again.snapshot("A.md", "a\n", 9000, 0).unwrap());
        assert_eq!(again.of("B.md")[0].data, "b \"q\"");
    }

    #[test]
    fn prune_drops_what_is_older_than_keep_days_and_clear_drops_all() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("v.jsonl");
        let mut store = SnapshotStore::at(Some(file.clone()));
        let day = 24 * 60 * MIN;
        store.snapshot("A.md", "old", 0, 0).unwrap();
        store.snapshot("A.md", "new", 6 * day, 0).unwrap();
        assert_eq!(store.prune(8 * day, 7 * day).unwrap(), 1);
        assert_eq!(store.all().len(), 1);
        assert_eq!(store.prune(8 * day, 7 * day).unwrap(), 0);
        store.clear().unwrap();
        assert!(!file.exists());
        assert!(store.all().is_empty());
        // A cleared store snapshots afresh.
        assert!(store.snapshot("A.md", "new", 9 * day, 7 * day).unwrap());
    }

    #[test]
    fn the_store_is_outside_the_vault() {
        let vault = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let store = SnapshotStore::in_config(Some(config.path().to_path_buf()), vault.path());
        let file = store.file().unwrap();
        assert!(file.starts_with(config.path().join("file-recovery")));
        assert!(!file.starts_with(vault.path()));
    }

    #[test]
    fn diff_lines_marks_what_changed() {
        assert_eq!(
            diff_lines("a\nb\nc\n", "a\nc\nd\n"),
            vec![
                DiffLine::Same("a".into()),
                DiffLine::Removed("b".into()),
                DiffLine::Same("c".into()),
                DiffLine::Added("d".into()),
            ]
        );
    }

    #[test]
    fn time_ago_reads_like_moment() {
        assert_eq!(time_ago(0, 3_000), "a few seconds ago");
        assert_eq!(time_ago(0, 60_000), "a minute ago");
        assert_eq!(time_ago(0, 10 * MIN), "10 minutes ago");
        assert_eq!(time_ago(0, 60 * MIN), "an hour ago");
        assert_eq!(time_ago(0, 5 * 60 * MIN), "5 hours ago");
        assert_eq!(time_ago(0, 24 * 60 * MIN), "a day ago");
        assert_eq!(time_ago(0, 3 * 24 * 60 * MIN), "3 days ago");
    }
}
