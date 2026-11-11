//! `app:migrate-frontmatter`, "Format converter: Frontmatter migration", as
//! Obsidian 1.15.0 ran it for the 2026-11-09 probe
//! ([[Specification#Format converter: Frontmatter migration]]): the modal
//! ([`crate::migrate_modal`]), a run over every Markdown file of the vault
//! that writes only the notes [`scoria_formats::migrate`] changes, the
//! notice **Updated N files.** with **Undo**, and Undo putting back each
//! written file's bytes with **Reverted changes to N files.**
//!
//! The files are read from disk, not from the index, so a note's BOM and
//! line endings are what the converter sees, as Obsidian's did.

use super::*;
use crate::migrate_modal::{Counts, MigrateEvent, MigrateModal, Stage};
use scoria_formats::migrate::{migrate_frontmatter, Migration};
use std::sync::atomic::{AtomicBool, Ordering};

/// One file the run wrote: its path, its bytes before and its text after.
#[derive(Debug, Clone)]
pub(super) struct Migrated {
    pub path: String,
    pub before: Vec<u8>,
    pub after: String,
}

/// What a run did.
#[derive(Debug, Default)]
pub(super) struct MigrationRun {
    pub counts: Counts,
    pub written: Vec<Migrated>,
}

/// The modal while it is up, and a run's stop flag while one goes.
pub(super) struct MigrateState {
    view: Entity<MigrateModal>,
    _subscription: Subscription,
    stop: Option<Arc<AtomicBool>>,
    /// The last finished run's files, for Done's notice.
    finished: Vec<Migrated>,
}

impl VaultState {
    /// Converts every Markdown file under the vault, writing each one that
    /// changes, until `stop` is set. A file that cannot be read as UTF-8,
    /// whose block does not parse, or that cannot be written is counted as
    /// failed and left as it was.
    pub(super) fn migrate_frontmatter(&self, stop: &AtomicBool) -> MigrationRun {
        let paths: Vec<String> = self
            .vault()
            .markdown_files()
            .map(|f| f.path.to_string())
            .collect();
        let mut run = MigrationRun::default();
        for path in paths {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            run.counts.processed += 1;
            let Ok(absolute) = self.vault().write_path(&path) else {
                run.counts.failed += 1;
                continue;
            };
            let Ok(before) = std::fs::read(&absolute) else {
                run.counts.failed += 1;
                continue;
            };
            let Ok(text) = std::str::from_utf8(&before) else {
                run.counts.failed += 1;
                continue;
            };
            match migrate_frontmatter(text) {
                Migration::Unchanged => {}
                Migration::Failed => run.counts.failed += 1,
                Migration::Changed(after) => {
                    if scoria_vault::write_no_follow(&absolute, after.as_bytes()).is_err() {
                        run.counts.failed += 1;
                        continue;
                    }
                    run.counts.modified += 1;
                    run.written.push(Migrated {
                        path,
                        before,
                        after,
                    });
                }
            }
        }
        run
    }

    /// Undo: each file the run wrote gets its bytes back, unless it changed
    /// again since (another app's save, an edit), which keeps the newer
    /// bytes. Returns how many were reverted, and the index changes.
    pub(super) fn revert_migration(&self, files: &[Migrated]) -> (usize, Changes) {
        let mut reverted = Vec::new();
        for file in files {
            let Ok(absolute) = self.vault().write_path(&file.path) else {
                continue;
            };
            match std::fs::read(&absolute) {
                Ok(now) if now == file.after.as_bytes() => {}
                _ => continue,
            }
            if scoria_vault::write_no_follow(&absolute, &file.before).is_ok() {
                reverted.push(FileChange::Modified(file.path.clone()));
            }
        }
        let n = reverted.len();
        (n, self.absorb(reverted))
    }
}

/// The notice after Done.
pub(super) fn updated_notice(n: usize) -> String {
    format!("Updated {n} file{}.", plural(n))
}

/// The notice after Undo.
pub(super) fn reverted_notice(n: usize) -> String {
    format!("Reverted changes to {n} file{}.", plural(n))
}

impl Workspace {
    /// `app:migrate-frontmatter`: the modal, at its first page.
    pub(super) fn open_migrate_frontmatter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.is_none() {
            return;
        }
        let view = cx.new(MigrateModal::new);
        let subscription =
            cx.subscribe_in(&view, window, |this, _, event, window, cx| match event {
                MigrateEvent::Start => this.start_migration(window, cx),
                MigrateEvent::Stop => {
                    if let Some(stop) = this.migrate.as_ref().and_then(|m| m.stop.as_ref()) {
                        stop.store(true, Ordering::Relaxed);
                    }
                }
                MigrateEvent::Done => this.finish_migration(window, cx),
                MigrateEvent::Close => this.close_migrate(window, cx),
            });
        self.close_editor_suggest(cx);
        window.focus(&view.focus_handle(cx), cx);
        self.migrate = Some(MigrateState {
            view,
            _subscription: subscription,
            stop: None,
            finished: Vec::new(),
        });
        self.modal = Modal::MigrateFrontmatter;
        cx.notify();
    }

    /// Start conversion: the open note saved first, so the run reads what
    /// is on screen, then every file converted off the UI thread.
    fn start_migration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let Some(migrate) = self.migrate.as_mut() else {
            return;
        };
        if migrate.stop.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        migrate.stop = Some(stop.clone());
        migrate.view.update(cx, |m, cx| {
            m.set_stage(Stage::Running(Counts::default()), cx)
        });
        self.flush_save(cx);
        cx.spawn_in(window, async move |this, cx| {
            let run = cx
                .background_executor()
                .spawn(async move { state.migrate_frontmatter(&stop) })
                .await;
            this.update_in(cx, |ws, _window, cx| ws.migration_ran(run, cx))
                .ok();
        })
        .detach();
    }

    fn migration_ran(&mut self, run: MigrationRun, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let changes = state.absorb(
            run.written
                .iter()
                .map(|m| FileChange::Modified(m.path.clone()))
                .collect(),
        );
        self.after_changes(changes, cx);
        match self.migrate.as_mut() {
            Some(migrate) => {
                migrate.stop = None;
                // Go back and a second run start over: Done reports the
                // last run, as each Finished! page shows only its own.
                migrate.finished = run.written;
                migrate
                    .view
                    .update(cx, |m, cx| m.set_stage(Stage::Finished(run.counts), cx));
            }
            // The modal went while the run was going: the run stands, and
            // its notice goes up now.
            None => self.offer_migration_undo(run.written, cx),
        }
        cx.notify();
    }

    /// Done: the modal closes and the notice offers Undo.
    fn finish_migration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let finished = self
            .migrate
            .as_mut()
            .map(|m| std::mem::take(&mut m.finished))
            .unwrap_or_default();
        self.close_migrate(window, cx);
        self.offer_migration_undo(finished, cx);
    }

    fn offer_migration_undo(&mut self, written: Vec<Migrated>, cx: &mut Context<Self>) {
        if written.is_empty() {
            return;
        }
        let n = written.len();
        self.migration_undo = written;
        self.report_with(updated_notice(n), Some(NoticeAction::UndoMigration), cx);
    }

    fn close_migrate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A run in progress is stopped; what it wrote stays, and its
        // notice follows when it returns.
        if let Some(stop) = self.migrate.take().and_then(|m| m.stop) {
            stop.store(true, Ordering::Relaxed);
        }
        if self.modal == Modal::MigrateFrontmatter {
            self.close_modal(window, cx);
        }
        cx.notify();
    }

    /// The notice's Undo.
    pub(super) fn undo_migration(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let files = std::mem::take(&mut self.migration_undo);
        if files.is_empty() {
            return;
        }
        let (n, changes) = state.revert_migration(&files);
        self.after_changes(changes, cx);
        self.report(reverted_notice(n), cx);
    }

    /// The modal's element, for the overlay.
    pub(super) fn migrate_element(&self) -> Option<gpui::AnyElement> {
        self.migrate
            .as_ref()
            .map(|m| m.view.clone().into_any_element())
    }

    pub(super) fn describe_migrate(&self, cx: &App) -> Option<Value> {
        self.migrate.as_ref().map(|m| m.view.read(cx).describe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_frontmatter_is_runnable_and_obsidians() {
        assert!(RUNNABLE.contains(&"app:migrate-frontmatter"));
        assert!(scoria_commands::known_ids().contains(&"app:migrate-frontmatter"));
    }

    #[test]
    fn the_notices_count_files() {
        assert_eq!(updated_notice(18), "Updated 18 files.");
        assert_eq!(updated_notice(1), "Updated 1 file.");
        assert_eq!(reverted_notice(18), "Reverted changes to 18 files.");
    }

    /// A run over a vault writes only the notes that change, counts every
    /// Markdown file, leaves the canvas alone, and Undo puts every byte
    /// back.
    #[test]
    fn a_run_writes_only_what_changes_and_undo_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files: &[(&str, &str)] = &[
            ("a.md", "---\ntags: a, b\n---\nbody\n"),
            ("b.md", "---\ntags: [a, b]\n---\nbody\n"),
            ("Sub/c.md", "---\r\nalias: x\r\nk: v # c\r\n---\r\nbody\r\n"),
            ("d.md", "no frontmatter\n"),
            ("e.md", "---\ntags: a\ntags: b\n---\n"),
            ("board.canvas", "{}"),
        ];
        for (path, text) in files {
            let abs = root.join(path);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(abs, text).unwrap();
        }
        let state = VaultState::load(&root.to_path_buf()).unwrap();
        let run = state.migrate_frontmatter(&AtomicBool::new(false));
        assert_eq!(
            run.counts,
            Counts {
                processed: 5,
                modified: 2,
                failed: 1
            }
        );
        let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
        assert_eq!(read("a.md"), "---\ntags:\n  - a\n  - b\n---\nbody\n");
        assert_eq!(read("b.md"), "---\ntags: [a, b]\n---\nbody\n");
        assert_eq!(
            read("Sub/c.md"),
            "---\r\naliases:\n  - x\nk: v\n---\r\nbody\r\n"
        );
        assert_eq!(read("e.md"), "---\ntags: a\ntags: b\n---\n");
        assert_eq!(read("board.canvas"), "{}");

        // A file changed again since the run keeps its newer bytes.
        std::fs::write(root.join("a.md"), "edited\n").unwrap();
        let (n, _) = state.revert_migration(&run.written);
        assert_eq!(n, 1);
        assert_eq!(read("a.md"), "edited\n");
        assert_eq!(
            read("Sub/c.md"),
            "---\r\nalias: x\r\nk: v # c\r\n---\r\nbody\r\n"
        );
    }

    #[test]
    fn a_stopped_run_stops() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "---\ntags: a b\n---\n").unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let run = state.migrate_frontmatter(&AtomicBool::new(true));
        assert_eq!(run.counts, Counts::default());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.md")).unwrap(),
            "---\ntags: a b\n---\n"
        );
    }
}
