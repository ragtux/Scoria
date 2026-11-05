//! The flags, written down once.
//!
//! [`FLAGS`] is the one place a flag, whether it takes a value and its help
//! text are recorded: [`parse`] looks a flag up there to decide what to do
//! with the argument after it, and [`usage`] prints the same table, so a
//! flag cannot work and be undocumented, or be documented and silently
//! ignored. [`Args`] is what the app reads afterwards.
//!
//! The parser is strict, which the old one was not: an unknown flag, a
//! missing value and a third positional argument are errors with a message
//! naming the flag, rather than a note name set to `--surce`. What it keeps
//! is the shape — `scoria <vault> [note]`, a bare `obsidian://` argument,
//! and the flags that open the app in a given state for a screenshot.

use std::path::PathBuf;

/// One flag of the command line.
pub struct Flag {
    /// How it is written, `--` included.
    pub name: &'static str,
    /// A second spelling that means the same flag.
    pub alias: Option<&'static str>,
    /// What its value is called in the help text; `None` makes it a switch.
    pub value: Option<&'static str>,
    /// The value a flag that was given bare takes. `Some` makes the value
    /// optional: the next argument is only taken as the value when it does
    /// not look like a flag, so `--graph --source` is two flags.
    pub default: Option<&'static str>,
    /// The heading it is listed under.
    pub section: &'static str,
    /// One line, as the help text prints it.
    pub help: &'static str,
}

const fn f(
    name: &'static str,
    value: Option<&'static str>,
    default: Option<&'static str>,
    section: &'static str,
    help: &'static str,
) -> Flag {
    Flag {
        name,
        alias: None,
        value,
        default,
        section,
        help,
    }
}

const fn alias(mut flag: Flag, alias: &'static str) -> Flag {
    flag.alias = Some(alias);
    flag
}

const OPENING: &str = "Opening";
const WINDOW: &str = "The window";
const STATE: &str = "Opening in a state, for screenshots and smoke tests";
const DRIVING: &str = "Driving the app, and the spikes";
const ABOUT: &str = "About";

/// Every flag, in the order the help text lists them.
pub const FLAGS: &[Flag] = &[
    f(
        "--note",
        Some("<name>"),
        None,
        OPENING,
        "Open this note once the vault has indexed: a link target, so `Name`, `Name#Heading` or `Name#^block-id`",
    ),
    f(
        "--source",
        None,
        None,
        OPENING,
        "Open it in Source mode, whatever `defaultViewMode` says",
    ),
    f(
        "--reading",
        None,
        None,
        OPENING,
        "Open it in Reading view, whatever `defaultViewMode` says",
    ),
    f(
        "--uri",
        Some("<uri>"),
        None,
        OPENING,
        "Act on an `obsidian://` (or `scoria://`) URI once the vault has loaded; a bare argument of that scheme does the same",
    ),
    f(
        "--new-window",
        None,
        None,
        OPENING,
        "Start another Scoria instead of handing the arguments to the one already showing this vault",
    ),
    f(
        "--size",
        Some("<w>x<h>"),
        None,
        WINDOW,
        "Window size in logical pixels, `--size 1400x900`",
    ),
    f(
        "--frame",
        Some("<style>"),
        None,
        WINDOW,
        "`hidden` (the default, Obsidian's on Linux and Windows) or `native`, the desktop's own title bar",
    ),
    f(
        "--search",
        Some("<query>"),
        None,
        STATE,
        "Open the Search pane with this query",
    ),
    f(
        "--switcher",
        Some("<query>"),
        None,
        STATE,
        "Open the quick switcher with this query",
    ),
    f(
        "--pane",
        Some("<tab>"),
        None,
        STATE,
        "Show this right-dock tab: `backlinks`, `outgoing`, `tags`, `properties`, `outline`",
    ),
    f(
        "--graph",
        Some("<kind>"),
        Some("global"),
        STATE,
        "Open the graph view: `global` or `local`",
    ),
    f(
        "--palette",
        Some("<query>"),
        Some(""),
        STATE,
        "Open the command palette with this query",
    ),
    f(
        "--settings",
        Some("<tab>"),
        Some("general"),
        STATE,
        "Open Settings on this tab: `general`, `appearance`, `hotkeys`, ...",
    ),
    f(
        "--menu",
        Some("<key>"),
        None,
        STATE,
        "Open the Settings dropdown that writes this config key",
    ),
    f(
        "--find",
        Some("<query>"),
        Some(""),
        STATE,
        "Open the editor's find panel with this query",
    ),
    f(
        "--find-all",
        Some("<query>"),
        Some(""),
        STATE,
        "The same, and select every match (Find all)",
    ),
    f(
        "--rename",
        Some("<name>"),
        None,
        STATE,
        "Rename the opened note to this, rewriting the links to it",
    ),
    f(
        "--prompt",
        Some("<kind>"),
        Some("rename"),
        STATE,
        "Open a file prompt for the opened note: `rename`, `move`, `delete`, `folder`",
    ),
    f(
        "--cell",
        Some("<index>"),
        None,
        STATE,
        "Open this cell of the note's first table for editing, the way clicking one does",
    ),
    f(
        "--template",
        Some("<query>"),
        None,
        STATE,
        "Open the template chooser and pick the template this matches",
    ),
    f(
        "--type",
        Some("<text>"),
        None,
        STATE,
        "Type this text at the cursor in the source editor, which is how the autocompletion popup is opened for a screenshot",
    ),
    alias(
        f(
            "--accept",
            Some("<keys>"),
            Some("enter"),
            STATE,
            "Press these keys after `--type`, comma separated: `enter`, `tab`, `shift-tab`, `shift-enter`, `escape`, `up`, `down`",
        ),
        "--press",
    ),
    f(
        "--command",
        Some("<ids>"),
        None,
        STATE,
        "Run these command ids at startup, comma separated, after everything else",
    ),
    f(
        "--control",
        Some("<socket>"),
        None,
        DRIVING,
        "Bind the full control socket at this path, so a test or `scoria-mcp` can read the window and drive it",
    ),
    f(
        "--synthetic",
        Some("<lines>"),
        Some("50000"),
        DRIVING,
        "Open a generated note of this many lines instead of a vault file, for the editor spike",
    ),
    f(
        "--stress",
        Some("<steps>"),
        Some("600"),
        DRIVING,
        "Run this many self-driven edits, print the timings and quit",
    ),
    alias(
        f("--help", None, None, ABOUT, "Print this and exit"),
        "-h",
    ),
    alias(
        f(
            "--version",
            None,
            None,
            ABOUT,
            "Print the version and exit",
        ),
        "-V",
    ),
];

/// What the command line asked for.
// No `Eq`: `--size` is a pair of floats.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Args {
    /// The first positional argument: a vault folder, or a file inside one
    /// ([`crate::resolve`] turns the second into the first plus [`Args::open_path`]).
    pub vault: Option<PathBuf>,
    pub note: Option<String>,
    /// A vault-relative path to open, which is what a file argument becomes.
    pub open_path: Option<String>,
    pub synthetic: Option<usize>,
    /// Number of self-driven edit and navigation steps to run before quitting.
    pub stress: Option<usize>,
    /// Pre-filled Search query, for screenshots.
    pub search: Option<String>,
    /// Open the quick switcher with this query, for screenshots.
    pub switcher: Option<String>,
    /// Right-dock tab to show: backlinks, outgoing, tags, properties, outline.
    pub pane: Option<String>,
    /// Open the graph view: `global` or `local`.
    pub graph: Option<String>,
    /// Open the command palette with this query, for screenshots.
    pub palette: Option<String>,
    /// Open Settings on this tab (`hotkeys`, `appearance`, ...).
    pub settings: Option<String>,
    /// Open the note in source mode.
    pub source: bool,
    /// Open the note in reading view, whatever `defaultViewMode` says.
    pub reading: bool,
    /// Open the find panel with this query.
    pub find: Option<String>,
    /// Open the find panel with this query and select every match (Find all).
    pub find_all: Option<String>,
    /// Rename the opened note to this name at startup (links rewritten).
    pub rename: Option<String>,
    /// Open a file prompt for the opened note: rename, move, delete, folder.
    pub prompt: Option<String>,
    /// Type this text at the cursor in the source editor, which is how the
    /// autocompletion popup is opened for a screenshot (`--type "[[al"`).
    pub type_text: Option<String>,
    /// Press these keys, comma separated, after `--type`.
    pub accept: Option<String>,
    /// An `obsidian://` URI to act on once the vault has loaded.
    pub uri: Option<String>,
    /// Open this cell of the note's first table for editing, before
    /// `--type`, the way clicking one does.
    pub cell: Option<usize>,
    /// Run these command ids at startup, comma separated, after everything else.
    pub command: Option<String>,
    /// Open the template chooser and pick the template this matches.
    pub template: Option<String>,
    /// Open the Settings dropdown that writes this key, for screenshots.
    pub menu: Option<String>,
    /// Bind the control socket at this path, so something outside the app
    /// can read its state and drive it.
    pub control: Option<PathBuf>,
    /// Window size in logical pixels, `--size 1400x900`.
    pub size: Option<(f32, f32)>,
    /// `hidden` (the default, Obsidian's on Linux) or `native`.
    pub frame: Option<String>,
    /// Start a second Scoria rather than handing these arguments to the one
    /// already showing the vault.
    pub new_window: bool,
    pub help: bool,
    pub version: bool,
}

impl Args {
    /// Whether this run takes part in the one-window-per-vault handover at
    /// all — as a client, and as the instance a later run finds.
    ///
    /// A run with `--control` does not: it is being driven from outside, and
    /// the thing driving it spawned this process to own that socket, so
    /// handing its arguments to an earlier Scoria would leave the caller
    /// waiting on a window that never appears. Neither does a spike, which
    /// edits a generated note and quits, nor a run told `--new-window`, nor
    /// one in an environment that set `SCORIA_SINGLE_INSTANCE=0`.
    pub fn single_instance(&self) -> bool {
        !self.new_window
            && self.control.is_none()
            && self.synthetic.is_none()
            && self.stress.is_none()
            && !matches!(
                std::env::var("SCORIA_SINGLE_INSTANCE").as_deref(),
                Ok("0") | Ok("no") | Ok("false")
            )
    }

    /// The view mode the flags ask for, as the control protocol names it.
    pub fn mode(&self) -> Option<&'static str> {
        match (self.source, self.reading) {
            (true, _) => Some("source"),
            (_, true) => Some("reading"),
            _ => None,
        }
    }
}

/// The flag `arg` names, by either spelling.
pub fn flag(arg: &str) -> Option<&'static Flag> {
    FLAGS.iter().find(|f| f.name == arg || f.alias == Some(arg))
}

/// Parses a command line, without the program name.
pub fn parse<I, S>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut out = Args::default();
    let mut it = args.into_iter().map(Into::into).peekable();
    while let Some(arg) = it.next() {
        // A URI is recognized by its scheme wherever it stands, because
        // that is how a desktop hands one over: the entry's `Exec` line is
        // `scoria %u`, and %u is substituted with the URL and nothing else.
        if is_uri(&arg) {
            out.uri = Some(arg);
            continue;
        }
        // `--flag=value`, which a desktop entry and a shell script both
        // tend to write.
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with('-') => {
                (name.to_string(), Some(value.to_string()))
            }
            _ => (arg.clone(), None),
        };
        if let Some(spec) = flag(&name) {
            let value = match (spec.value, inline, spec.default) {
                (None, Some(_), _) => {
                    return Err(format!("{} takes no value", spec.name));
                }
                (None, None, _) => None,
                (Some(_), Some(value), _) => Some(value),
                // A required value is the next argument, whatever it looks
                // like: `--type ---` types three hyphens.
                (Some(metavar), None, None) => Some(
                    it.next()
                        .ok_or_else(|| format!("{} needs {metavar}", spec.name))?,
                ),
                // An optional one is the next argument only when that is
                // not itself a flag.
                (Some(_), None, Some(default)) => match it.peek() {
                    Some(next) if !next.starts_with('-') || next == "-" => it.next(),
                    _ => Some(default.to_string()),
                },
            };
            apply(&mut out, spec, value)?;
            continue;
        }
        if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option: {arg}"));
        }
        if out.vault.is_none() {
            out.vault = Some(PathBuf::from(arg));
        } else if out.note.is_none() {
            out.note = Some(arg);
        } else {
            return Err(format!("unexpected argument: {arg}"));
        }
    }
    Ok(out)
}

/// Whether `arg` is a URI Scoria acts on. `scoria://` is the same scheme
/// under Scoria's own name, for a desktop that has Obsidian installed and
/// so cannot give `obsidian://` away.
pub fn is_uri(arg: &str) -> bool {
    arg.starts_with("obsidian://") || arg.starts_with("scoria://")
}

/// Puts one parsed flag into [`Args`]. Every entry of [`FLAGS`] is handled
/// here; `the_table_and_the_parser_agree` is what keeps the two in step.
fn apply(args: &mut Args, spec: &Flag, value: Option<String>) -> Result<(), String> {
    let text = value.unwrap_or_default();
    match spec.name {
        "--note" => args.note = Some(text),
        "--source" => args.source = true,
        "--reading" => args.reading = true,
        "--uri" => {
            if !is_uri(&text) {
                return Err(format!("--uri wants an obsidian:// URI, not {text}"));
            }
            args.uri = Some(text);
        }
        "--new-window" => args.new_window = true,
        "--size" => {
            args.size = Some(
                parse_size(&text)
                    .ok_or_else(|| format!("--size wants <width>x<height>, not {text}"))?,
            )
        }
        "--frame" => args.frame = Some(text),
        "--search" => args.search = Some(text),
        "--switcher" => args.switcher = Some(text),
        "--pane" => args.pane = Some(text),
        "--graph" => args.graph = Some(text),
        "--palette" => args.palette = Some(text),
        "--settings" => args.settings = Some(text),
        "--menu" => args.menu = Some(text),
        "--find" => args.find = Some(text),
        "--find-all" => args.find_all = Some(text),
        "--rename" => args.rename = Some(text),
        "--prompt" => args.prompt = Some(text),
        "--cell" => args.cell = Some(number(spec.name, &text)?),
        "--template" => args.template = Some(text),
        "--type" => args.type_text = Some(text),
        "--accept" => args.accept = Some(text),
        "--command" => args.command = Some(text),
        "--control" => args.control = Some(PathBuf::from(text)),
        "--synthetic" => args.synthetic = Some(number(spec.name, &text)?),
        "--stress" => args.stress = Some(number(spec.name, &text)?),
        "--help" => args.help = true,
        "--version" => args.version = true,
        other => {
            return Err(format!(
                "{other} is in the flag table but the parser ignores it"
            ))
        }
    }
    Ok(())
}

fn number(flag: &str, text: &str) -> Result<usize, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{flag} wants a number, not {text}"))
}

/// `1400x900` as a width and a height.
pub fn parse_size(value: &str) -> Option<(f32, f32)> {
    let (width, height) = value.split_once(['x', 'X'])?;
    let (width, height): (f32, f32) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    (width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0)
        .then_some((width, height))
}

/// `scoria 0.4.0`, which is what `--version` prints.
pub fn version() -> String {
    format!("scoria {}", env!("CARGO_PKG_VERSION"))
}

/// The help text, written from [`FLAGS`].
pub fn usage() -> String {
    // Wide enough for the longest flag with its metavar, so the help
    // column lines up; `--find-all <query>` is the one that sets it.
    let width = FLAGS
        .iter()
        .map(|f| f.name.len() + f.value.map_or(0, |v| v.len() + 1))
        .max()
        .unwrap_or(16);
    let mut out = format!(
        "{} — a native knowledge base on Obsidian's own vault format\n\
         \n\
         Usage:\n\
         \u{20} scoria [options] [<vault>] [<note>]\n\
         \u{20} scoria [options] <file>              a note, canvas, base or PDF; its vault is the\n\
         \u{20}                                      nearest folder above it holding .obsidian\n\
         \u{20} scoria [options] obsidian://open?file=Note\n\
         \n\
         With no vault Scoria opens the vault manager, which is also where \"Open folder as vault\"\n\
         is. A second run on a vault that is already open hands it the note, file or URI and raises\n\
         its window instead of starting again; --new-window opts out.\n",
        version()
    );
    let mut section = "";
    for flag in FLAGS {
        if flag.section != section {
            section = flag.section;
            out.push_str(&format!("\n{section}:\n"));
        }
        let name = match flag.value {
            Some(metavar) => format!("{} {metavar}", flag.name),
            None => flag.name.to_string(),
        };
        out.push_str(&format!("  {name:width$}  {}\n", flag.help));
        if let Some(alias) = flag.alias {
            out.push_str(&format!("  {alias:width$}  The same flag\n"));
        }
    }
    out.push_str(
        "\nEnvironment:\n\
         \u{20} SCORIA_FRAME              Like --frame, for a whole session\n\
         \u{20} SCORIA_SINGLE_INSTANCE=0  Like --new-window, for a whole session\n\
         \u{20} SCORIA_CONFIG_DIR         Where scoria.json (the vault list) lives\n\
         \u{20} SCORIA_INSTANCE_DIR       Where the per-vault handover sockets live\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &[&str]) -> Args {
        parse(line.iter().copied()).expect("parses")
    }

    #[test]
    fn a_vault_and_a_note_are_positional() {
        let a = args(&["/vaults/rocks", "Basalt"]);
        assert_eq!(a.vault, Some(PathBuf::from("/vaults/rocks")));
        assert_eq!(a.note.as_deref(), Some("Basalt"));
        // And the note has a flag of its own, which is what the tools use.
        let a = args(&["/vaults/rocks", "--note", "Rocks/Basalt#Texture"]);
        assert_eq!(a.note.as_deref(), Some("Rocks/Basalt#Texture"));
        // A third positional is a mistake, not a second note.
        let e = parse(["/v", "a", "b"]).expect_err("refused");
        assert_eq!(e, "unexpected argument: b");
    }

    #[test]
    fn a_uri_is_recognized_by_its_scheme_anywhere() {
        for line in [
            vec!["obsidian://open?file=Basalt"],
            vec!["--source", "obsidian://open?file=Basalt"],
            vec!["--uri", "obsidian://open?file=Basalt"],
        ] {
            assert_eq!(
                args(&line).uri.as_deref(),
                Some("obsidian://open?file=Basalt"),
                "{line:?}"
            );
            assert_eq!(args(&line).vault, None, "a URI is not a vault path");
        }
        // Scoria's own scheme, for a desktop where Obsidian owns the other.
        assert_eq!(
            args(&["scoria://daily"]).uri.as_deref(),
            Some("scoria://daily")
        );
        let e = parse(["--uri", "https://obsidian.md"]).expect_err("refused");
        assert!(e.contains("obsidian:// URI"), "{e}");
    }

    #[test]
    fn an_unknown_flag_is_an_error_rather_than_a_note() {
        // The old parser made `--surce` the note name and opened nothing.
        let e = parse(["/v", "--surce"]).expect_err("refused");
        assert_eq!(e, "unknown option: --surce");
        let e = parse(["--note"]).expect_err("refused");
        assert_eq!(e, "--note needs <name>");
        let e = parse(["--source=yes"]).expect_err("refused");
        assert_eq!(e, "--source takes no value");
        let e = parse(["--cell", "last"]).expect_err("refused");
        assert_eq!(e, "--cell wants a number, not last");
        let e = parse(["--size", "big"]).expect_err("refused");
        assert!(e.contains("<width>x<height>"), "{e}");
    }

    #[test]
    fn a_value_may_be_attached_with_an_equals_sign() {
        let a = args(&["--note=Basalt", "--size=800x600", "--graph=local"]);
        assert_eq!(a.note.as_deref(), Some("Basalt"));
        assert_eq!(a.size, Some((800.0, 600.0)));
        assert_eq!(a.graph.as_deref(), Some("local"));
    }

    #[test]
    fn an_optional_value_is_not_the_next_flag() {
        // `--graph --source` is two flags; the old parser ate the second.
        let a = args(&["--graph", "--source"]);
        assert_eq!(a.graph.as_deref(), Some("global"));
        assert!(a.source);
        let a = args(&["--palette", "--settings"]);
        assert_eq!(a.palette.as_deref(), Some(""));
        assert_eq!(a.settings.as_deref(), Some("general"));
        let a = args(&["--synthetic", "--stress"]);
        assert_eq!(a.synthetic, Some(50_000));
        assert_eq!(a.stress, Some(600));
        let a = args(&["--synthetic", "20000", "--stress", "40"]);
        assert_eq!(a.synthetic, Some(20_000));
        assert_eq!(a.stress, Some(40));
        // A required value is taken whatever it looks like: this types the
        // three hyphens that open a frontmatter block.
        let a = args(&["--type", "---"]);
        assert_eq!(a.type_text.as_deref(), Some("---"));
    }

    #[test]
    fn an_alias_sets_the_same_field() {
        assert_eq!(args(&["--press", "tab"]).accept.as_deref(), Some("tab"));
        assert_eq!(args(&["--accept"]).accept.as_deref(), Some("enter"));
        assert!(args(&["-h"]).help);
        assert!(args(&["-V"]).version);
    }

    #[test]
    fn the_table_and_the_parser_agree() {
        // Every flag in the table parses, and parsing it changes something:
        // a flag the `apply` match has forgotten errors, and one that sets
        // nothing would leave Args at its default.
        for flag in FLAGS {
            let line: Vec<String> = match flag.value {
                Some(_) => vec![flag.name.to_string(), sample(flag).to_string()],
                None => vec![flag.name.to_string()],
            };
            let parsed =
                parse(line.clone()).unwrap_or_else(|e| panic!("{} does not parse: {e}", flag.name));
            assert_ne!(
                parsed,
                Args::default(),
                "{} parses but sets nothing",
                flag.name
            );
            if let Some(alias) = flag.alias {
                let mut line = line;
                line[0] = alias.to_string();
                let by_alias = parse(line).unwrap_or_else(|e| panic!("{alias}: {e}"));
                assert_eq!(by_alias, parsed, "{alias} is not {}", flag.name);
            }
        }
    }

    /// A value each flag accepts, for the agreement test.
    fn sample(flag: &Flag) -> &'static str {
        match flag.name {
            "--size" => "800x600",
            "--cell" | "--synthetic" | "--stress" => "3",
            "--uri" => "obsidian://daily",
            _ => "x",
        }
    }

    #[test]
    fn the_help_text_lists_every_flag_under_a_heading() {
        let help = usage();
        for flag in FLAGS {
            assert!(help.contains(flag.name), "{} is undocumented", flag.name);
            if let Some(value) = flag.value {
                assert!(
                    help.contains(&format!("{} {value}", flag.name)),
                    "{} is documented without its value",
                    flag.name
                );
            }
        }
        for section in [OPENING, WINDOW, STATE, DRIVING, ABOUT] {
            assert!(help.contains(&format!("\n{section}:\n")), "{section}");
        }
        assert!(help.starts_with("scoria "), "{help}");
        assert!(help.contains("--new-window opts out"));
    }

    #[test]
    fn single_instance_is_off_for_a_driven_run() {
        assert!(args(&["/v"]).single_instance());
        assert!(!args(&["/v", "--new-window"]).single_instance());
        assert!(!args(&["/v", "--control", "/run/x.sock"]).single_instance());
        assert!(!args(&["/v", "--synthetic"]).single_instance());
        assert!(!args(&["/v", "--stress", "10"]).single_instance());
    }

    #[test]
    fn the_mode_flags_name_a_control_mode() {
        assert_eq!(args(&["/v"]).mode(), None);
        assert_eq!(args(&["/v", "--source"]).mode(), Some("source"));
        assert_eq!(args(&["/v", "--reading"]).mode(), Some("reading"));
        // Both is Source, as the startup order has always had it.
        assert_eq!(
            args(&["/v", "--reading", "--source"]).mode(),
            Some("source")
        );
    }
}
