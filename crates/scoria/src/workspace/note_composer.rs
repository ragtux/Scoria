//! The Note composer core plugin's three commands, end to end:
//! `note-composer:merge-file`, `note-composer:split-file` ("Extract current
//! selection...") and `note-composer:extract-heading` ("Extract this
//! heading...").
//!
//! The bytes come from `scoria-composer`, which holds every rule the
//! 2026-11-09 probe settled; this is the part with a vault and a window:
//! the prompt ([`super::composer_prompt`]), the confirmation and
//! `askBeforeMerging`, the "Update links" question and `alwaysUpdateLinks`
//! (the rename's own: the links to the source are planned by the rename
//! planner as a rename of the source to the target), the files written
//! with the rename's rule that a note changed on disk since it was indexed
//! keeps its own bytes, and the source removed through the delete path, so
//! `trashOption` decides where it goes as it does for `app:delete-file`.
//!
//! An extraction edits the open note through its editor -- one undo step,
//! saved the way typing is -- and writes the note it extracts into on disk.

use super::composer_prompt::{Candidate, Choice, ComposerEvent, ComposerPrompt, Kind, LinksAnswer};
use super::*;
use scoria_composer::{Placement, Replacement};

/// The open composer modal and what it is in the middle of.
pub(super) struct ComposerModal {
    pub(super) view: Entity<ComposerPrompt>,
    _subscription: Subscription,
    op: Op,
}

#[derive(Debug, Clone)]
enum Op {
    Merge {
        source: String,
        /// Set once a note is chosen: the target, whether the merge creates
        /// it (Ctrl+Enter), and Enter or Shift+Enter.
        target: Option<(String, bool, Placement)>,
    },
    Extract {
        source: String,
        /// The editor's text when the prompt opened, and the range taken;
        /// the edit is refused if the buffer moved under the prompt.
        text: String,
        range: Range<usize>,
    },
}

/// What a merge did on disk.
#[derive(Debug, Default)]
pub(super) struct MergeOutcome {
    pub changes: Changes,
    /// The links rewritten and the files they were in, the source's own
    /// among them -- the numbers Obsidian's Notice gives.
    pub links: usize,
    pub files: usize,
    /// Notes whose links were left because they changed on disk since the
    /// index read them.
    pub stale: Vec<String>,
}

/// What an extraction wrote to the note it extracted into, and what the
/// source's editor is to do.
#[derive(Debug)]
pub(super) struct ExtractOutcome {
    pub changes: Changes,
    pub plan: scoria_composer::ExtractPlan,
}

impl VaultState {
    /// The composer's template, when `template` names one that can be read:
    /// the path as written (it is stored with `.md`), or with `.md` added.
    fn composer_template(&self) -> Option<String> {
        let path = self.note_composer().template.clone().unwrap_or_default();
        let path = path.trim().trim_matches('/');
        if path.is_empty() {
            return None;
        }
        let vault = self.vault();
        let path = if vault.exists(path) {
            path.to_string()
        } else {
            format!("{path}.md")
        };
        let raw = std::fs::read_to_string(vault.absolute(&path)).ok()?;
        Some(scoria_text::FileText::parse(&raw).text)
    }

    /// The index brought up to what is on disk for `paths`: a note saved a
    /// moment ago has not been through the watcher yet, and a plan drawn
    /// from stale text would write the wrong bytes.
    fn reindex(&self, paths: &[&str]) -> Changes {
        self.absorb(
            paths
                .iter()
                .filter(|p| self.vault().exists(p))
                .map(|p| FileChange::Modified(p.to_string()))
                .collect(),
        )
    }

    /// The "Update links" question's numbers for merging `source` into
    /// `target`: `(files, links)`, the source's own links counted.
    pub(super) fn merge_link_counts(&self, source: &str, target: &str) -> (usize, usize) {
        self.reindex(&[source, target]);
        let texts = self.texts();
        let cache = self.cache();
        let rewrites =
            scoria_composer::link_rewrites(&cache, source, target, self.link_format(), &|p| {
                texts.get(p).map(|t| t.to_string())
            });
        scoria_composer::counts(&rewrites)
    }

    /// Writes the merge of `source` into `target` (created when `create`):
    /// the target's new bytes and, with `update_links`, every other note's
    /// links to the source pointed at the target. The source itself is left
    /// for the delete path.
    pub(super) fn merge_files(
        &self,
        source: &str,
        target: &str,
        create: bool,
        place: Placement,
        update_links: bool,
        at: scoria_templates::Stamp,
    ) -> Result<MergeOutcome, String> {
        let mut changes = self.reindex(&[source, target]);
        if create && self.vault().exists(target) {
            return Err(format!("{target} already exists"));
        }
        let template = self.composer_template();
        let (plan, rewrites, target_was) = {
            let texts = self.texts();
            let cache = self.cache();
            let read = |p: &str| texts.get(p).map(|t| t.to_string());
            let source_text = read(source).ok_or_else(|| format!("{source} is not a note"))?;
            let target_text = if create {
                String::new()
            } else {
                read(target).ok_or_else(|| format!("{target} is not a note"))?
            };
            let rewrites = if update_links {
                scoria_composer::link_rewrites(&cache, source, target, self.link_format(), &read)
            } else {
                Vec::new()
            };
            let plan = scoria_composer::plan_merge(
                source,
                &source_text,
                target,
                &target_text,
                place,
                template.as_deref().map(|t| (t, at)),
                &rewrites,
            );
            (plan, rewrites, target_text)
        };
        let mut written = Vec::new();
        if create {
            changes
                .files
                .extend(self.create_note(target, &plan.target)?.files);
        } else {
            // The target is the one note the merge cannot leave alone: if
            // it changed on disk since it was read, nothing is written.
            let absolute = self.vault().write_path(target).map_err(|e| e.to_string())?;
            match std::fs::read_to_string(&absolute) {
                Ok(on_disk) if on_disk == target_was => {}
                _ => return Err(format!("{target} changed on disk; nothing was merged")),
            }
            scoria_vault::write_no_follow(&absolute, plan.target.as_bytes())
                .map_err(|e| format!("Could not write {target}: {e}"))?;
            written.push(target.to_string());
        }
        let mut stale = Vec::new();
        {
            let texts = self.texts();
            for rw in &plan.others {
                let absolute = self
                    .vault()
                    .write_path(&rw.new_path)
                    .map_err(|e| e.to_string())?;
                let was = texts.get(&rw.old_path).map(|t| t.to_string());
                match (std::fs::read_to_string(&absolute), was) {
                    (Ok(on_disk), Some(was)) if on_disk == was => {}
                    _ => {
                        stale.push(rw.new_path.clone());
                        continue;
                    }
                }
                scoria_vault::write_no_follow(&absolute, rw.text.as_bytes())
                    .map_err(|e| format!("Could not update links in {}: {e}", rw.new_path))?;
                written.push(rw.new_path.clone());
            }
        }
        let indexed = self.absorb(
            written
                .iter()
                .map(|p| FileChange::Modified(p.clone()))
                .collect(),
        );
        changes.files.extend(indexed.files);
        changes.notes.extend(indexed.notes);
        let kept: Vec<&scoria_cache::Rewrite> = rewrites
            .iter()
            .filter(|r| !stale.contains(&r.new_path))
            .collect();
        Ok(MergeOutcome {
            changes,
            links: kept.iter().map(|r| r.links).sum(),
            files: kept.len(),
            stale,
        })
    }

    /// Writes `range` of `source_text` (the open note's buffer) into
    /// `target`, created when `target_was` is `None`, and plans the link
    /// left in its place. The source is not written: its editor makes the
    /// replacement.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn extract_into(
        &self,
        source: &str,
        source_text: &str,
        range: Range<usize>,
        target: &str,
        create: bool,
        place: Placement,
        at: scoria_templates::Stamp,
    ) -> Result<ExtractOutcome, String> {
        let mut changes = if create {
            Changes::default()
        } else {
            self.reindex(&[target])
        };
        let template = self.composer_template();
        let opts = scoria_composer::ExtractOptions {
            replacement: Replacement::from_config(self.note_composer().replacement_text.as_deref()),
            link: scoria_suggest::Options {
                format: self.link_format(),
                markdown_links: self.app().use_markdown_links.unwrap_or(false),
            },
            template: template.as_deref().map(|t| (t, at)),
        };
        let target_was = if create {
            None
        } else {
            Some(
                self.texts()
                    .get(target)
                    .map(|t| t.to_string())
                    .ok_or_else(|| format!("{target} is not a note"))?,
            )
        };
        let plan = {
            // The link is the shortest that reaches the note once it
            // exists, so a new note is known to the resolver first.
            let mut resolver = self.cache().resolver().clone();
            resolver.insert(target);
            scoria_composer::plan_extract(
                source,
                source_text,
                range,
                target,
                target_was.as_deref(),
                place,
                &resolver,
                &opts,
            )
        };
        if create {
            changes
                .files
                .extend(self.create_note(target, &plan.target_text)?.files);
        } else {
            let absolute = self.vault().write_path(target).map_err(|e| e.to_string())?;
            match std::fs::read_to_string(&absolute) {
                Ok(on_disk) if Some(on_disk.as_str()) == target_was.as_deref() => {}
                _ => return Err(format!("{target} changed on disk; nothing was extracted")),
            }
            scoria_vault::write_no_follow(&absolute, plan.target_text.as_bytes())
                .map_err(|e| format!("Could not write {target}: {e}"))?;
            let indexed = self.absorb(vec![FileChange::Modified(target.to_string())]);
            changes.files.extend(indexed.files);
            changes.notes.extend(indexed.notes);
        }
        Ok(ExtractOutcome { changes, plan })
    }

    /// Where a note an extraction creates under `name` goes: the
    /// `newFileLocation` folder (the root when `folder` names one that does
    /// not exist), numbered when the name is taken there.
    pub(super) fn composer_new_path(&self, name: &str, source: &str) -> String {
        let path = self.new_note_path(name, Some(source));
        let (folder, file) = path.rsplit_once('/').unwrap_or(("", path.as_str()));
        let stem = file.strip_suffix(".md").unwrap_or(file);
        self.vault().available_path(folder, stem, "md")
    }
}

impl Workspace {
    /// The palette's checks: merge-file wants a note on screen in either
    /// view, split-file a selection in the editor, extract-heading the bare
    /// caret on a heading line. Not one of them is listed over a canvas or
    /// an empty tab (2026-11-09, s20).
    pub(super) fn composer_can_run(&self, id: &str, cx: &App) -> Option<bool> {
        let markdown = self
            .current
            .as_deref()
            .is_some_and(|p| FileKind::from_path(p) == FileKind::Markdown);
        Some(match id {
            "note-composer:merge-file" => {
                markdown && matches!(self.main_view(cx), MainView::Reading | MainView::Editor)
            }
            "note-composer:split-file" => {
                markdown
                    && self.main_view(cx) == MainView::Editor
                    && !self.editor.read(cx).selection().is_empty()
            }
            "note-composer:extract-heading" => {
                if !markdown || self.main_view(cx) != MainView::Editor {
                    return Some(false);
                }
                let editor = self.editor.read(cx);
                let line = editor.cursor().line;
                editor.selection().is_empty() && self.outline.iter().any(|h| h.line == line)
            }
            _ => return None,
        })
    }

    /// Runs one of the three ids, when its check passes; `false` when the id
    /// is not the composer's. A failing check does nothing at all, as
    /// Obsidian's did (no prompt, no notice).
    pub(super) fn run_composer_command(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(can) = self.composer_can_run(id, cx) else {
            return false;
        };
        if can {
            match id {
                "note-composer:merge-file" => self.start_merge(window, cx),
                "note-composer:split-file" => self.start_split(window, cx),
                _ => self.start_extract_heading(window, cx),
            }
        }
        true
    }

    /// The notes the prompt can offer, and the recent ones it shows first,
    /// never the note it was opened from.
    fn composer_candidates(
        &self,
        state: &VaultState,
        source: &str,
    ) -> (Vec<Candidate>, Vec<Candidate>) {
        let candidate = |path: &str| Candidate {
            path: path.to_string(),
            label: path.strip_suffix(".md").unwrap_or(path).to_string(),
        };
        let vault = state.vault();
        let all: Vec<Candidate> = vault
            .files()
            .iter()
            .filter(|f| f.kind == FileKind::Markdown && f.path != source)
            .map(|f| candidate(&f.path))
            .collect();
        let mut recent: Vec<Candidate> = Vec::new();
        let history = self.history.iter().rev().map(|e| e.path.as_str());
        for path in history.chain(self.last_open_files.iter().map(String::as_str)) {
            if path != source
                && FileKind::from_path(path) == FileKind::Markdown
                && vault.exists(path)
                && !recent.iter().any(|c| c.path == path)
            {
                recent.push(candidate(path));
            }
        }
        (all, recent)
    }

    fn open_composer(
        &mut self,
        kind: Kind,
        op: Op,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let source = match &op {
            Op::Merge { source, .. } | Op::Extract { source, .. } => source.clone(),
        };
        let (all, recent) = self.composer_candidates(&state, &source);
        let view = match &self.composer {
            Some(modal) => modal.view.clone(),
            None => cx.new(ComposerPrompt::new),
        };
        let subscription = cx.subscribe_in(&view, window, |this, _, event, window, cx| {
            this.composer_event(event, window, cx)
        });
        view.update(cx, |v, cx| v.open(kind, all, recent, query, window, cx));
        self.close_editor_suggest(cx);
        self.composer = Some(ComposerModal {
            view,
            _subscription: subscription,
            op,
        });
        self.modal = Modal::Composer;
        cx.notify();
    }

    fn start_merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(source) = self.current.clone() {
            self.start_merge_of(source, window, cx);
        }
    }

    /// The merge prompt for `source`: the open note from the command, the
    /// menu's file from "Merge entire file with...".
    pub(super) fn start_merge_of(
        &mut self,
        source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if FileKind::from_path(&source) != FileKind::Markdown {
            return;
        }
        let op = Op::Merge {
            source,
            target: None,
        };
        self.open_composer(Kind::Merge, op, "", window, cx);
    }

    fn start_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.current.clone() else {
            return;
        };
        let (text, range) = {
            let editor = self.editor.read(cx);
            (editor.text().to_string(), editor.selection())
        };
        let op = Op::Extract {
            source,
            text,
            range,
        };
        self.open_composer(Kind::Extract, op, "", window, cx);
    }

    /// The section is read off the one parser's metadata for the buffer as
    /// it stands, and the heading's text is the name offered.
    fn start_extract_heading(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.current.clone() else {
            return;
        };
        let (text, line) = {
            let editor = self.editor.read(cx);
            (editor.text().to_string(), editor.cursor().line)
        };
        let meta = index_markdown(&text);
        let Some(section) = scoria_composer::heading_section(&text, &meta, line) else {
            return;
        };
        let op = Op::Extract {
            source,
            text,
            range: section.range,
        };
        self.open_composer(Kind::Extract, op, &section.name, window, cx);
    }

    fn close_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composer = None;
        self.close_modal(window, cx);
        self.focus_main(window, cx);
        cx.notify();
    }

    fn composer_event(
        &mut self,
        event: &ComposerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(op) = self.composer.as_ref().map(|m| m.op.clone()) else {
            return;
        };
        match (event, op) {
            (ComposerEvent::Close, _) => self.close_composer(window, cx),
            (ComposerEvent::Chosen(Choice::Note(target), place), Op::Merge { source, .. }) => {
                self.merge_chosen(source, target.clone(), false, *place, window, cx)
            }
            (ComposerEvent::CreateNew(name), Op::Merge { source, .. }) => {
                let Some(state) = self.state.clone() else {
                    return;
                };
                let target = state.composer_new_path(name, &source);
                self.merge_chosen(source, target, true, Placement::Append, window, cx)
            }
            (ComposerEvent::Confirmed { dont_ask }, Op::Merge { .. }) => {
                if *dont_ask {
                    if let Some(state) = self.state.clone() {
                        let mut next = state.note_composer().clone().with_defaults();
                        next.ask_before_merging = Some(false);
                        self.change_setting(SettingChange::NoteComposer(next), cx);
                    }
                }
                self.merge_links(window, cx);
            }
            (ComposerEvent::Links(answer), Op::Merge { .. }) => {
                let update = match answer {
                    LinksAnswer::DoNot => false,
                    LinksAnswer::Once => true,
                    // The rename prompt's "Always update": the setting first,
                    // then the links by it.
                    LinksAnswer::Always => {
                        if let Some(state) = self.state.clone() {
                            let mut app = state.app().clone();
                            app.always_update_links = Some(true);
                            self.change_setting(SettingChange::App(app), cx);
                        }
                        true
                    }
                };
                self.finish_merge(update, window, cx);
            }
            (
                ComposerEvent::Chosen(choice, place),
                Op::Extract {
                    source,
                    text,
                    range,
                },
            ) => self.extract_chosen(source, text, range, choice.clone(), *place, window, cx),
            _ => {}
        }
    }

    /// A target is chosen: ask first when `askBeforeMerging` is on (it is
    /// when absent), then the links.
    fn merge_chosen(
        &mut self,
        source: String,
        target: String,
        create: bool,
        place: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let ask = state.note_composer().ask_before_merging.unwrap_or(true);
        let Some(modal) = self.composer.as_mut() else {
            return;
        };
        modal.op = Op::Merge {
            source: source.clone(),
            target: Some((target.clone(), create, place)),
        };
        if ask {
            let text = scoria_composer::confirmation(&source, &target);
            let view = modal.view.clone();
            view.update(cx, |v, cx| v.confirm(text, window, cx));
        } else {
            self.merge_links(window, cx);
        }
    }

    /// The source's edits go to disk first; then, unless
    /// `alwaysUpdateLinks` says to rewrite or nothing links to the source,
    /// the rename's "Update links" question.
    fn merge_links(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let Some(Op::Merge {
            source,
            target: Some((target, _, _)),
        }) = self.composer.as_ref().map(|m| m.op.clone())
        else {
            return;
        };
        self.flush_save(cx);
        if state.update_links_automatically() {
            return self.finish_merge(true, window, cx);
        }
        let (files, links) = state.merge_link_counts(&source, &target);
        if links == 0 {
            return self.finish_merge(false, window, cx);
        }
        if let Some(modal) = &self.composer {
            let view = modal.view.clone();
            view.update(cx, |v, cx| {
                v.ask_links(update_links_prompt(files, links), window, cx)
            });
        }
    }

    fn finish_merge(&mut self, update_links: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let Some(Op::Merge {
            source,
            target: Some((target, create, place)),
        }) = self.composer.as_ref().map(|m| m.op.clone())
        else {
            return;
        };
        self.close_composer(window, cx);
        self.flush_save(cx);
        let outcome = match state.merge_files(&source, &target, create, place, update_links, now())
        {
            Ok(outcome) => outcome,
            Err(err) => return self.report(err, cx),
        };
        let rewrote = outcome.links > 0;
        let was_open = self.current.as_deref() == Some(source.as_str());
        self.after_changes(outcome.changes, cx);
        // The source goes the way `app:delete-file` sends a file, by
        // `trashOption`, without the delete prompt: the merge's own
        // confirmation was the question.
        self.delete_now(&source, window, cx);
        // Where the tab ends up: on the target when links were rewritten
        // or the tab has nowhere to go back to, otherwise back where it
        // was before the source -- the probe's whole record of it, not a
        // rule it explained. A merge from a Files row of a note that was
        // not open leaves the tab alone.
        if was_open {
            if rewrote || self.history_ix == 0 || self.history.is_empty() {
                self.open_file(target, None, cx);
            } else {
                self.navigate_history(cx);
            }
        }
        // No Notice for the merge itself; one for the links, in the
        // rename's words.
        if rewrote {
            self.report(
                format!(
                    "Updated {} link{} in {} file{}.",
                    outcome.links,
                    plural(outcome.links),
                    outcome.files,
                    plural(outcome.files),
                ),
                cx,
            );
        }
        if !outcome.stale.is_empty() {
            let n = outcome.stale.len();
            self.report(
                format!(
                    "{n} file{} changed on disk and {} left alone",
                    plural(n),
                    if n == 1 { "was" } else { "were" }
                ),
                cx,
            );
        }
    }

    /// Enter on a row of the extraction prompt. No confirmation is asked,
    /// even into an existing note; a name the composer refuses raises its
    /// Notice and changes nothing.
    #[allow(clippy::too_many_arguments)]
    fn extract_chosen(
        &mut self,
        source: String,
        text: String,
        range: Range<usize>,
        choice: Choice,
        place: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let (target, create) = match choice {
            Choice::Note(path) => (path, false),
            Choice::Create(name) => {
                // The observed refusal first, then Scoria's own rule for
                // names every platform it runs on can hold.
                let checked = scoria_composer::check_name(&name)
                    .and_then(|()| scoria_formats::validate_file_name(&name));
                if let Err(err) = checked {
                    self.close_composer(window, cx);
                    return self.report(err, cx);
                }
                (state.composer_new_path(&name, &source), true)
            }
        };
        self.close_composer(window, cx);
        let unchanged =
            self.current.as_deref() == Some(source.as_str()) && self.editor.read(cx).text() == text;
        if !unchanged {
            return self.report(format!("{source} changed; nothing was extracted"), cx);
        }
        let outcome = match state.extract_into(&source, &text, range, &target, create, place, now())
        {
            Ok(outcome) => outcome,
            Err(err) => return self.report(err, cx),
        };
        let (range, replacement) = (outcome.plan.range.clone(), outcome.plan.replacement.clone());
        self.after_changes(outcome.changes, cx);
        self.editor
            .update(cx, |e, cx| e.replace_range(range, &replacement, window, cx));
        window.focus(&self.editor.focus_handle(cx), cx);
        cx.notify();
    }

    /// The modal's element, for the overlay.
    pub(super) fn composer_element(&self) -> Option<gpui::AnyElement> {
        self.composer
            .as_ref()
            .map(|m| m.view.clone().into_any_element())
    }

    /// The modal's rows and stage, for the control socket.
    pub(super) fn describe_composer(&self, rows: usize, cx: &App) -> Option<Value> {
        self.composer
            .as_ref()
            .map(|m| m.view.read(cx).describe(rows, cx))
    }
}

#[cfg(test)]
mod tests;
