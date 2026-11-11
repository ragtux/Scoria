//! The Audio recorder's two commands and its ribbon button, as Obsidian
//! 1.15.0 behaved for the 2026-11-10 probe
//! ([[Specification#Audio recorder]]); the capture, encoding and file are
//! [`scoria_audio`].
//!
//! `audio-recorder:start` is offered everywhere while nothing records,
//! `audio-recorder:stop` only while something does, and the ribbon's
//! "Start/stop recording" (`lucide-mic`) is active meanwhile; nothing else
//! shows, as in Obsidian. At the stop the file is named from the local time
//! (`Recording YYYYMMDDHHmmss.m4a`) and goes where `attachmentFolderPath`
//! says, the active file standing in for the note; then:
//!
//! - a note being edited gets `\n![[…]]\n` over its selection;
//! - a note in reading view gets nothing;
//! - anything else (an empty tab, a canvas, an image) opens the recording.
//!
//! The encoder runs at about 0.75x real time, so the file is only ready a
//! while after the stop (about a third of the recording's length); until
//! then a notice says "Saving recording…", which is Scoria's own.

use super::*;
use scoria_audio::capture::Capture;
use scoria_audio::session::{mp4_time, Destination, Finished};

/// What the recording lands in, decided at the stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Landing {
    /// A note in Live Preview or Source mode: the embed over its selection.
    Note(String),
    /// A note in reading view: nothing.
    Reading,
    /// Anything else: the recording opens.
    Open,
}

pub(super) const SAVING: &str = "Saving recording…";

/// The vault path of `abs` under `root`, `/`-separated.
fn vault_path(root: &Path, abs: &Path) -> Option<String> {
    let rel = abs.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// The attachment folder for a recording made with `active` the active
/// file (the vault root standing in when there is none), absolute and
/// checked to be inside the vault; the root when it is not.
fn recording_folder(state: &VaultState, active: &str) -> (PathBuf, Option<String>) {
    let folder =
        scoria_vault::ops::attachment_folder(state.app().attachment_folder_path.as_deref(), active);
    let probe = if folder.is_empty() {
        "x".to_string()
    } else {
        format!("{folder}/x")
    };
    match state.vault().write_path(&probe) {
        Ok(abs) => (
            abs.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| state.root.clone()),
            None,
        ),
        Err(err) => (state.root.clone(), Some(err.to_string())),
    }
}

impl Workspace {
    /// Whether a recording is capturing.
    pub(super) fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// `audio-recorder:start`.
    pub(super) fn start_recording(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() || self.state.is_none() {
            return;
        }
        match Capture::start() {
            Ok(capture) => {
                eprintln!(
                    "audio recorder: recording from {} at {} Hz, {} channel(s) of {}",
                    capture.device, capture.sample_rate, capture.channels, capture.format
                );
                self.recording = Some(capture);
                cx.notify();
            }
            Err(err) => self.report(err, cx),
        }
    }

    /// Where the recording should land, from what is active now.
    fn recording_landing(&self) -> Landing {
        match self.current.as_deref() {
            Some(path) if FileKind::from_path(path) == FileKind::Markdown => {
                if self.mode.is_editing() {
                    Landing::Note(path.to_string())
                } else {
                    Landing::Reading
                }
            }
            _ => Landing::Open,
        }
    }

    /// `audio-recorder:stop`: the microphone stops at once; the file is
    /// written once the encoder has caught up, and only then is the note
    /// edited or the recording opened.
    pub(super) fn stop_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let Some(capture) = self.recording.take() else {
            return;
        };
        let landing = self.recording_landing();
        let note = self.current.clone().unwrap_or_default();
        let now = chrono::Local::now();
        let stem = scoria_audio::file_stem(now.naive_local());
        let session = capture.stop();
        let (folder_abs, refused) = recording_folder(&state, &note);
        if let Some(err) = refused {
            // Still finished, so nothing is lost: into the vault root.
            self.report(format!("Recording saved to the vault root: {err}"), cx);
        }
        let done = session.finish(Destination {
            folder: folder_abs,
            stem,
            creation: mp4_time(now.timestamp()),
        });
        self.recordings_saving += 1;
        self.keep_saving_notice(cx);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    done.recv()
                        .unwrap_or_else(|_| Err("The recorder stopped unexpectedly".into()))
                })
                .await;
            this.update_in(cx, |ws, window, cx| {
                ws.recording_saved(result, landing, window, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Renews "Saving recording…" until every recording is in place: a
    /// notice fades after a few seconds, and the drain can take longer.
    fn keep_saving_notice(&mut self, cx: &mut Context<Self>) {
        if self.recordings_saving == 0 {
            return;
        }
        self.report(SAVING, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            this.update(cx, |ws, cx| ws.keep_saving_notice(cx)).ok();
        })
        .detach();
    }

    fn recording_saved(
        &mut self,
        result: Result<Finished, String>,
        landing: Landing,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recordings_saving = self.recordings_saving.saturating_sub(1);
        if self.recordings_saving == 0 {
            self.notices.expire_text(SAVING);
        }
        let Some(state) = self.state.clone() else {
            return;
        };
        let finished = match result {
            Ok(finished) => finished,
            Err(err) => return self.report(err, cx),
        };
        eprintln!(
            "audio recorder: {:.1} s recorded, saved {:.1} s after the stop",
            finished.seconds,
            finished.drain.as_secs_f64()
        );
        self.last_recording_drain = Some((finished.seconds, finished.drain.as_secs_f64()));
        let changes = state.apply(std::slice::from_ref(&finished.path));
        self.after_changes(changes, cx);
        let Some(path) = vault_path(&state.root, &finished.path) else {
            return;
        };
        match landing {
            Landing::Note(note) => {
                // The note still being edited here: the embed over what is
                // selected now. A note left during the drain gets nothing,
                // the file being in the vault either way.
                if self.current.as_deref() == Some(note.as_str()) && self.mode.is_editing() {
                    let embed = scoria_audio::embed_insertion(&state.embed_link(&path, &note));
                    let range = self.editor.read(cx).selection();
                    self.editor
                        .update(cx, |e, cx| e.replace_range(range, &embed, window, cx));
                }
            }
            Landing::Reading => {}
            Landing::Open => {
                if self.current.is_some() {
                    self.new_tab(cx);
                }
                self.open_file(path, None, cx);
            }
        }
        cx.notify();
    }

    /// Quitting: a recording still capturing is stopped and kept, and every
    /// recording still being encoded is finished before the app goes.
    pub(super) fn finish_recordings_for_quit(&mut self) {
        if let (Some(capture), Some(state)) = (self.recording.take(), self.state.clone()) {
            let now = chrono::Local::now();
            let (folder_abs, _) = recording_folder(&state, self.current.as_deref().unwrap_or(""));
            let _ = capture.stop().finish(Destination {
                folder: folder_abs,
                stem: scoria_audio::file_stem(now.naive_local()),
                creation: mp4_time(now.timestamp()),
            });
        }
        scoria_audio::session::wait_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recorder_commands_are_runnable_and_obsidians() {
        let known = scoria_commands::known_ids();
        for id in ["audio-recorder:start", "audio-recorder:stop"] {
            assert!(RUNNABLE.contains(&id), "{id} runnable");
            assert!(known.contains(&id), "{id} is Obsidian's");
        }
    }

    #[test]
    fn a_vault_path_is_slash_separated() {
        let root = Path::new("/v");
        assert_eq!(
            vault_path(root, Path::new("/v/Attachments/Recording 1.m4a")).as_deref(),
            Some("Attachments/Recording 1.m4a")
        );
        assert_eq!(vault_path(root, Path::new("/elsewhere/x.m4a")), None);
    }
}
