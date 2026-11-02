//! One window per vault: the handover a second `scoria` makes.
//!
//! Obsidian is one process holding a window per vault, so launching it
//! again — from a terminal, from a file manager, or by following an
//! `obsidian://` link — raises the window that already shows the vault and
//! opens what the arguments named in it. Scoria is one process per window,
//! so the same behaviour needs a rendezvous: each run that opens a vault
//! binds a socket named after that vault, and a later run finds it,
//! connects, sends what it was asked for and exits.
//!
//! The socket speaks the control protocol ([`scoria_control`]), which the
//! app already serves for `--control`, but a Scoria someone started by
//! double-clicking a folder should not be driveable — synthetic
//! keystrokes, screenshots, file writes — by anything that happens to know
//! the path. So the app binds this one in a restricted scope
//! ([`scoria_control::INSTANCE_METHODS`]): the handover calls and nothing
//! else.
//!
//! Where the socket lives has to be a folder only this user can write to,
//! because connecting to it is enough to open and create notes:
//! `$XDG_RUNTIME_DIR` on Linux (the compositor's own, mode 0700),
//! `$TMPDIR` on macOS (per-user by design), the user's temp folder on
//! Windows (per-profile, and the socket inherits its ACL). A machine with
//! none of those gets no handover rather than a socket in shared `/tmp`.

use crate::args::Args;
use scoria_control::Client;
use scoria_vault::VaultRegistry;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What [`forward`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forward {
    /// A running Scoria took the arguments; this process is finished.
    Handled,
    /// Nothing was listening for this vault: carry on and open a window.
    NoInstance,
}

/// Long enough that the window is only ever waited on, not given up on,
/// and short enough that a wedged Scoria does not hold the terminal.
const CALL: Duration = Duration::from_secs(20);

/// How long the handover waits for a vault that is still indexing. The
/// app's own `open` refuses until the index lands.
const LOAD_MS: usize = 15_000;

/// The folder the per-vault sockets live in, or `None` when this machine
/// offers no per-user one.
pub fn socket_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SCORIA_INSTANCE_DIR") {
        return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
        Some(std::env::temp_dir().join("scoria"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("TMPDIR").map(|t| PathBuf::from(t).join("scoria"))
    } else {
        std::env::var_os("XDG_RUNTIME_DIR").map(|r| PathBuf::from(r).join("scoria"))
    }
}

/// The socket a Scoria showing `vault` listens on, or `None` when there is
/// nowhere to put one — or when the name would not fit in a socket address,
/// which holds 108 bytes and nothing the handover does is worth a run that
/// fails at `bind`.
pub fn socket_for(vault: &Path) -> Option<PathBuf> {
    let path = socket_dir()?.join(format!("{:016x}.sock", vault_hash(vault)));
    (path.as_os_str().len() <= 100).then_some(path)
}

/// [`socket_for`], with the folder made and kept to this user.
pub fn prepare(vault: &Path) -> Option<PathBuf> {
    let path = socket_for(vault)?;
    let dir = path.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Some(path)
}

/// The name a vault's socket is keyed on. Symbolic links are resolved, so
/// the same folder reached by two paths is one instance; what cannot be
/// canonicalized (a vault on a mount that has gone away) keeps the path as
/// given, which at worst means no handover.
fn vault_hash(vault: &Path) -> u64 {
    let path = std::fs::canonicalize(vault).unwrap_or_else(|_| vault.to_path_buf());
    let text = path.to_string_lossy();
    // Case-insensitive where the file system is.
    let text = if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.into_owned()
    };
    // FNV-1a, in-crate because the name has to be stable: a hasher whose
    // seed changed between runs would hide a running instance from the
    // next launch.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Hands this run's arguments to the Scoria already showing the vault.
///
/// `vault_hint` is the vault a URI named, if it named one, resolved through
/// the machine's list — the app refuses a URI meant for another vault, so
/// routing it by name is what makes `obsidian://open?vault=Rocks` reach the
/// window holding Rocks rather than whichever one was used last.
pub fn forward(args: &Args, vault_hint: Option<&str>) -> Result<Forward, String> {
    if !args.single_instance() {
        return Ok(Forward::NoInstance);
    }
    let Some(socket) = handover_socket(args, vault_hint) else {
        return Ok(Forward::NoInstance);
    };
    // One attempt: the instance is listening or it is not, and a socket
    // file left behind by a Scoria that is gone refuses the connection.
    let Ok(mut client) = Client::connect(&socket, Duration::ZERO) else {
        return Ok(Forward::NoInstance);
    };
    let describe = |error: String| format!("the Scoria already open did not take over: {error}");
    client
        .call_timeout("ping", json!({}), Duration::from_secs(5))
        .map_err(describe)?;

    let opening = args.uri.is_some() || args.open_path.is_some() || args.note.is_some();
    if opening {
        // A window that is still scanning its vault cannot open a note yet,
        // and this is the one call that may legitimately take a while.
        client
            .call_timeout("wait_loaded", json!({"timeout_ms": LOAD_MS}), CALL)
            .map_err(describe)?;
    }
    let mode = args.mode();
    if let Some(uri) = &args.uri {
        client
            .call_timeout("uri", json!({"uri": uri}), CALL)
            .map_err(describe)?;
    } else if let Some(path) = &args.open_path {
        client
            .call_timeout("open_path", params(&[("path", path)], mode), CALL)
            .map_err(describe)?;
    } else if let Some(note) = &args.note {
        let answer = client
            .call_timeout("open", params(&[("name", note)], mode), CALL)
            .map_err(describe)?;
        // `open` answers with a null current file for a name that matches
        // nothing, rather than failing: say so, since the window the person
        // is now looking at is showing something else.
        if answer.get("current").is_some_and(Value::is_null) {
            return Err(format!("no note in the vault is called {note}"));
        }
    } else if let Some(mode) = mode {
        client
            .call_timeout("set_mode", json!({"mode": mode}), CALL)
            .map_err(describe)?;
    }
    client
        .call_timeout("window", json!({"activate": true}), CALL)
        .map_err(describe)?;
    Ok(Forward::Handled)
}

/// The socket this run's arguments should go to: the vault's own, the one
/// the URI names, or — for a launch that named no vault at all, which is
/// what clicking the desktop entry does — the window used most recently.
fn handover_socket(args: &Args, vault_hint: Option<&str>) -> Option<PathBuf> {
    if let Some(vault) = args.vault.as_deref() {
        return socket_for(vault);
    }
    if let Some(name) = vault_hint {
        if let Some(vault) = vault_named(name) {
            return socket_for(&vault);
        }
    }
    live_sockets().into_iter().next()
}

/// The remembered vaults' sockets, most recently opened first, and only
/// those a Scoria has left a socket file for.
fn live_sockets() -> Vec<PathBuf> {
    let Ok(Some(registry)) = VaultRegistry::open() else {
        return Vec::new();
    };
    let mut vaults: Vec<(i64, PathBuf)> = registry
        .vaults()
        .iter()
        .map(|(_, entry)| (entry.ts, entry.path.clone()))
        .collect();
    vaults.sort_by_key(|(ts, _)| std::cmp::Reverse(*ts));
    vaults
        .into_iter()
        .filter_map(|(_, vault)| socket_for(&vault))
        .filter(|socket| socket.exists())
        .collect()
}

/// The vault a URI's `vault` parameter names: an id of the machine's list,
/// or a vault folder's name, which is what Obsidian's own URIs carry.
fn vault_named(name: &str) -> Option<PathBuf> {
    let registry = VaultRegistry::open().ok().flatten()?;
    registry
        .vaults()
        .iter()
        .find(|(id, entry)| *id == name || entry.name().eq_ignore_ascii_case(name))
        .map(|(_, entry)| entry.path.clone())
}

/// Request params, with `mode` left out rather than sent as null.
fn params(pairs: &[(&str, &str)], mode: Option<&str>) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_string(), json!(value));
    }
    if let Some(mode) = mode {
        map.insert("mode".into(), json!(mode));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse;
    use scoria_control::net::UnixListener;
    use scoria_control::{Request, Response};
    use std::io::{BufRead, BufReader, Write};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Mutex, MutexGuard};

    /// The socket directory and the configuration folder are environment
    /// variables, and a test process has one environment: these tests take
    /// turns rather than reading each other's.
    static ENV: Mutex<()> = Mutex::new(());

    /// A socket directory and vault list of this test's own, so nothing
    /// here reaches a Scoria the developer is running.
    struct Env {
        _guard: MutexGuard<'static, ()>,
        dir: tempfile::TempDir,
        config: tempfile::TempDir,
    }

    impl Env {
        fn new() -> Env {
            let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
            // Short names: a socket address holds 108 bytes, and the temp
            // folder plus a hashed name has to fit inside one.
            let dir = tempfile::Builder::new()
                .prefix("sc")
                .tempdir()
                .expect("a temp dir");
            let config = tempfile::Builder::new()
                .prefix("sc-cfg")
                .tempdir()
                .expect("a temp dir");
            std::env::set_var("SCORIA_INSTANCE_DIR", dir.path());
            std::env::set_var("SCORIA_CONFIG_DIR", config.path());
            std::env::remove_var("SCORIA_SINGLE_INSTANCE");
            Env {
                _guard: guard,
                dir,
                config,
            }
        }

        /// A vault folder, and the socket a Scoria showing it would bind.
        fn vault(&self, name: &str) -> (PathBuf, PathBuf) {
            let vault = self.dir.path().join(name);
            std::fs::create_dir_all(&vault).expect("a vault folder");
            let socket = prepare(&vault).expect("a socket path");
            (vault, socket)
        }

        /// Notes the vault on this machine's list, as opening it does.
        fn remember(&self, vault: &Path, ts: i64) {
            let mut registry =
                VaultRegistry::load(self.config.path().join(scoria_vault::registry::FILE_NAME))
                    .expect("a list");
            registry.record_open(vault, ts);
            registry.save().expect("writes the list");
        }
    }

    impl Drop for Env {
        fn drop(&mut self) {
            std::env::remove_var("SCORIA_INSTANCE_DIR");
            std::env::remove_var("SCORIA_CONFIG_DIR");
        }
    }

    /// Answers like the app's instance socket, recording what it was asked.
    fn listen(socket: &Path) -> Receiver<(String, Value)> {
        let listener = UnixListener::bind(socket).expect("binds");
        let (calls, seen) = channel();
        std::thread::spawn(move || serve(listener, calls));
        seen
    }

    fn serve(listener: UnixListener, calls: Sender<(String, Value)>) {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut writer = stream;
        for line in reader.lines().map_while(Result::ok) {
            let request: Request = serde_json::from_str(&line).expect("a request");
            let _ = calls.send((request.method.clone(), request.params.clone()));
            let result = match request.method.as_str() {
                "ping" => json!({"ok": true, "protocol": scoria_control::PROTOCOL}),
                "wait_loaded" => json!({"loaded": true}),
                // A name that matches nothing: what the app answers.
                "open" if request.params["name"] == json!("Nothing") => json!({"current": null}),
                "open" | "open_path" => json!({"current": "Rocks/Basalt.md"}),
                "window" => json!({"active": true}),
                _ => json!({"ok": true}),
            };
            let line = Response::ok(request.id, result).line();
            if writer.write_all(line.as_bytes()).is_err() || writer.flush().is_err() {
                return;
            }
        }
    }

    fn methods(seen: &Receiver<(String, Value)>) -> Vec<String> {
        seen.try_iter().map(|(name, _)| name).collect()
    }

    fn args_for(parts: &[&str]) -> Args {
        parse(parts.iter().copied()).expect("parses")
    }

    #[test]
    fn a_vault_keeps_one_socket_whatever_the_path_looks_like() {
        let env = Env::new();
        let (vault, socket) = env.vault("rocks");
        // The same folder spelled with a redundant `.` is one instance.
        assert_eq!(socket, socket_for(&vault.join(".")).expect("a path"));
        // A different folder is a different one.
        let (_, other) = env.vault("volcanoes");
        assert_ne!(socket, other);
        assert!(socket.starts_with(socket_dir().expect("a dir")));
        assert!(
            socket.extension().is_some_and(|e| e == "sock"),
            "{socket:?}"
        );
        // And the folder it lives in is this user's alone.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(socket.parent().expect("a parent"))
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "{mode:o}");
        }
    }

    #[test]
    fn nothing_listening_means_this_run_opens_the_window() {
        let env = Env::new();
        let (vault, socket) = env.vault("rocks");
        let args = args_for(&[&vault.to_string_lossy()]);
        assert_eq!(forward(&args, None), Ok(Forward::NoInstance));
        // A socket file left behind by a Scoria that is gone is not an
        // instance either: connecting to it is refused.
        std::fs::write(&socket, b"").expect("a stale file");
        assert_eq!(forward(&args, None), Ok(Forward::NoInstance));
    }

    #[test]
    fn a_driven_run_never_hands_over() {
        let env = Env::new();
        let (vault, socket) = env.vault("rocks");
        let seen = listen(&socket);
        for extra in [
            ["--new-window", ""],
            ["--control", "/tmp/sc-driven.sock"],
            ["--synthetic", "10"],
            ["--stress", "10"],
        ] {
            let mut parts = vec![vault.to_string_lossy().to_string()];
            parts.extend(
                extra
                    .iter()
                    .filter(|a| !a.is_empty())
                    .map(|a| a.to_string()),
            );
            let args = parse(parts.clone()).expect("parses");
            assert_eq!(
                forward(&args, None),
                Ok(Forward::NoInstance),
                "{parts:?} opens its own window"
            );
        }
        assert!(methods(&seen).is_empty(), "the instance was left alone");
    }

    #[test]
    fn a_file_argument_is_opened_in_the_running_window() {
        let env = Env::new();
        let (vault, socket) = env.vault("rocks");
        let seen = listen(&socket);

        let mut args = args_for(&[&vault.to_string_lossy(), "--source"]);
        args.open_path = Some("Rocks/Basalt.md".into());
        assert_eq!(forward(&args, None), Ok(Forward::Handled));

        let calls: Vec<(String, Value)> = seen.try_iter().collect();
        let names: Vec<&str> = calls.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["ping", "wait_loaded", "open_path", "window"]);
        assert_eq!(calls[2].1["path"], json!("Rocks/Basalt.md"));
        assert_eq!(calls[2].1["mode"], json!("source"));
        // Raising the window is the whole point of the handover.
        assert_eq!(calls[3].1["activate"], json!(true));
    }

    #[test]
    fn a_note_name_goes_over_as_open_and_a_miss_is_reported() {
        let env = Env::new();
        let (vault, socket) = env.vault("rocks");
        let seen = listen(&socket);
        let args = args_for(&[&vault.to_string_lossy(), "--note", "Basalt", "--reading"]);
        assert_eq!(forward(&args, None), Ok(Forward::Handled));
        let calls: Vec<(String, Value)> = seen.try_iter().collect();
        assert_eq!(calls[2].0, "open");
        assert_eq!(calls[2].1["name"], json!("Basalt"));
        assert_eq!(calls[2].1["mode"], json!("reading"));
        assert_eq!(calls.last().expect("a call").0, "window");

        // A note the vault has not got is reported rather than passed off as
        // opened: the window the person is looking at shows something else.
        let (other, other_socket) = env.vault("more-rocks");
        let seen = listen(&other_socket);
        let args = args_for(&[&other.to_string_lossy(), "--note", "Nothing"]);
        let e = forward(&args, None).expect_err("refused");
        assert_eq!(e, "no note in the vault is called Nothing");
        assert_eq!(methods(&seen), ["ping", "wait_loaded", "open"]);
    }

    #[test]
    fn a_uri_goes_over_as_a_uri() {
        let env = Env::new();
        let (vault, socket) = env.vault("rocks");
        let seen = listen(&socket);
        let args = args_for(&[&vault.to_string_lossy(), "obsidian://open?file=Basalt"]);
        assert_eq!(forward(&args, None), Ok(Forward::Handled));
        let calls: Vec<(String, Value)> = seen.try_iter().collect();
        assert_eq!(calls.len(), 4, "{calls:?}");
        assert_eq!(calls[2].0, "uri");
        assert_eq!(calls[2].1["uri"], json!("obsidian://open?file=Basalt"));
    }

    #[test]
    fn a_launch_with_no_vault_raises_the_window_used_last() {
        let env = Env::new();
        let (older, older_socket) = env.vault("older");
        let (newer, newer_socket) = env.vault("newer");
        env.remember(&older, 1_000);
        env.remember(&newer, 2_000);
        let older_seen = listen(&older_socket);
        let newer_seen = listen(&newer_socket);

        // What clicking the desktop entry does: no arguments at all.
        let args = args_for(&[]);
        assert_eq!(forward(&args, None), Ok(Forward::Handled));
        assert_eq!(methods(&newer_seen), ["ping", "window"]);
        assert!(methods(&older_seen).is_empty(), "the older vault is idle");

        // A URI naming a vault goes to that vault's window instead, by the
        // folder's name, which is what Obsidian's URIs carry.
        let args = args_for(&["obsidian://open?vault=older&file=Basalt"]);
        assert_eq!(forward(&args, Some("older")), Ok(Forward::Handled));
        assert_eq!(
            methods(&older_seen),
            ["ping", "wait_loaded", "uri", "window"]
        );
        assert!(methods(&newer_seen).is_empty(), "the newer vault is idle");
    }
}
