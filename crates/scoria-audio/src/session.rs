//! A recording in progress: a worker thread that encodes each block the
//! microphone hands over as it arrives, and at the stop drains what is
//! left and writes the file.
//!
//! The channel into the worker is unbounded on purpose. The audio callback
//! must never wait (a blocked callback drops samples on every host), and
//! oxideav-aac encodes at about 0.75x real time, so the worker falls behind
//! by about a third of the recording's length: that backlog has to queue
//! somewhere. What queues is the microphone's own blocks, never the whole
//! recording, and each block is gone once its hops are encoded.
//!
//! The file is written as `.<name>.m4a.part` beside its destination and
//! renamed into place only once complete, so the vault never shows a
//! half-written recording; [`wait_pending`] holds a quitting app until every
//! worker has finished, so quitting during the drain does not lose one.

use crate::encode::Encoding;
use crate::EXTENSION;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Where the finished recording goes.
#[derive(Debug, Clone)]
pub struct Destination {
    /// The folder, absolute, already checked to be inside the vault.
    pub folder: PathBuf,
    /// `Recording YYYYMMDDHHmmss`.
    pub stem: String,
    /// The MP4 creation time, seconds since 1904.
    pub creation: u32,
}

/// What a finished recording came to.
#[derive(Debug, Clone)]
pub struct Finished {
    /// The file, absolute.
    pub path: PathBuf,
    /// Seconds of audio in it.
    pub seconds: f64,
    /// From the stop to the file being in place: the encoder catching up.
    pub drain: Duration,
}

enum Msg {
    Pcm(Vec<f32>),
    Finish(Destination, Sender<Result<Finished, String>>, Instant),
}

/// Hands the microphone's blocks to the worker. Cheap to clone; the audio
/// callback owns one.
#[derive(Clone)]
pub struct Feed {
    tx: Sender<Msg>,
}

impl Feed {
    /// Queues one block of interleaved samples. Never waits.
    pub fn send(&self, interleaved: &[f32]) {
        let _ = self.tx.send(Msg::Pcm(interleaved.to_vec()));
    }
}

/// The worker of one recording.
pub struct Session {
    tx: Sender<Msg>,
}

static PENDING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

impl Session {
    /// Starts the worker for `channels` channels at `sample_rate`.
    pub fn start(sample_rate: u32, channels: usize) -> Result<(Session, Feed), String> {
        let encoding = Encoding::new(sample_rate)?;
        let (tx, rx) = channel::<Msg>();
        let handle = std::thread::Builder::new()
            .name("audio-recorder".into())
            .spawn(move || work(encoding, channels, rx))
            .map_err(|e| format!("Could not start the recorder: {e}"))?;
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|h| !h.is_finished());
        pending.push(handle);
        Ok((Session { tx: tx.clone() }, Feed { tx }))
    }

    /// Ends the recording: every block already queued is encoded, then the
    /// file is written to `dest`. The answer arrives on the receiver once
    /// the file is in place. Call it after the capture has stopped, so no
    /// block arrives after the end.
    pub fn finish(self, dest: Destination) -> Receiver<Result<Finished, String>> {
        let (done_tx, done_rx) = channel();
        if let Err(e) = self.tx.send(Msg::Finish(dest, done_tx, Instant::now())) {
            // The worker is gone (its encoder failed): say so.
            if let Msg::Finish(_, done, _) = e.0 {
                let _ = done.send(Err("The recorder stopped unexpectedly".into()));
            }
        }
        done_rx
    }
}

fn work(mut encoding: Encoding, channels: usize, rx: Receiver<Msg>) {
    let mut failed: Option<String> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Pcm(block) => {
                if failed.is_none() {
                    if let Err(e) = encoding.push(&block, channels) {
                        failed = Some(e);
                    }
                }
            }
            Msg::Finish(dest, done, stopped) => {
                let result = match failed.take() {
                    Some(e) => Err(e),
                    None => {
                        let seconds = encoding.seconds();
                        encoding
                            .finish(dest.creation)
                            .and_then(|bytes| write_into_place(&dest, &bytes))
                            .map(|path| Finished {
                                path,
                                seconds,
                                drain: stopped.elapsed(),
                            })
                    }
                };
                let _ = done.send(result);
                return;
            }
        }
    }
    // Every sender dropped without a Finish: the recording was abandoned.
}

/// Writes `bytes` as `.<name>.part` in the folder, then renames it to the
/// first free `<stem>.m4a`, `<stem> 1.m4a`, … Returns the final path.
pub fn write_into_place(dest: &Destination, bytes: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(&dest.folder)
        .map_err(|e| format!("Could not create {}: {e}", dest.folder.display()))?;
    let part = dest.folder.join(format!(".{}.{EXTENSION}.part", dest.stem));
    write_synced(&part, bytes).map_err(|e| format!("Could not write the recording: {e}"))?;
    let target = free_name(&dest.folder, &dest.stem);
    std::fs::rename(&part, &target).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("Could not save the recording: {e}")
    })?;
    Ok(target)
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

fn free_name(folder: &Path, stem: &str) -> PathBuf {
    let plain = folder.join(format!("{stem}.{EXTENSION}"));
    if !plain.exists() {
        return plain;
    }
    (1..)
        .map(|n| folder.join(format!("{stem} {n}.{EXTENSION}")))
        .find(|p| !p.exists())
        .expect("some number is free")
}

/// Blocks until every recording's worker has finished: for an app that is
/// quitting while a recording is still being encoded.
pub fn wait_pending() {
    let handles: Vec<JoinHandle<()>> =
        std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
    for h in handles {
        let _ = h.join();
    }
}

/// Whether any recording is still being encoded.
pub fn any_pending() -> bool {
    PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|h| !h.is_finished())
}

/// Seconds since 1904-01-01, the MP4 epoch, for `unix` seconds.
pub fn mp4_time(unix: i64) -> u32 {
    (unix + 2_082_844_800).clamp(0, u32::MAX as i64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taken_name_is_numbered_and_the_part_file_goes() {
        let dir = tempfile::tempdir().unwrap();
        let dest = Destination {
            folder: dir.path().join("Attachments"),
            stem: "Recording 20261110175445".into(),
            creation: 0,
        };
        let first = write_into_place(&dest, b"one").unwrap();
        let second = write_into_place(&dest, b"two").unwrap();
        assert_eq!(first.file_name().unwrap(), "Recording 20261110175445.m4a");
        assert_eq!(
            second.file_name().unwrap(),
            "Recording 20261110175445 1.m4a"
        );
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        let mut names: Vec<_> = std::fs::read_dir(&dest.folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "Recording 20261110175445 1.m4a",
                "Recording 20261110175445.m4a"
            ]
        );
    }

    #[test]
    fn the_mp4_epoch_is_1904() {
        assert_eq!(mp4_time(0), 2_082_844_800);
    }
}
