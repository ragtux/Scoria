//! File recovery: snapshots of the vault's notes kept outside it, and
//! `file-recovery:open` ("File recovery: Open local history") over them,
//! as Obsidian 1.15.0 did for the 2026-11-09 probe
//! ([[Specification#Observed File recovery, Obsidian 1.15.0 (installer build 1.13.7)]]).
//!
//! A snapshot is taken when a file is opened and when Scoria saves or sees
//! a file modified, each time only once `intervalMinutes` has passed since
//! the file's last one; snapshots older than `keepDays` are pruned when the
//! vault opens. The store is [`scoria_vault::recovery::SnapshotStore`],
//! under Scoria's configuration folder: the vault gains no file. A note and
//! a canvas are snapshotted (Obsidian took one of a canvas on opening,
//! whatever its description says); nothing else is.
//!
//! The modal is [`crate::recovery_modal`]; Restore writes the snapshot over
//! the file at once and says "Successfully restored version from <when>".

use super::*;
use crate::recovery_modal::{RecoveryEvent, RecoveryModal, Source};
use scoria_formats::config::{FILE_RECOVERY_DEFAULT_INTERVAL, FILE_RECOVERY_DEFAULT_KEEP_DAYS};
use scoria_vault::recovery::{time_ago, Snapshot, SnapshotStore};

const MINUTE_MS: f64 = 60_000.0;
const DAY_MS: f64 = 24.0 * 60.0 * MINUTE_MS;

/// The modal while it is up.
pub(super) struct RecoveryState {
    view: Entity<RecoveryModal>,
    _subscription: Subscription,
}

/// The store for the vault at `root`, pruned by `keepDays`. Tests keep
/// theirs in memory, so no run writes under the developer's configuration
/// folder.
pub(super) fn open_store(root: &Path, config: &FileRecoveryConfig) -> SnapshotStore {
    let mut store = if cfg!(test) {
        SnapshotStore::at(None)
    } else {
        SnapshotStore::for_vault(root)
    };
    let _ = store.prune(now_millis(), keep_ms(config));
    store
}

fn interval_ms(config: &FileRecoveryConfig) -> i64 {
    let minutes = config
        .interval_minutes
        .as_ref()
        .and_then(|n| n.as_f64())
        .filter(|m| *m >= 0.0)
        .unwrap_or(FILE_RECOVERY_DEFAULT_INTERVAL as f64);
    (minutes * MINUTE_MS) as i64
}

fn keep_ms(config: &FileRecoveryConfig) -> i64 {
    let days = config
        .keep_days
        .as_ref()
        .and_then(|n| n.as_f64())
        .filter(|d| *d >= 0.0)
        .unwrap_or(FILE_RECOVERY_DEFAULT_KEEP_DAYS as f64);
    (days * DAY_MS) as i64
}

/// The files a snapshot is taken of.
fn snapshotted(path: &str) -> bool {
    matches!(
        FileKind::from_path(path),
        FileKind::Markdown | FileKind::Canvas
    )
}

pub(super) fn restored_notice(ts: i64, now: i64) -> String {
    format!("Successfully restored version from {}", time_ago(ts, now))
}

impl VaultState {
    /// Takes a snapshot of `path` holding `text` now, if one is due and the
    /// File recovery plugin is on. A store that cannot be written is left
    /// silently: a snapshot is a convenience, never a reason to stop a save.
    pub(super) fn take_snapshot(&self, path: &str, text: &str) -> bool {
        if !snapshotted(path) || !self.core_plugin_enabled("file-recovery") {
            return false;
        }
        let interval = interval_ms(&self.file_recovery());
        self.recovery
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot(path, text, now_millis(), interval)
            .unwrap_or(false)
    }

    /// `path`'s snapshots, newest first.
    pub(super) fn snapshots_of(&self, path: &str) -> Vec<Snapshot> {
        self.recovery
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .of(path)
    }

    pub(super) fn snapshot_paths(&self) -> Vec<String> {
        self.recovery
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .paths()
    }

    /// Restore: the snapshot's text over its file, which is written at
    /// once, as Obsidian's was.
    pub(super) fn restore_snapshot(&self, snapshot: &Snapshot) -> Result<Changes, String> {
        let absolute = self
            .vault()
            .write_path(&snapshot.path)
            .map_err(|e| e.to_string())?;
        scoria_vault::write_no_follow(&absolute, snapshot.data.as_bytes())
            .map_err(|e| format!("Could not restore {}: {e}", snapshot.path))?;
        Ok(self.absorb(vec![FileChange::Modified(snapshot.path.clone())]))
    }
}

impl Workspace {
    /// A file shown in the main area: its snapshot on opening.
    pub(super) fn snapshot_on_open(&self, path: &str) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        if !snapshotted(path) {
            return;
        }
        let text = match state.texts().get(path) {
            Some(text) => Some(text.to_string()),
            None => std::fs::read_to_string(state.vault().absolute(path)).ok(),
        };
        if let Some(text) = text {
            state.take_snapshot(path, &text);
        }
    }

    /// Files modified on disk, by Scoria or anything else: a snapshot of
    /// each that is due.
    pub(super) fn snapshot_changes(&self, changes: &Changes) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        for change in &changes.files {
            if let FileChange::Modified(path) = change {
                if let Some(text) = state.texts().get(path).map(|t| t.to_string()) {
                    state.take_snapshot(path, &text);
                }
            }
        }
    }

    /// `file-recovery:open`: the modal, the active file chosen.
    pub(super) fn open_file_recovery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        // What is on screen counts: the open note's unsaved edits are saved
        // first, so the comparison is with the file as the person sees it.
        self.flush_save(cx);
        let query = self
            .current
            .as_deref()
            .map(|p| crate::recovery_modal::display_name(p).to_string())
            .unwrap_or_default();
        let snapshots_state = state.clone();
        let current_state = state.clone();
        let source = Source {
            paths: state.snapshot_paths(),
            snapshots: Arc::new(move |p| snapshots_state.snapshots_of(p)),
            current: Arc::new(move |p| {
                std::fs::read_to_string(current_state.vault().absolute(p)).ok()
            }),
        };
        let view = cx.new(|cx| RecoveryModal::new(source, &query, window, cx));
        let subscription =
            cx.subscribe_in(&view, window, |this, _, event, window, cx| match event {
                RecoveryEvent::Restore(snapshot) => {
                    let snapshot = snapshot.clone();
                    this.restore_snapshot(&snapshot, window, cx);
                }
                RecoveryEvent::Copy(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                }
                RecoveryEvent::Close => this.close_recovery(window, cx),
            });
        self.close_editor_suggest(cx);
        self.recovery = Some(RecoveryState {
            view,
            _subscription: subscription,
        });
        self.modal = Modal::FileRecovery;
        cx.notify();
    }

    fn restore_snapshot(
        &mut self,
        snapshot: &Snapshot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        match state.restore_snapshot(snapshot) {
            Ok(changes) => {
                self.after_changes(changes, cx);
                self.report(restored_notice(snapshot.ts, now_millis()), cx);
            }
            Err(err) => self.report(err, cx),
        }
    }

    fn close_recovery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.recovery = None;
        if self.modal == Modal::FileRecovery {
            self.close_modal(window, cx);
        }
        cx.notify();
    }

    pub(super) fn recovery_element(&self) -> Option<gpui::AnyElement> {
        self.recovery
            .as_ref()
            .map(|r| r.view.clone().into_any_element())
    }

    pub(super) fn describe_recovery(&self, cx: &App) -> Option<Value> {
        self.recovery.as_ref().map(|r| r.view.read(cx).describe(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_recovery_open_is_runnable_and_obsidians() {
        assert!(RUNNABLE.contains(&"file-recovery:open"));
        assert!(scoria_commands::known_ids().contains(&"file-recovery:open"));
    }

    #[test]
    fn the_settings_set_the_clock() {
        let config = FileRecoveryConfig::default();
        assert_eq!(interval_ms(&config), 5 * 60_000);
        assert_eq!(keep_ms(&config), 7 * 24 * 3_600_000);
        let config = FileRecoveryConfig {
            interval_minutes: config::js_number(2.5),
            keep_days: config::js_number(0.0),
            ..Default::default()
        };
        assert_eq!(interval_ms(&config), 150_000);
        assert_eq!(keep_ms(&config), 0);
    }

    #[test]
    fn the_restore_notice_says_when() {
        assert_eq!(
            restored_notice(0, 2_000),
            "Successfully restored version from a few seconds ago"
        );
    }

    /// Snapshots are of notes and canvases, taken once per interval, and a
    /// restore writes one back over the file; the vault gains no file.
    #[test]
    fn snapshots_and_a_restore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("Note.md"), "Version one.\n").unwrap();
        std::fs::write(root.join("pic.png"), [0u8; 4]).unwrap();
        let state = VaultState::load(&root.to_path_buf()).unwrap();
        assert!(state.take_snapshot("Note.md", "Version one.\n"));
        assert!(!state.take_snapshot("Note.md", "Version two.\n"));
        assert!(!state.take_snapshot("pic.png", "x"));
        assert!(state.take_snapshot("Board.canvas", "{}"));
        assert_eq!(state.snapshot_paths(), ["Board.canvas", "Note.md"]);
        std::fs::write(root.join("Note.md"), "Version three.\n").unwrap();
        let snap = state.snapshots_of("Note.md").remove(0);
        state.restore_snapshot(&snap).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("Note.md")).unwrap(),
            "Version one.\n"
        );
        let mut names: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["Note.md", "pic.png"]);
    }

    #[test]
    fn a_plugin_turned_off_takes_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".obsidian")).unwrap();
        std::fs::write(
            dir.path().join(".obsidian/core-plugins.json"),
            "{\"file-recovery\": false}",
        )
        .unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert!(!state.take_snapshot("Note.md", "x"));
    }
}
