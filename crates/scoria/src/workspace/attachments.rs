//! Files brought in from outside the vault: `editor:attach-file` ("Insert
//! attachment"), `editor:download-attachments` ("Download attachments for
//! current file") and `app:open-file` ("Open file from outside the
//! vault..."), as Obsidian 1.15.0 did them for the 2026-11-09 probe
//! ([[Specification#Observed attachment commands, Obsidian 1.15.0 (installer build 1.13.7)]],
//! [[Specification#Open file from outside the vault, a path chosen]]).
//!
//! The names, the offered links and the rewritten text are
//! `scoria_vault::attachments`'; the folder a file lands in is
//! `attachmentFolderPath`'s (`scoria_vault::ops::attachment_folder`), and
//! the embed written is autocompletion's (`scoria_suggest::file_link`), so a
//! picked file, a downloaded one and a `![[` accepted from the popup are
//! linked the same way.

use super::download_modal::{DownloadEvent, DownloadModal};
use super::*;
use scoria_vault::attachments as att;

/// The extensions Obsidian 1.15.0's open dialog filters on ("Supported
/// files"), in the order it sent them: every extension a registered view
/// opens.
pub(super) const OPEN_FILE_EXTENSIONS: [&str; 25] = [
    "md", "bmp", "png", "jpg", "jpeg", "gif", "svg", "webp", "avif", "mp3", "wav", "m4a", "3gp",
    "flac", "ogg", "oga", "opus", "mp4", "webm", "ogv", "mov", "mkv", "pdf", "canvas", "base",
];

/// The download modal while it is up, and the note it was opened for.
pub(super) struct DownloadsModal {
    view: Entity<DownloadModal>,
    _subscription: Subscription,
    note: String,
}

/// The vault path of `picked` when it is inside the vault at `root`, `/`
/// separated; `None` for a file anywhere else. Both are made absolute and
/// their links resolved first, so a vault opened by a relative path or
/// through a link still recognises its own files.
pub(super) fn path_in_vault(root: &Path, picked: &Path) -> Option<String> {
    let root = std::fs::canonicalize(root).ok()?;
    let picked = std::fs::canonicalize(picked).ok()?;
    let rel = picked.strip_prefix(&root).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    (!rel.is_empty()).then_some(rel)
}

/// Whether the open dialog's filter lets `path` through.
pub(super) fn opens_from_dialog(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| OPEN_FILE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

impl VaultState {
    /// The embed of the vault file `target` written in `note`: the path in
    /// `newLinkFormat`, `useMarkdownLinks` choosing the shape, and no
    /// display text -- `![](pic%20one.png)` where autocompletion's row
    /// writes the name in the brackets (Insert attachment, 1.15.0).
    pub(super) fn embed_link(&self, target: &str, note: &str) -> String {
        let mut resolver = self.cache().resolver().clone();
        resolver.insert(target);
        let opts = scoria_suggest::Options {
            format: self.link_format(),
            markdown_links: self.app().use_markdown_links.unwrap_or(false),
        };
        att::with_alt(
            &scoria_suggest::file_link(&resolver, target, note, true, &opts),
            "",
        )
    }

    /// Writes `bytes` as the attachment `stem.ext` of `note`: the folder
    /// `attachmentFolderPath` gives, created if missing, and the next free
    /// name in it.
    pub(super) fn add_attachment(
        &self,
        note: &str,
        stem: &str,
        ext: &str,
        bytes: &[u8],
    ) -> Result<(String, Changes), String> {
        let setting = self.app().attachment_folder_path.clone();
        let folder = scoria_vault::ops::attachment_folder(setting.as_deref(), note);
        let (path, files) = self
            .vault
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .add_attachment(&folder, stem, ext, bytes)
            .map_err(|e| e.user_message())?;
        Ok((path, self.absorb(files)))
    }

    /// Copies the file at `source` (anywhere on disk, the vault included)
    /// in as an attachment of `note`, under its own name.
    pub(super) fn import_attachment(
        &self,
        note: &str,
        source: &Path,
    ) -> Result<(String, Changes), String> {
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{} is not a file", source.display()))?;
        let (stem, ext) = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_string()),
            _ => (name.clone(), String::new()),
        };
        let bytes = std::fs::read(source)
            .map_err(|e| format!("Could not read {}: {e}", source.display()))?;
        self.add_attachment(note, &stem, &ext, &bytes)
    }
}

impl Workspace {
    /// The note the attachment commands act on: a Markdown file in the
    /// active tab, shown in an editing mode.
    fn attachment_note(&self) -> Option<String> {
        let note = self.current.clone()?;
        (self.mode.is_editing() && FileKind::from_path(&note) == FileKind::Markdown).then_some(note)
    }

    /// `can_run` for the three, `None` for any other id.
    pub(super) fn attachments_can_run(&self, id: &str) -> Option<bool> {
        match id {
            // Offered in Live Preview, not in reading view or an empty tab.
            "editor:attach-file" | "editor:download-attachments" => {
                Some(self.attachment_note().is_some())
            }
            "app:open-file" => Some(self.state.is_some()),
            _ => None,
        }
    }

    /// "Insert attachment": one file asked for, copied in, and its embed
    /// written over the selection with the caret after it. Picking nothing
    /// changes nothing.
    pub(super) fn attach_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(note) = self.attachment_note() else {
            return;
        };
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Insert attachment".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let source = match picked.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.remove(0),
                Ok(Err(err)) => {
                    this.update(cx, |ws, cx| {
                        ws.report(format!("Could not open the file dialog: {err}"), cx)
                    })
                    .ok();
                    return;
                }
                _ => return,
            };
            this.update_in(cx, |ws, window, cx| {
                ws.finish_attach(&note, &source, window, cx)
            })
            .ok();
        })
        .detach();
    }

    fn finish_attach(
        &mut self,
        note: &str,
        source: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        // The note the dialog was opened over, still in front.
        if self.current.as_deref() != Some(note) {
            return;
        }
        let (path, changes) = match state.import_attachment(note, source) {
            Ok(done) => done,
            Err(err) => return self.report(err, cx),
        };
        self.after_changes(changes, cx);
        let link = state.embed_link(&path, note);
        let range = self.editor.read(cx).selection();
        self.editor
            .update(cx, |e, cx| e.replace_range(range, &link, window, cx));
        window.focus(&self.editor.focus_handle(cx), cx);
        cx.notify();
    }

    /// "Download attachments for current file": the notice when the note
    /// has no remote image, else the modal over its URLs.
    pub(super) fn start_download_attachments(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(note) = self.attachment_note() else {
            return;
        };
        let urls = att::distinct_urls(&att::external_embeds(self.editor.read(cx).text()));
        if urls.is_empty() {
            return self.report(att::NOTHING_TO_DOWNLOAD, cx);
        }
        let view = cx.new(|cx| DownloadModal::new(urls, cx));
        let subscription =
            cx.subscribe_in(&view, window, |this, _, event, window, cx| match event {
                DownloadEvent::Download(urls) => {
                    let urls = urls.clone();
                    this.close_downloads(window, cx);
                    this.download_attachments(urls, window, cx);
                }
                DownloadEvent::Close => this.close_downloads(window, cx),
            });
        self.close_editor_suggest(cx);
        window.focus(&view.focus_handle(cx), cx);
        self.downloads = Some(DownloadsModal {
            view,
            _subscription: subscription,
            note,
        });
        self.modal = Modal::Downloads;
        cx.notify();
    }

    fn close_downloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let note = self.downloads.take().map(|d| d.note);
        if self.modal == Modal::Downloads {
            self.close_modal(window, cx);
        }
        if note.is_some() && note == self.current {
            window.focus(&self.editor.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Fetches `urls` one after another, saving each as it arrives; when
    /// every one came back the note's embeds of them are rewritten and the
    /// count reported. The first failure ends the run with no notice and no
    /// rewrite, the files already saved left where they are -- Obsidian's
    /// behaviour for a 404 and for a refused connection.
    fn download_attachments(
        &mut self,
        urls: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(note) = self.current.clone() else {
            return;
        };
        if urls.is_empty() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let mut links: Vec<(String, String)> = Vec::new();
            for url in urls {
                let fetch_url = url.clone();
                let got = cx
                    .background_executor()
                    .spawn(async move { crate::remote::download(&fetch_url) })
                    .await;
                let Ok(got) = got else {
                    return;
                };
                let (stem, ext) = att::downloaded_name(&url, got.content_type.as_deref());
                let saved = this.update(cx, |ws, cx| {
                    let state = ws.state.clone()?;
                    match state.add_attachment(&note, &stem, &ext, &got.bytes) {
                        Ok((path, changes)) => {
                            ws.after_changes(changes, cx);
                            Some(state.embed_link(&path, &note))
                        }
                        Err(err) => {
                            ws.report(err, cx);
                            None
                        }
                    }
                });
                match saved {
                    Ok(Some(link)) => links.push((url, link)),
                    _ => return,
                }
            }
            this.update_in(cx, |ws, window, cx| {
                ws.rewrite_downloaded(&note, &links, window, cx)
            })
            .ok();
        })
        .detach();
    }

    fn rewrite_downloaded(
        &mut self,
        note: &str,
        links: &[(String, String)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        if self.current.as_deref() == Some(note) {
            let (_, edits) = att::rewrite(self.editor.read(cx).text(), links);
            self.editor
                .update(cx, |e, cx| e.replace_ranges(edits, window, cx));
        } else {
            // The note left the tab while the files came in: its bytes on
            // disk are rewritten instead.
            let written = (|| -> Result<Changes, String> {
                let abs = state.vault().write_path(note).map_err(|e| e.to_string())?;
                let text = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
                let (new, _) = att::rewrite(&text, links);
                scoria_vault::write_no_follow(&abs, new.as_bytes()).map_err(|e| e.to_string())?;
                Ok(state.absorb(vec![FileChange::Modified(note.to_string())]))
            })();
            match written {
                Ok(changes) => self.after_changes(changes, cx),
                Err(err) => return self.report(err, cx),
            }
        }
        self.report(att::downloaded_notice(links.len()), cx);
    }

    /// The modal's element, for the overlay.
    pub(super) fn downloads_element(&self) -> Option<gpui::AnyElement> {
        self.downloads
            .as_ref()
            .map(|d| d.view.clone().into_any_element())
    }

    pub(super) fn describe_downloads(&self, cx: &App) -> Option<Value> {
        self.downloads.as_ref().map(|d| d.view.read(cx).describe())
    }

    /// "Open file from outside the vault...": a file asked for; one inside
    /// the vault opens in the active tab as a link click would. Scoria has
    /// no tab for a file outside its vault, which is what Obsidian opens one
    /// in (a new tab after the active one, its leaf `file:`-pathed), so that
    /// pick is refused with a message rather than half done.
    pub(super) fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.vault_root() else {
            return;
        };
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let picked = match picked.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.remove(0),
                Ok(Err(err)) => {
                    this.update(cx, |ws, cx| {
                        ws.report(format!("Could not open the file dialog: {err}"), cx)
                    })
                    .ok();
                    return;
                }
                _ => return,
            };
            this.update(cx, |ws, cx| {
                if !opens_from_dialog(&picked) {
                    // Obsidian's dialog would not have let it be picked.
                    return ws.report(
                        format!("{} is not a file Scoria opens", picked.display()),
                        cx,
                    );
                }
                match path_in_vault(&root, &picked) {
                    Some(path) => {
                        // The switcher's open: the active tab, as a
                        // link click would. The note already there stays.
                        if ws.current.as_deref() != Some(path.as_str()) {
                            ws.open_file(path, None, cx);
                        }
                    }
                    None => ws.report(
                        format!(
                            "{} is outside the vault; Scoria opens files from inside it only",
                            picked.display()
                        ),
                        cx,
                    ),
                }
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nprobe";

    /// A vault with `Sub/Note.md` and `app.json` as given, and a file
    /// `pic one.png` outside it.
    fn setup(app_json: &str) -> (tempfile::TempDir, VaultState, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("Probe Vault");
        std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
        std::fs::create_dir_all(vault.join("Sub")).unwrap();
        std::fs::write(vault.join(".obsidian/app.json"), app_json).unwrap();
        std::fs::write(vault.join("Sub/Note.md"), "line one\nline two\n").unwrap();
        let outside = dir.path().join("Outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("pic one.png"), PNG).unwrap();
        let state = VaultState::load(&vault).unwrap();
        (dir, state, outside.join("pic one.png"))
    }

    /// Insert attachment under each `attachmentFolderPath` the probe ran
    /// (`s02`): where the copy lands and the embed written for it.
    #[test]
    fn an_inserted_attachment_lands_where_obsidian_put_it_with_its_embed() {
        for (setting, landed) in [
            ("/", "pic one.png"),
            ("./", "Sub/pic one.png"),
            ("./att", "Sub/att/pic one.png"),
            ("Attach", "Attach/pic one.png"),
        ] {
            let (dir, state, pic) = setup(&format!("{{\"attachmentFolderPath\": \"{setting}\"}}"));
            let (path, _) = state.import_attachment("Sub/Note.md", &pic).unwrap();
            assert_eq!(path, landed, "under {setting}");
            assert_eq!(
                std::fs::read(dir.path().join("Probe Vault").join(landed)).unwrap(),
                PNG
            );
            assert_eq!(state.embed_link(&path, "Sub/Note.md"), "![[pic one.png]]");
            assert!(pic.exists(), "the source is copied, not moved");
        }
        // The same file again takes the next number.
        let (_dir, state, pic) = setup("{\"attachmentFolderPath\": \"/\"}");
        state.import_attachment("Sub/Note.md", &pic).unwrap();
        let (again, _) = state.import_attachment("Sub/Note.md", &pic).unwrap();
        assert_eq!(again, "pic one 1.png");
        assert_eq!(
            state.embed_link(&again, "Sub/Note.md"),
            "![[pic one 1.png]]"
        );
    }

    /// `s02` `markdownLinks` and `markdownRelative`, and a file already in
    /// the vault copied again (`alreadyInVault`).
    #[test]
    fn an_inserted_attachment_follows_the_link_settings() {
        let (_dir, state, pic) = setup("{\"useMarkdownLinks\": true}");
        let (path, _) = state.import_attachment("Sub/Note.md", &pic).unwrap();
        assert_eq!(state.embed_link(&path, "Sub/Note.md"), "![](pic%20one.png)");
        let (_dir, state, pic) =
            setup("{\"useMarkdownLinks\": true, \"newLinkFormat\": \"relative\"}");
        let (path, _) = state.import_attachment("Sub/Note.md", &pic).unwrap();
        assert_eq!(
            state.embed_link(&path, "Sub/Note.md"),
            "![](../pic%20one.png)"
        );
        let (dir, state, _) = setup("{}");
        let inside = dir.path().join("Probe Vault/Sub/inside.png");
        std::fs::write(&inside, PNG).unwrap();
        let (path, _) = state.import_attachment("Sub/Note.md", &inside).unwrap();
        assert_eq!(path, "inside.png");
        assert_eq!(state.embed_link(&path, "Sub/Note.md"), "![[inside.png]]");
    }

    /// `s03` `collision` and `markdownLinks` end to end: each URL saved in
    /// document order under its name (a `photo.png` already there), and
    /// the note's bytes after the rewrite equal Obsidian's.
    #[test]
    fn downloaded_attachments_rewrite_the_note_to_obsidians_bytes() {
        let m = "http://127.0.0.1:9343";
        let note = format!(
            "a ![]({m}/img/photo.png) b\n![alt]({m}/img/two%20words.png)\n[link]({m}/img/pic.jpg)\n\
             ![x]({m}/png-noext)\n![j]({m}/jpeg-noext)\n![q]({m}/img/photo.png?size=2)\n\
             ![sized|100]({m}/img/pic.jpg)\n![t]({m}/img/again/photo.png \"a title\")\n\
             <img src=\"{m}/img/pic.jpg\">\n![[{m}/img/photo.png]]\n![pdf]({m}/doc.pdf)\n"
        );
        let types = |url: &str| -> &'static str {
            if url.contains("jpeg") || url.ends_with(".jpg") {
                "image/jpeg"
            } else if url.ends_with(".pdf") {
                "application/pdf"
            } else {
                "image/png"
            }
        };
        for (app, existing, expected) in [
            (
                "{}",
                true,
                format!(
                    "a ![[photo 1.png]] b\n![[two%20words.png|alt]]\n[link]({m}/img/pic.jpg)\n\
                     ![[Image.png|x]]\n![[Image.jpg|j]]\n![[photo 2.png|q]]\n![[pic.jpg|sized]]\n\
                     ![[photo 3.png|t]]\n<img src=\"{m}/img/pic.jpg\">\n![[{m}/img/photo.png]]\n\
                     ![[doc.pdf|pdf]]\n"
                ),
            ),
            (
                "{\"useMarkdownLinks\": true}",
                false,
                format!(
                    "a ![](photo.png) b\n![alt](two%20words.png)\n[link]({m}/img/pic.jpg)\n\
                     ![x](Image.png)\n![j](Image.jpg)\n![q](photo%201.png)\n![sized](pic.jpg)\n\
                     ![t](photo%202.png)\n<img src=\"{m}/img/pic.jpg\">\n![[{m}/img/photo.png]]\n\
                     ![pdf](doc.pdf)\n"
                ),
            ),
        ] {
            let (dir, state, _) = setup(app);
            if existing {
                std::fs::write(dir.path().join("Probe Vault/photo.png"), b"existing").unwrap();
            }
            let urls = att::distinct_urls(&att::external_embeds(&note));
            assert_eq!(urls.len(), 8);
            let mut links = Vec::new();
            for url in &urls {
                let (stem, ext) = att::downloaded_name(url, Some(types(url)));
                let (path, _) = state
                    .add_attachment("Sub/Page.md", &stem, &ext, PNG)
                    .unwrap();
                links.push((url.clone(), state.embed_link(&path, "Sub/Page.md")));
            }
            let (out, _) = att::rewrite(&note, &links);
            assert_eq!(out, expected, "with {app}");
            if existing {
                assert_eq!(
                    std::fs::read(dir.path().join("Probe Vault/photo.png")).unwrap(),
                    b"existing"
                );
            }
        }
    }

    /// The four this round settled run and are Obsidian's ids; the
    /// frontmatter converter is left unrunnable.
    #[test]
    fn the_attachment_and_spellcheck_commands_are_runnable() {
        let known = scoria_commands::known_ids();
        for id in [
            "editor:toggle-spellcheck",
            "editor:attach-file",
            "editor:download-attachments",
            "app:open-file",
        ] {
            assert!(RUNNABLE.contains(&id), "{id} runnable");
            assert!(known.contains(&id), "{id} is Obsidian's");
        }
    }

    #[test]
    fn the_dialogs_filter_is_obsidians_twenty_five_extensions() {
        assert_eq!(OPEN_FILE_EXTENSIONS.len(), 25);
        assert!(opens_from_dialog(Path::new("/x/Note.md")));
        assert!(opens_from_dialog(Path::new("/x/pic.PNG")));
        assert!(opens_from_dialog(Path::new("/x/Board.canvas")));
        assert!(opens_from_dialog(Path::new("/x/t.base")));
        assert!(!opens_from_dialog(Path::new("/x/doc.txt")));
        assert!(!opens_from_dialog(Path::new("/x/noext")));
    }

    #[test]
    fn a_picked_path_is_in_the_vault_only_under_its_root() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("Probe Vault");
        std::fs::create_dir_all(vault.join("Sub")).unwrap();
        std::fs::write(vault.join("Sub/Deep.md"), "deep\n").unwrap();
        std::fs::create_dir_all(dir.path().join("Outside")).unwrap();
        std::fs::write(dir.path().join("Outside/Outside Note.md"), "o\n").unwrap();
        // A sibling whose name starts like the vault's is not inside it.
        std::fs::create_dir_all(dir.path().join("Probe Vault 2")).unwrap();
        std::fs::write(dir.path().join("Probe Vault 2/N.md"), "n\n").unwrap();
        assert_eq!(
            path_in_vault(&vault, &vault.join("Sub/Deep.md")).as_deref(),
            Some("Sub/Deep.md")
        );
        assert_eq!(
            path_in_vault(&vault, &dir.path().join("Outside/Outside Note.md")),
            None
        );
        assert_eq!(
            path_in_vault(&vault, &dir.path().join("Probe Vault 2/N.md")),
            None
        );
        assert_eq!(
            path_in_vault(&vault.join("Sub/.."), &vault.join("Sub/Deep.md")).as_deref(),
            Some("Sub/Deep.md")
        );
    }
}
