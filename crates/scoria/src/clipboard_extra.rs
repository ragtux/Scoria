//! The clipboard formats a paste needs that gpui does not read: the files
//! a file manager copied (`text/uri-list`) and copied rich text
//! (`text/html`).
//!
//! gpui's Linux clipboard (the pinned Zed commit's `gpui_linux`) reads
//! `text/plain` and the image types only, on Wayland and X11 alike, and its
//! data offer is private to it. A second Wayland connection cannot read the
//! selection without the data-control protocol, and patching gpui means
//! forking the pinned commit, so the least invasive way in is the desktop's
//! own clipboard tools: `wl-paste` (wl-clipboard) on Wayland, `xclip` on
//! X11, run for the paste with a short deadline. Without them a paste is
//! what it was before: the plain text, or an image. Nothing is read on the
//! other platforms yet (gpui's macOS and Windows clipboards have the same
//! gap; their native calls are left for a session on those machines).

use std::path::PathBuf;

/// What the side channel found on the clipboard.
#[derive(Debug, Default)]
pub struct Extra {
    /// Local files, from a file manager's copy.
    pub files: Vec<PathBuf>,
    /// The `text/html` on offer, when it was asked for and there are no
    /// files.
    pub html: Option<String>,
}

/// Reads the clipboard's files, and its HTML when `want_html`.
pub fn read(want_html: bool) -> Extra {
    #[cfg(target_os = "linux")]
    {
        linux::read(want_html)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = want_html;
        Extra::default()
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::Extra;
    use std::io::Read;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// How long one tool may take before the paste goes on without it.
    const DEADLINE: Duration = Duration::from_millis(800);

    #[derive(Clone, Copy)]
    enum Session {
        Wayland,
        X11,
    }

    /// The session gpui itself chose: Wayland when `WAYLAND_DISPLAY` is set.
    fn session() -> Option<Session> {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
        if set("WAYLAND_DISPLAY") {
            Some(Session::Wayland)
        } else if set("DISPLAY") {
            Some(Session::X11)
        } else {
            None
        }
    }

    /// Runs `program` and returns its stdout, or `None` when it is missing,
    /// fails or overruns the deadline.
    fn run(program: &str, args: &[&str]) -> Option<Vec<u8>> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut stdout = child.stdout.take()?;
        // Read on a thread of its own: a large offer would fill the pipe
        // and the tool would never exit.
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            stdout.read_to_end(&mut out).ok().map(|_| out)
        });
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let out = reader.join().ok().flatten()?;
                    return status.success().then_some(out);
                }
                Ok(None) if start.elapsed() < DEADLINE => {
                    std::thread::sleep(Duration::from_millis(3));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }

    fn types(session: Session) -> Vec<String> {
        let out = match session {
            Session::Wayland => run("wl-paste", &["--list-types"]),
            Session::X11 => run("xclip", &["-selection", "clipboard", "-t", "TARGETS", "-o"]),
        };
        out.map(|b| {
            String::from_utf8_lossy(&b)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
    }

    fn get(session: Session, ty: &str) -> Option<Vec<u8>> {
        match session {
            Session::Wayland => run("wl-paste", &["--no-newline", "--type", ty]),
            Session::X11 => run("xclip", &["-selection", "clipboard", "-t", ty, "-o"]),
        }
    }

    pub(super) fn read(want_html: bool) -> Extra {
        let Some(session) = session() else {
            return Extra::default();
        };
        let types = types(session);
        // gpui marks its own Wayland offer with `pid/<pid>`: Scoria's own
        // copy is text gpui reads, and asking this process for it from a
        // tool would wait on the very thread that is asking.
        let own = format!("pid/{}", std::process::id());
        if types.is_empty() || types.contains(&own) {
            return Extra::default();
        }
        let mut extra = Extra::default();
        if types.iter().any(|t| t == "text/uri-list") {
            if let Some(list) = get(session, "text/uri-list") {
                extra.files = scoria_paste::clipboard::uri_list_paths(
                    &scoria_paste::clipboard::decode_text(&list),
                )
                .into_iter()
                .map(|p| PathBuf::from(std::ffi::OsString::from_vec(p)))
                .filter(|p| p.is_file())
                .collect();
            }
        }
        if extra.files.is_empty() && want_html && types.iter().any(|t| t == "text/html") {
            extra.html = get(session, "text/html")
                .map(|b| scoria_paste::clipboard::decode_text(&b))
                .filter(|h| !h.trim().is_empty());
        }
        extra
    }
}
