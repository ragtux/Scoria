//! Scoria application.
//!
//! `scoria <vault> [--note <path>]` opens a vault in the three-pane shell:
//! Files on the left, the reading view (or the source editor, Ctrl+E) in the
//! middle, and Backlinks, Outline and Tags on the right. A file argument
//! (`scoria Rocks/Basalt.md`) opens the vault that holds it and then the
//! note, and a `scoria://` or `obsidian://` argument is acted on once the
//! vault has loaded. `--synthetic N` and `--stress` keep the editor spike
//! from the first gate runnable; the other flags open the app in a given
//! state for screenshots and smoke tests, and `--control <socket>` lets a
//! test or the MCP server drive the running app instead (see
//! `docs/scoria/Development.md`).
//!
//! The command line itself lives in `scoria-cli`, which has no gpui in it:
//! the flag table and its help text, the file argument, and the handover
//! that makes a second `scoria` on an open vault raise that window instead
//! of starting a second one.

mod assets;
mod backlinks;
mod bases;
mod bookmarks;
mod canvas;
mod chooser;
mod control;
mod copy_results;
mod debug_info;
mod editor;
mod explorer;
mod file_menu;
mod fold;
mod graph;
mod help;
mod hover_preview;
mod inline_field;
mod math;
mod mentions;
mod palette;
mod panes;
mod pdf;
mod popout;
mod preview;
mod prompt;
mod properties;
mod reading;
mod remote;
mod search;
mod settings;
mod suggest;
mod switcher;
mod text_input;
mod theme;
mod title_field;
mod tree;
mod uri;
mod vaults;
mod watch;
mod workspace;

use gpui::{
    actions, point, prelude::*, px, size, App, Bounds, Focusable, KeyBinding, TitlebarOptions,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions,
};
use scoria_cli::{instance, Args, Forward, Target};
use scoria_commands::CommandRegistry;
use std::fmt::Write as _;
use std::sync::Arc;
use theme::Theme;
use workspace::{Commands, ViewMode, Workspace};

actions!(scoria, [Quit]);

/// A deterministic note of `lines` lines exercising headings, wikilinks,
/// tags, emphasis, code, lists, and callouts, for the 50k-line spike.
fn synthetic_note(lines: usize) -> String {
    const WORDS: &[&str] = &[
        "vault", "note", "link", "graph", "heading", "callout", "tag", "cursor", "scroll",
        "render", "parser", "cache", "index", "editor", "canvas", "search", "theme", "plugin",
    ];
    let mut out = String::with_capacity(lines * 48);
    let mut seed: u64 = 7;
    let mut pick = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        WORDS[(seed % WORDS.len() as u64) as usize]
    };
    out.push_str("---\ntitle: Synthetic\ntags: [spike]\n---\n");
    let mut n = 4;
    while n < lines {
        match n % 40 {
            0 => {
                let _ = writeln!(out, "\n## Section {} {}", n / 40, pick());
                n += 2;
            }
            7 => {
                let _ = writeln!(
                    out,
                    "> [!tip] {} {}\n> {} {} [[Note {}]]",
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    n
                );
                n += 2;
            }
            12..=15 => {
                let _ = writeln!(
                    out,
                    "- [ ] {} {} #{} [[Note {}|{}]]",
                    pick(),
                    pick(),
                    pick(),
                    n % 97,
                    pick()
                );
                n += 1;
            }
            20 => {
                let _ = writeln!(out, "```rust\nfn {}() {{ // [[not a link]] }}\n```", pick());
                n += 3;
            }
            _ => {
                let _ = writeln!(
                    out,
                    "{} {} **{}** {} [[Note {}#{}]] {} `{}` #{}/{} {} *{}* =={}== {}.",
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    n % 211,
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    pick(),
                    pick()
                );
                n += 1;
            }
        }
    }
    out
}

/// What the command line asked this process to do.
enum Startup {
    /// Open a window with these arguments.
    Run(Box<Args>),
    /// Nothing more to do: the help or the version was printed, or a Scoria
    /// that is already running took the arguments over.
    Done,
    /// Give up with this exit status, the message already printed.
    Failed(i32),
}

/// Everything the arguments decide before a window exists: the help text,
/// the version, what a path argument names, and whether this run belongs to
/// the Scoria that already has the vault open.
///
/// The exit statuses are the usual ones — 2 for a command line that could
/// not be used, 1 for a handover that was refused — so a script can tell a
/// typo from a vault that answered.
fn startup() -> Startup {
    let mut args = match scoria_cli::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("scoria: {error}");
            eprintln!("try `scoria --help`");
            return Startup::Failed(2);
        }
    };
    if args.help {
        print!("{}", scoria_cli::usage());
        return Startup::Done;
    }
    if args.version {
        println!("{}", scoria_cli::version());
        return Startup::Done;
    }
    // A folder is the vault; a file is a note (or a canvas, a base, a PDF)
    // in the vault above it, which is what a file manager hands over and
    // what shell completion produces.
    if let Some(given) = args.vault.clone() {
        match scoria_cli::resolve(&given) {
            Ok(Target::Vault(root)) => args.vault = Some(root),
            Ok(Target::File { vault, path }) => {
                args.vault = Some(vault);
                if args.note.is_none() && args.open_path.is_none() {
                    args.open_path = Some(path);
                }
            }
            Err(error) => {
                eprintln!("scoria: {error}");
                return Startup::Failed(2);
            }
        }
    }
    // `obsidian://open?vault=Rocks` is for the window showing Rocks, and
    // the app refuses a URI meant for another vault, so the vault the URI
    // names is what routes it.
    let named = args
        .uri
        .as_deref()
        .and_then(|uri| uri::parse(uri).ok())
        .and_then(|uri| uri.vault().map(str::to_string));
    match instance::forward(&args, named.as_deref()) {
        Ok(Forward::Handled) => return Startup::Done,
        Ok(Forward::NoInstance) => {}
        Err(error) => {
            eprintln!("scoria: {error}");
            return Startup::Failed(1);
        }
    }
    Startup::Run(Box::new(args))
}

/// Whether the window is opened without the desktop's own title bar, which
/// is what Obsidian's "Hidden" frame style means and its default on Linux
/// and Windows. Answered once: the window asks on the way up, and the
/// workspace asks again on every frame.
pub fn hidden_frame() -> bool {
    static HIDDEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HIDDEN.get_or_init(|| frame_style() != "native")
}

/// The "Window frame style" this process runs with: `--frame`'s value,
/// `SCORIA_FRAME`'s, the one stored in `scoria.json`, or `hidden`. Obsidian
/// keeps the choice beside its vault list rather than under any vault, so
/// the stored value is the registry's (`VaultRegistry::frame`); the flag
/// and the variable are a way to try the other style for one run without
/// changing what the next run gets.
fn frame_style() -> String {
    // Parsed again rather than threaded through: `hidden_frame` is asked
    // from inside a paint, where there is no `Args` to hand it, and the
    // answer is kept in a `OnceLock` either way.
    if let Some(frame) = scoria_cli::parse(std::env::args().skip(1))
        .ok()
        .and_then(|args| args.frame)
    {
        return frame;
    }
    if let Ok(v) = std::env::var("SCORIA_FRAME") {
        return v;
    }
    stored_frame_style()
}

/// The frame style `scoria.json` holds, or `hidden`: what the Settings
/// dropdown shows, since the file is what the next start reads. Read once
/// and kept in step by [`remember_frame_style`], so the Interface tab does
/// not open the file on every frame.
pub fn stored_frame_style() -> String {
    stored_frame()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Writes the frame style to `scoria.json` and remembers it. The window
/// keeps the frame it opened with; the next start reads the file.
pub fn write_frame_style(value: &str) -> Result<(), String> {
    let Some(mut registry) =
        scoria_vault::registry::VaultRegistry::open().map_err(|e| e.to_string())?
    else {
        return Err("no configuration folder to keep it in".into());
    };
    registry.set_frame((value != "hidden").then_some(value));
    registry.save().map_err(|e| e.to_string())?;
    *stored_frame().lock().unwrap_or_else(|e| e.into_inner()) = value.to_string();
    Ok(())
}

fn stored_frame() -> &'static std::sync::Mutex<String> {
    static STORED: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();
    STORED.get_or_init(|| {
        let stored = scoria_vault::registry::VaultRegistry::open()
            .ok()
            .flatten()
            .and_then(|r| r.frame().map(str::to_string))
            .unwrap_or_else(|| "hidden".into());
        std::sync::Mutex::new(stored)
    })
}

/// The families Settings → Appearance's font pages offer, and that a font
/// list is tested against. gpui's list adds its own fallback stack whether
/// the platform has those families or not, so each of them is kept only if
/// asking for it really lands on it; the display face Scoria bundles for
/// its wordmark (one weight, not a reading face) is left out.
fn installed_families(cx: &App) -> Vec<String> {
    const GPUI_FALLBACKS: &[&str] = &[
        "Helvetica",
        "Segoe UI",
        "Ubuntu",
        "Adwaita Sans",
        "Cantarell",
        "Noto Sans",
        "DejaVu Sans",
        "Arial",
    ];
    let text = cx.text_system().clone();
    theme::fonts::pickable(text.all_font_names(), |name| {
        if name == assets::DISPLAY_FONT_FAMILY {
            return true;
        }
        if !GPUI_FALLBACKS.contains(&name) {
            return false;
        }
        let id = text.resolve_font(&gpui::font(name.to_string()));
        !text
            .get_font_for_id(id)
            .is_some_and(|f| f.family.eq_ignore_ascii_case(name))
    })
}

/// The "Custom app icon" `scoria.json` records, for a window to carry.
/// Only X11 takes an icon from the app (gpui reads `WindowOptions::icon`
/// there alone); everywhere else the icon is the executable's or the
/// desktop entry's, so the file is not even read.
pub fn custom_window_icon() -> Option<Arc<image::RgbaImage>> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let path = scoria_vault::registry::VaultRegistry::open()
        .ok()
        .flatten()?
        .icon_path()?;
    Some(Arc::new(image::open(path).ok()?.into_rgba8()))
}

/// Every key binding in the app, from scratch: the views' own and the
/// registry's hotkeys. Called at startup and again whenever `hotkeys.json`
/// changes, since a binding made from the old registry would otherwise stay.
pub fn bind_all_keys(cx: &mut App, registry: &CommandRegistry) {
    cx.clear_key_bindings();
    editor::bind_keys(cx);
    workspace::bind_keys(cx, registry);
    text_input::bind_keys(cx);
    graph::bind_keys(cx);
    canvas::view::bind_keys(cx);
    pdf::view::bind_keys(cx);
    bases::toolbar::bind_keys(cx);
    explorer::bind_keys(cx);
    file_menu::bind_keys(cx);
    settings::bind_keys(cx);
    help::bind_keys(cx);
    vaults::bind_keys(cx);
    prompt::bind_keys(cx);
    copy_results::bind_keys(cx);
    debug_info::bind_keys(cx);
    cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
}

fn main() {
    let args = match startup() {
        Startup::Run(args) => args,
        Startup::Done => return,
        Startup::Failed(code) => std::process::exit(code),
    };
    // A run that opens a vault binds the socket named after it, so the next
    // `scoria` on the same vault finds this window. Taken before gpui
    // starts, since binding happens inside the app.
    let handover = args
        .single_instance()
        .then(|| args.vault.as_deref().and_then(instance::prepare))
        .flatten();
    // The theme comes from appearance.json and the key bindings from
    // hotkeys.json; read both before the window opens so the first frame
    // already has the right colors and keys.
    let vault = args
        .vault
        .as_ref()
        .and_then(|root| scoria_vault::Vault::open(root).ok());
    // The window is what knows the desktop's colour scheme, and it does not
    // exist yet; dark is assumed here and the theme is rebuilt below once
    // the window can answer, which only matters for `theme: "system"`.
    let appearance = vault.as_ref().and_then(|v| v.appearance_config().ok());
    let theme = match (vault.as_ref(), appearance.as_ref()) {
        (Some(v), Some(a)) => Theme::load(v.root(), v.config_dir(), a, true),
        _ => Theme::dark(),
    };
    let mut registry = CommandRegistry::with_known_ids();
    // A core plugin that is off has no commands: not in the palette, and
    // nothing bound to its keys. `core-plugins.json` is read here so the
    // first frame is already right, and `change_setting` rebuilds this the
    // same way when a toggle changes it.
    if let Some(vault) = vault.as_ref() {
        let core = vault.core_plugins().unwrap_or_default();
        registry.retain_enabled(|id| {
            scoria_formats::config::CORE_PLUGINS
                .iter()
                .find(|p| p.id == id)
                .is_none_or(|p| {
                    scoria_formats::config::CorePlugins::is_enabled_or_default(core.as_ref(), p)
                })
        });
    }
    if let Some(hotkeys) = vault.as_ref().and_then(|v| v.hotkeys().ok()) {
        registry.apply_overrides(&hotkeys);
    }
    drop(vault);
    let registry = Arc::new(registry);

    // Obsidian's "Window frame style" is "Hidden" on Linux and Windows: no
    // title bar of the desktop's, the tab strip at the window's top edge and
    // the window buttons in its right corner. `--frame native` asks the
    // desktop for its own frame instead, which is the choice for a
    // compositor that draws client windows badly.
    let frame = args.frame.clone().unwrap_or_else(frame_style);
    let decorations = if frame == "native" {
        WindowDecorations::Server
    } else {
        WindowDecorations::Client
    };

    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            cx.set_global(theme);
            // Remote images (`![alt](https://...)`) load through gpui's `img()`,
            // which fetches with the app's HTTP client; the default refuses all.
            cx.set_http_client(std::sync::Arc::new(remote::RemoteClient::new()));
            if let Err(e) = cx.text_system().add_fonts(assets::fonts()) {
                eprintln!("could not register the bundled fonts: {e}");
            }
            theme::fonts::set_installed(installed_families(cx));
            bind_all_keys(cx, &registry);
            cx.set_global(Commands(registry));

            let (width, height) = args.size.unwrap_or((1400.0, 900.0));
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let vault = args.vault.clone();
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Scoria".into()),
                            // What hides the caption on Windows and macOS,
                            // where `window_decorations` is not read at all:
                            // the window keeps the system's shadow and resize
                            // border and loses only the title bar, which is
                            // the frame Obsidian draws there. macOS keeps its
                            // traffic lights, at the point Obsidian's window
                            // options put them; the workspace pads the header
                            // under them (see `frame_left_space`).
                            appears_transparent: (cfg!(target_os = "windows")
                                || cfg!(target_os = "macos"))
                                && frame != "native",
                            traffic_light_position: (cfg!(target_os = "macos")
                                && frame != "native")
                                .then(|| point(px(19.), px(12.))),
                        }),
                        // The see-through title bar would otherwise drag
                        // the window from a press on a tab; see
                        // `frame_window_options`.
                        is_movable: !(cfg!(target_os = "macos") && frame != "native"),
                        // KWin only honours the xdg-activation token that
                        // `activate_window` asks for when it can tie it to an
                        // application, which is what this id is for.
                        app_id: Some("scoria".into()),
                        window_decorations: Some(decorations),
                        // A client-drawn frame paints its shadow in a margin
                        // around the content, which has to be see-through.
                        // Only Linux draws that margin: Windows keeps the
                        // system's shadow outside the window, so a
                        // transparent background there would only show the
                        // desktop through the app's own rounded corners.
                        window_background: match decorations {
                            WindowDecorations::Client if !cfg!(target_os = "windows") => {
                                WindowBackgroundAppearance::Transparent
                            }
                            _ => WindowBackgroundAppearance::Opaque,
                        },
                        window_min_size: Some(size(px(480.), px(320.))),
                        icon: custom_window_icon(),
                        ..Default::default()
                    },
                    |window, cx| cx.new(|cx| Workspace::new(vault, window, cx)),
                )
                .expect("open main window");
            // Now there is a window to ask: `theme: "system"` follows the
            // desktop, which is Obsidian's default and what a vault with no
            // theme key means.
            if let Some(a) = appearance {
                let follows_system =
                    !matches!(a.theme.as_deref(), Some("moonstone") | Some("obsidian"));
                if follows_system {
                    let dark = window
                        .update(cx, |_, window, _| {
                            matches!(
                                window.appearance(),
                                WindowAppearance::Dark | WindowAppearance::VibrantDark
                            )
                        })
                        .unwrap_or(true);
                    window
                        .update(cx, |ws, _, cx| ws.set_system_dark(dark, cx))
                        .ok();
                }
            }
            cx.on_action(move |_: &Quit, cx| {
                // Save the note and the layout before the window goes away.
                window.update(cx, |ws, _, cx| ws.on_quit(cx)).ok();
                control::cleanup();
                cx.quit();
            });
            // Ctrl+Q is not the only way out: closing the window from the
            // titlebar quits too, and a socket file left behind makes the next
            // run look like one that is already listening.
            cx.on_app_quit(|_| {
                control::cleanup();
                async {}
            })
            .detach();

            // Bound before the startup flags are applied, so `--control`
            // composes with `--note`, `--source` and the rest of them.
            if let Some(path) = args.control.clone() {
                control::start(path, control::Scope::Full, window, cx);
            }
            if let Some(path) = handover.clone() {
                control::start(path, control::Scope::Instance, window, cx);
            }

            let note = args.note.clone();
            let open_path = args.open_path.clone();
            let search = args.search.clone();
            let switcher = args.switcher.clone();
            let pane = args.pane.clone();
            let graph = args.graph.clone();
            let palette = args.palette.clone();
            let settings_tab = args.settings.clone();
            let source = args.source;
            let type_text = args.type_text.clone();
            let accept = args.accept.clone();
            let uri = args.uri.clone();
            let cell = args.cell;
            let command = args.command.clone();
            let template = args.template.clone();
            let menu = args.menu.clone();
            let reading = args.reading;
            let find = args.find.clone();
            let find_all = args.find_all.clone();
            let rename = args.rename.clone();
            let prompt = args.prompt.clone();
            let synthetic = args.synthetic;
            window
                .update(cx, |ws, window, cx| {
                    if let Some(lines) = synthetic {
                        let text = synthetic_note(lines);
                        ws.editor().update(cx, |editor, cx| {
                            editor.set_text(format!("synthetic-{lines}.md"), text, cx)
                        });
                        ws.set_mode(ViewMode::Source, window, cx);
                        // The flag loop below never runs for a synthetic note, so
                        // this is as arranged as the window is going to get.
                        ws.mark_started();
                    } else {
                        window.focus(&ws.focus_handle(cx), cx);
                    }
                })
                .expect("window update");
            cx.activate(true);

            // The vault indexes in the background; open the requested note once it lands.
            if synthetic.is_none() {
                cx.spawn(async move |cx| loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    let done = window
                        .update(cx, |ws, window, cx| {
                            if !ws.is_loaded() {
                                return false;
                            }
                            // The vault's default view mode, unless a flag says
                            // otherwise — or `workspace.json` restored the tabs,
                            // each in the mode it recorded.
                            let mode = if source {
                                Some(ViewMode::Source)
                            } else if reading {
                                Some(ViewMode::Reading)
                            } else if ws.layout_restored() {
                                None
                            } else {
                                Some(ws.default_mode())
                            };
                            if let Some(mode) = mode {
                                ws.set_mode(mode, window, cx);
                            }
                            // With a layout restored and no note asked for, the
                            // window stays as `workspace.json` left it, which is
                            // the empty state when nothing was open.
                            let opened = match (&open_path, &note) {
                                // A file argument names the file itself, so
                                // it is opened by path rather than resolved
                                // as a link: a `.canvas`, `.base` or `.pdf`
                                // is no link target, and two notes of the
                                // same name in different folders are
                                // distinct files.
                                (Some(path), _) => match ws.vault_file_path(path) {
                                    Some(key) => {
                                        ws.open_file(key, None, cx);
                                        true
                                    }
                                    None => {
                                        eprintln!("scoria: no such file in the vault: {path}");
                                        false
                                    }
                                },
                                (None, Some(n)) => ws.open_by_name(n, cx),
                                (None, None) => false,
                            };
                            if !opened && !ws.layout_restored() {
                                ws.open_first(cx);
                            }
                            if let Some(q) = &search {
                                ws.show_search(q, Some(window), cx);
                            }
                            if let Some(q) = &switcher {
                                ws.open_switcher(q, window, cx);
                            }
                            if let Some(p) = &pane {
                                ws.show_right_tab(p, cx);
                            }
                            if let Some(g) = &graph {
                                ws.show_graph(g == "local", window, cx);
                            }
                            if let Some(q) = &palette {
                                ws.open_palette(q, window, cx);
                            }
                            if let Some(t) = &settings_tab {
                                ws.open_settings(settings::Tab::parse(t), window, cx);
                            }
                            if let Some(q) = &find {
                                ws.open_find(q, false, window, cx);
                            }
                            if let Some(q) = &find_all {
                                ws.open_find(q, false, window, cx);
                                ws.editor()
                                    .update(cx, |e, cx| e.find_all_matches(window, cx));
                            }
                            if let Some(name) = &rename {
                                ws.rename_current(name, window, cx);
                            }
                            if let Some(kind) = &prompt {
                                ws.open_file_prompt(kind, window, cx);
                            }
                            if let Some(ix) = cell {
                                ws.editor().update(cx, |e, cx| e.edit_cell(ix, window, cx));
                            }
                            if let Some(t) = &type_text {
                                ws.editor().update(cx, |e, cx| e.type_text(t, window, cx));
                            }
                            if let Some(keys) = &accept {
                                ws.editor()
                                    .update(cx, |e, cx| e.press_keys(keys, window, cx));
                            }
                            if let Some(ids) = &command {
                                for id in ids.split(',') {
                                    ws.run_command(id.trim(), window, cx);
                                }
                            }
                            if let Some(q) = &template {
                                ws.pick_template(q, window, cx);
                            }
                            if let Some(key) = &menu {
                                ws.open_settings_menu(key, cx);
                            }
                            if let Some(u) = &uri {
                                if let Err(err) = ws.handle_uri(u, window, cx) {
                                    eprintln!("{err}");
                                }
                            }
                            ws.mark_started();
                            true
                        })
                        .unwrap_or(true);
                    if done {
                        break;
                    }
                })
                .detach();
            }

            let stress = match args.stress {
                Some(_) if args.control.is_some() => {
                    eprintln!("--stress is ignored with --control: the socket drives the app");
                    None
                }
                steps => steps,
            };
            if let Some(steps) = stress {
                cx.spawn(async move |cx| {
                    // Without `--synthetic` the run edits the opened note (and
                    // saves it): wait for the vault to index and the note to open.
                    while synthetic.is_none() {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(50))
                            .await;
                        let ready = window
                            .update(cx, |ws, window, cx| {
                                let open = ws.is_loaded() && ws.current().is_some();
                                if open && !ws.mode().is_editing() {
                                    ws.set_mode(ViewMode::Source, window, cx);
                                }
                                open
                            })
                            .unwrap_or(true);
                        if ready {
                            break;
                        }
                    }
                    let started = std::time::Instant::now();
                    for step in 0..steps {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(8))
                            .await;
                        let _ = window.update(cx, |ws, window, cx| {
                            ws.editor()
                                .update(cx, |editor, cx| editor.stress_step(step, window, cx));
                        });
                    }
                    // Let the last parse land (and, on a real note, the autosave
                    // fire) before reporting.
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(if synthetic.is_some() {
                            1500
                        } else {
                            3000
                        }))
                        .await;
                    let report = window
                        .update(cx, |ws, _, cx| ws.editor().read(cx).report())
                        .unwrap_or_default();
                    eprintln!(
                        "stress: {steps} steps in {:.1} s\n{report}",
                        started.elapsed().as_secs_f64()
                    );
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
}
