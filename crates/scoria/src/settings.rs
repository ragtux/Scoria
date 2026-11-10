//! The Settings window (`app:open-settings`, Ctrl+,): the tabs Obsidian
//! 1.14.4 shows (General, Appearance, Interface, Editor, Files and links,
//! Hotkeys, Core plugins, Community plugins, then one per enabled core
//! plugin with options) with every value read from `.obsidian/`.
//!
//! Obsidian draws Settings as a modal over the workspace. Scoria gives it
//! a window of its own, the way it does the vault manager and Help: the
//! vault behind it stays readable and usable while a setting is changed,
//! and the panel is sized and moved by the desktop rather than by a
//! backdrop. [`SettingsView`] is the panel, held by the workspace so its
//! state outlives the window; [`SettingsWindow`] is the window around it.
//!
//! Toggles write. Each one carries a [`Flag`] naming the key Obsidian
//! writes, what that key means when it is absent, and how to read and set
//! it, so the state a control shows and the bytes a click writes cannot
//! drift apart. A click clones the config, flips the one field and emits
//! [`SettingChange`]; the workspace writes the file and applies what the
//! running app can change on the spot. `Vault::write_config` merges into
//! the file's own JSON, so nothing but the changed key moves.
//!
//! A dropdown writes the option it names, a slider the step it is clicked
//! on and a text field what is typed into it, each through the same table
//! its own display is read from. Every row also
//! says how far it gets in Scoria ([`Honored`]): nothing on a setting the
//! app follows as you click it, "on restart" for the theme and fonts, and
//! "not in Scoria yet" for the ones written only for Obsidian to read. The
//! vault is shared, so writing a setting Scoria ignores is still worth
//! doing — pretending it changes something here is not.

use crate::assets::{logo, DISPLAY_FONT_FAMILY};
use crate::palette::hotkey_chip;
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{fonts, Theme};
use crate::workspace::{
    client_frame, frame_window_options, window_title_row, Commands, VaultState, WindowCorners,
    Workspace,
};
use gpui::{
    actions, canvas, deferred, div, hsla, linear_color_stop, linear_gradient, point, prelude::*,
    px, size, uniform_list, App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, Hsla, KeyBinding, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Rgba, ScrollHandle, SharedString, Subscription,
    UniformListScrollHandle, WeakEntity, Window, WindowBounds, WindowHandle, WindowOptions,
};
use scoria_commands::{hotkey_label, keystroke, Command};
use scoria_formats::config::{
    js_number, FileRecoveryConfig, FILE_RECOVERY_DEFAULT_INTERVAL, FILE_RECOVERY_DEFAULT_KEEP_DAYS,
};
use scoria_formats::config::{
    AppConfig, AppearanceConfig, BacklinkConfig, CanvasConfig, CorePlugins, DailyNotesConfig,
    Hotkey, Hotkeys, NoteComposerConfig, PagePreviewConfig, SwitcherConfig, TemplatesConfig,
    WebviewerConfig, ZkPrefixerConfig, CORE_PLUGINS, DAILY_NOTE_DEFAULT_FORMAT,
    WEBVIEWER_DEFAULT_SEARCH_URL, ZK_PREFIXER_DEFAULT_FORMAT,
};
use scoria_search::fuzzy_match;
use scoria_templates as templates;
use serde_json::{json, Value};
use std::ops::Range;
use std::sync::Arc;

actions!(settings, [Close]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Close, Some("Settings"))]);
}

/// Core plugins that get their own options tab once enabled, in the order
/// Obsidian lists them under the Core plugins heading of the navigation.
const PLUGIN_TABS: &[(&str, &str)] = &[
    ("switcher", "Quick switcher"),
    ("sync", "Sync"),
    ("backlink", "Backlinks"),
    ("canvas", "Canvas"),
    ("page-preview", "Page preview"),
    ("daily-notes", "Daily notes"),
    ("templates", "Templates"),
    ("note-composer", "Note composer"),
    ("command-palette", "Command palette"),
    ("zk-prefixer", "Unique note creator"),
    ("file-recovery", "File recovery"),
    ("webviewer", "Web viewer"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    General,
    Appearance,
    Interface,
    Editor,
    Files,
    Hotkeys,
    CorePlugins,
    CommunityPlugins,
    Plugin(&'static str),
}

impl Tab {
    const OPTIONS: &'static [Tab] = &[
        Tab::General,
        Tab::Appearance,
        Tab::Interface,
        Tab::Editor,
        Tab::Files,
        Tab::Hotkeys,
        Tab::CorePlugins,
        Tab::CommunityPlugins,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::Appearance => "Appearance",
            Tab::Interface => "Interface",
            Tab::Editor => "Editor",
            Tab::Files => "Files and links",
            Tab::Hotkeys => "Hotkeys",
            Tab::CorePlugins => "Core plugins",
            Tab::CommunityPlugins => "Community plugins",
            Tab::Plugin(id) => PLUGIN_TABS
                .iter()
                .find(|(pid, _)| *pid == id)
                .map(|(_, name)| *name)
                .unwrap_or(id),
        }
    }

    /// The name [`Tab::parse`] takes back, for `--settings` and for the
    /// control socket's state dump; [`Tab::name`] is the label on the tab,
    /// which does not round trip ("Files and links").
    pub fn id(self) -> &'static str {
        match self {
            Tab::General => "general",
            Tab::Appearance => "appearance",
            Tab::Interface => "interface",
            Tab::Editor => "editor",
            Tab::Files => "files",
            Tab::Hotkeys => "hotkeys",
            Tab::CorePlugins => "core-plugins",
            Tab::CommunityPlugins => "community-plugins",
            Tab::Plugin(id) => id,
        }
    }

    /// Parses `--settings <name>`: a tab name, its Obsidian tab id, or a plugin id.
    pub fn parse(name: &str) -> Option<Tab> {
        let n = name.trim().to_lowercase();
        let tab = match n.as_str() {
            "general" | "about" => Tab::General,
            "appearance" => Tab::Appearance,
            "interface" => Tab::Interface,
            "editor" => Tab::Editor,
            "files" | "file" | "files and links" => Tab::Files,
            "hotkeys" => Tab::Hotkeys,
            "core plugins" | "plugins" | "core-plugins" => Tab::CorePlugins,
            "community plugins" | "community-plugins" => Tab::CommunityPlugins,
            _ => {
                let (id, _) = PLUGIN_TABS
                    .iter()
                    .find(|(id, label)| *id == n || label.to_lowercase() == n)?;
                Tab::Plugin(id)
            }
        };
        Some(tab)
    }
}

/// A boolean setting of one config file: the key Obsidian writes, what the
/// key means when it is absent, and how to read and set it.
///
/// The control's state is read through `get` and written through `set`, so
/// a toggle cannot show one key and write another. `invert` is for the one
/// control Obsidian words the other way round: "Use [[Wikilinks]]" is
/// `useMarkdownLinks` negated.
macro_rules! flags {
    ($ty:ident, $config:ty, $($name:ident, $key:literal, $field:ident, $default:literal, $invert:literal;)+) => {
        #[derive(Clone, Copy, Debug)]
        struct $ty {
            key: &'static str,
            default: bool,
            invert: bool,
            get: fn(&$config) -> Option<bool>,
            set: fn(&mut $config, bool),
        }

        /// Two flags are the same setting when they write the same key;
        /// the accessors are function pointers, which do not compare.
        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                self.key == other.key
            }
        }

        impl Eq for $ty {}

        impl $ty {
            /// Every flag of this file, for the test that checks the keys.
            #[allow(dead_code)]
            const ALL: &'static [$ty] = &[$($name),+];

            /// What the control shows, from the config in hand.
            fn is_on(&self, config: &$config) -> bool {
                (self.get)(config).unwrap_or(self.default) != self.invert
            }

            /// `config` with the control flipped.
            fn toggled(&self, config: &$config) -> $config {
                let mut next = config.clone();
                (self.set)(&mut next, !self.is_on(config) != self.invert);
                next
            }
        }

        $(const $name: $ty = $ty {
            key: $key,
            default: $default,
            invert: $invert,
            get: |c| c.$field,
            set: |c, v| c.$field = Some(v),
        };)+
    };
}

flags!(
    AppFlag,
    AppConfig,
    SHOW_INLINE_TITLE, "showInlineTitle", show_inline_title, true, false;
    READABLE_LINE_LENGTH, "readableLineLength", readable_line_length, true, false;
    STRICT_LINE_BREAKS, "strictLineBreaks", strict_line_breaks, false, false;
    FOLD_HEADING, "foldHeading", fold_heading, true, false;
    FOLD_INDENT, "foldIndent", fold_indent, true, false;
    SHOW_LINE_NUMBER, "showLineNumber", show_line_number, false, false;
    SHOW_INDENT_GUIDE, "showIndentGuide", show_indent_guide, true, false;
    RIGHT_TO_LEFT, "rightToLeft", right_to_left, false, false;
    SPELLCHECK, "spellcheck", spellcheck, true, false;
    AUTO_PAIR_BRACKETS, "autoPairBrackets", auto_pair_brackets, true, false;
    AUTO_PAIR_MARKDOWN, "autoPairMarkdown", auto_pair_markdown, true, false;
    SMART_INDENT_LIST, "smartIndentList", smart_indent_list, true, false;
    USE_TAB, "useTab", use_tab, true, false;
    AUTO_CONVERT_HTML, "autoConvertHtml", auto_convert_html, true, false;
    VIM_MODE, "vimMode", vim_mode, false, false;
    FOCUS_NEW_TAB, "focusNewTab", focus_new_tab, true, false;
    ALWAYS_UPDATE_LINKS, "alwaysUpdateLinks", always_update_links, false, false;
    USE_WIKILINKS, "useMarkdownLinks", use_markdown_links, false, true;
    SHOW_UNSUPPORTED_FILES, "showUnsupportedFiles", show_unsupported_files, false, false;
    PROMPT_DELETE, "promptDelete", prompt_delete, true, false;
);

flags!(
    AppearanceFlag,
    AppearanceConfig,
    SHOW_VIEW_HEADER, "showViewHeader", show_view_header, true, false;
    SHOW_RIBBON, "showRibbon", show_ribbon, true, false;
    NATIVE_MENUS, "nativeMenus", native_menus, false, false;
    QUICK_FONT_SIZE, "baseFontSizeAction", base_font_size_action, false, false;
);

flags!(
    SwitcherFlag,
    SwitcherConfig,
    SHOW_EXISTING_ONLY, "showExistingOnly", show_existing_only, false, false;
    SHOW_ATTACHMENTS, "showAttachments", show_attachments, true, false;
    SHOW_ALL_FILE_TYPES, "showAllFileTypes", show_all_file_types, false, false;
);

flags!(
    BacklinkFlag,
    BacklinkConfig,
    BACKLINK_IN_DOCUMENT, "backlinkInDocument", backlink_in_document, false, false;
);

flags!(
    CanvasFlag,
    CanvasConfig,
    SNAP_TO_GRID, "snapToGrid", snap_to_grid, true, false;
    SNAP_TO_OBJECTS, "snapToObjects", snap_to_objects, true, false;
);

// The Page preview tab's ten toggles. Obsidian names each key after the
// surface the preview pops up over, not after the row's label, and Reading
// view is the one that is off when its key is absent.
flags!(
    PagePreviewFlag,
    PagePreviewConfig,
    PREVIEW_SEARCH, "search", search, true, false;
    PREVIEW_READING, "preview", preview, false, false;
    PREVIEW_EDITING, "editor", editor, true, false;
    PREVIEW_TAB_HEADER, "tab-header", tab_header, true, false;
    PREVIEW_FILES, "file-explorer", file_explorer, true, false;
    PREVIEW_PROPERTIES, "properties", properties, true, false;
    PREVIEW_BOOKMARKS, "bookmarks", bookmarks, true, false;
    PREVIEW_OUTLINE, "outline", outline, true, false;
    PREVIEW_BASES, "bases", bases, true, false;
    PREVIEW_GRAPH, "graph", graph, true, false;
);

flags!(
    NoteComposerFlag,
    NoteComposerConfig,
    ASK_BEFORE_MERGING, "askBeforeMerging", ask_before_merging, true, false;
);

flags!(
    WebviewerFlag,
    WebviewerConfig,
    OPEN_EXTERNAL_URLS, "openExternalURLs", open_external_urls, true, false;
    ENABLE_ADBLOCKING, "enableAdblocking", enable_adblocking, true, false;
);

/// What a toggle writes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Flag {
    App(AppFlag),
    Appearance(AppearanceFlag),
    Switcher(SwitcherFlag),
    Backlink(BacklinkFlag),
    Canvas(CanvasFlag),
    PagePreview(PagePreviewFlag),
    NoteComposer(NoteComposerFlag),
    Webviewer(WebviewerFlag),
    /// A core plugin in `core-plugins.json`, with what a vault without the
    /// file does.
    CorePlugin(&'static str, bool),
    /// A community plugin id in `community-plugins.json`; enabled means
    /// listed.
    CommunityPlugin(String),
    /// A CSS snippet by its name (`wider.css` is `wider`); enabled means
    /// listed in `appearance.json`'s `enabledCssSnippets`.
    CssSnippet(String),
}

/// A dropdown's options: what each row reads and what it writes.
///
/// The labels are Obsidian's, as the Settings tab shows them; the values
/// are what it puts in the file, which the phase 3 probe confirmed for
/// every one of these keys. See
/// [[Specification#Observed settings write behavior, Obsidian 1.14.4]].
type Options = &'static [(&'static str, &'static str)];

/// A setting that is not a toggle: the key it writes and the one pair of
/// accessors its control reads through and writes through, so what a row
/// shows and what it writes cannot drift apart.
#[derive(Clone, Copy)]
struct Typed {
    key: &'static str,
    options: Options,
    /// The value in the file now, as one of `options`' written values.
    read: fn(&VaultState) -> String,
    /// The file with this setting set to `value`.
    write: fn(&VaultState, &str) -> SettingChange,
    /// Which option a stored value that is none of `options`' own values
    /// stands for, for the one key that stores a shape rather than an enum:
    /// `attachmentFolderPath` writes `./Attachments` for its subfolder
    /// option, but `./Pictures` is that same option with another name typed
    /// into the row it reveals.
    shape: Option<fn(&str) -> usize>,
    /// Renders an option's label from its written value, for the one
    /// dropdown whose labels are today's date in each format; `None` falls
    /// back to the option's static label.
    label_of: Option<fn(&'static str) -> Option<String>>,
    /// The option that is a pick rather than a value: picking it writes
    /// nothing, and the view remembers it (`SettingsView::local_pick`) so
    /// the rows it reveals stay until another option is picked.
    local: Option<&'static str>,
}

impl Typed {
    /// The option the file's value names, or the first, which is Obsidian's
    /// default for every one of these keys.
    fn selected(&self, state: &VaultState) -> usize {
        let value = (self.read)(state);
        self.options
            .iter()
            .position(|(_, v)| *v == value)
            .or_else(|| self.shape.map(|shape| shape(&value)))
            .unwrap_or(0)
    }

    /// The selected option's static label, which the tests read back.
    #[cfg(test)]
    fn label(&self, state: &VaultState) -> &'static str {
        self.options[self.selected(state)].0
    }

    /// Option `ix`'s label as the control shows it: rendered through
    /// `label_of` when there is one, else the static label.
    fn option_label(&self, ix: usize) -> String {
        let (label, value) = self.options[ix];
        self.label_of
            .and_then(|render| render(value))
            .unwrap_or_else(|| label.to_string())
    }

    /// What the selected option writes, which is how a dependent row is
    /// keyed off the option rather than off the stored string's shape.
    fn value(&self, state: &VaultState) -> &'static str {
        self.options[self.selected(state)].1
    }
}

impl PartialEq for Typed {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Typed {}

impl std::fmt::Debug for Typed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typed").field("key", &self.key).finish()
    }
}

/// Reads a string key of `app.json`, with the value Obsidian writes when it
/// is absent.
macro_rules! app_choice {
    ($name:ident, $key:literal, $field:ident, $default:literal, $options:expr) => {
        const $name: Typed = Typed {
            key: $key,
            options: $options,
            read: |state| {
                state
                    .app()
                    .$field
                    .clone()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| $default.to_string())
            },
            write: |state, value| {
                let mut next = state.app().clone();
                next.$field = Some(value.to_string());
                SettingChange::App(next)
            },
            shape: None,
            label_of: None,
            local: None,
        };
    };
}

macro_rules! appearance_choice {
    ($name:ident, $key:literal, $field:ident, $default:literal, $options:expr) => {
        const $name: Typed = Typed {
            key: $key,
            options: $options,
            read: |state| {
                state
                    .appearance()
                    .$field
                    .clone()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| $default.to_string())
            },
            write: |state, value| {
                let mut next = state.appearance().clone();
                next.$field = Some(value.to_string());
                SettingChange::Appearance(next)
            },
            shape: None,
            label_of: None,
            local: None,
        };
    };
}

app_choice!(
    DEFAULT_VIEW_MODE,
    "defaultViewMode",
    default_view_mode,
    "source",
    &[("Editing view", "source"), ("Reading view", "preview")]
);
app_choice!(
    PROPERTIES_IN_DOCUMENT,
    "propertiesInDocument",
    properties_in_document,
    "visible",
    &[
        ("Visible", "visible"),
        ("Hidden", "hidden"),
        ("Source", "source")
    ]
);
app_choice!(
    NEW_FILE_LOCATION,
    "newFileLocation",
    new_file_location,
    "root",
    &[
        ("Vault folder", "root"),
        ("Same folder as current file", "current"),
        ("In the folder specified below", "folder")
    ]
);
app_choice!(
    NEW_LINK_FORMAT,
    "newLinkFormat",
    new_link_format,
    "shortest",
    &[
        ("Shortest path when possible", "shortest"),
        ("Relative path to file", "relative"),
        ("Absolute path in vault", "absolute")
    ]
);
app_choice!(
    TRASH_OPTION,
    "trashOption",
    trash_option,
    "system",
    &[
        ("Move to system trash", "system"),
        ("Move to Obsidian trash (.trash folder)", "local"),
        ("Permanently delete", "none")
    ]
);
appearance_choice!(
    BASE_COLOR_SCHEME,
    "theme",
    theme,
    "system",
    &[
        ("Adapt to system", "system"),
        ("Light", "moonstone"),
        ("Dark", "obsidian")
    ]
);

/// The four attachment locations, each storing a different *shape* of
/// `attachmentFolderPath` rather than an enum: the `<select>`'s own values
/// (`root`, `current`, `subfolder`, `folder`) never reach the file. Picking
/// an option writes at once, so with an empty `app.json` the first pick
/// leaves exactly `{"attachmentFolderPath": "/"}` — which is why `read`
/// hands back the stored string as it stands, `""` and all, rather than
/// filling in the default: an absent key is not the `/` the first option
/// writes. Observed on Obsidian 1.14.5; see
/// [[Specification#The attachment-location dropdown]].
/// Settings → Interface → "Window frame style". The labels and the stored
/// strings are 1.14.5's (`tools/obsidian-probe/results/2026-10-20-frame-
/// setting-key.json`): `hidden` is the default on Linux and Windows. The
/// value is not a vault's: it is read from and written to `scoria.json`
/// through `crate::stored_frame_style` and `SettingChange::Frame`.
const FRAME_STYLE: Typed = Typed {
    key: "frame",
    options: &[
        ("Hidden (default)", "hidden"),
        ("Obsidian frame", "custom"),
        ("Native frame", "native"),
    ],
    read: |_| crate::stored_frame_style(),
    write: |_, value| SettingChange::Frame(value.to_string()),
    shape: None,
    label_of: None,
    local: None,
};

const ATTACHMENT_LOCATION: Typed = Typed {
    key: "attachmentFolderPath",
    options: &[
        ("Vault folder", "/"),
        ("Same folder as current file", "./"),
        ("In subfolder under current folder", "./Attachments"),
        ("In the folder specified below", "Attachments"),
    ],
    read: |state| {
        state
            .app()
            .attachment_folder_path
            .clone()
            .unwrap_or_default()
    },
    write: |state, value| {
        let mut next = state.app().clone();
        next.attachment_folder_path = Some(value.to_string());
        SettingChange::App(next)
    },
    // A name typed into either dependent row leaves the shape the option
    // chose, so the shape is what the row is read back from.
    shape: Some(|value| match value {
        "" | "/" => 0,
        "./" => 1,
        v if v.starts_with("./") => 2,
        _ => 3,
    }),
    label_of: None,
    local: None,
};

/// `livePreview` is a two-option dropdown over a boolean key, so it reads
/// and writes `true` and `false` as the strings its options name.
const LIVE_PREVIEW: Typed = Typed {
    key: "livePreview",
    options: &[("Live Preview", "true"), ("Source mode", "false")],
    read: |state| state.app().live_preview.unwrap_or(true).to_string(),
    write: |state, value| {
        let mut next = state.app().clone();
        next.live_preview = Some(value == "true");
        SettingChange::App(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// Every dropdown that writes.
const TYPED_ALL: &[Typed] = &[
    DEFAULT_VIEW_MODE,
    PROPERTIES_IN_DOCUMENT,
    NEW_FILE_LOCATION,
    NEW_LINK_FORMAT,
    TRASH_OPTION,
    BASE_COLOR_SCHEME,
    LIVE_PREVIEW,
    ATTACHMENT_LOCATION,
    DAILY_FORMAT,
    CANVAS_NEW_FILE_LOCATION,
    CANVAS_WHEEL_BEHAVIOR,
    CANVAS_MOD_DRAG_BEHAVIOR,
    CANVAS_CARD_LABELS,
    COMPOSER_REPLACEMENT_TEXT,
    WEB_SEARCH_ENGINE,
];

/// A slider: the key, the range Obsidian's own control offers, and the
/// accessor pair the value is read through and written through.
///
/// Obsidian neither clamps nor rewrites a file holding a value outside the
/// range, so a file that already does keeps it; the range only bounds what
/// this control can set.
#[derive(Clone, Copy, Debug)]
struct Number {
    key: &'static str,
    min: i64,
    max: i64,
    read: fn(&VaultState) -> i64,
    write: fn(&VaultState, i64) -> SettingChange,
}

/// A free text field, written verbatim.
#[derive(Clone, Copy, Debug)]
struct TextField {
    key: &'static str,
    /// What Obsidian shows in the empty field, which is also what it writes
    /// for some of them when the field is cleared.
    placeholder: &'static str,
    /// What the typed name is stored behind: empty for a field that is the
    /// whole value, `./` for the Subfolder name row, which is one half of
    /// `attachmentFolderPath`.
    prefix: &'static str,
    read: fn(&VaultState) -> String,
    write: fn(&VaultState, &str) -> SettingChange,
}

impl TextField {
    fn same(&self, other: &TextField) -> bool {
        self.key == other.key && self.prefix == other.prefix
    }
}

/// `accentColor` in `appearance.json`. Obsidian's control is a native
/// colour dialog that writes a lowercase `#rrggbb`, and its "Restore
/// default" writes the empty string rather than dropping the key (probed
/// 2026-11-02). GPUI raises no colour dialog, so the swatch opens
/// Scoria's own picker ([`AccentPicker`]), which carries this field for a hex
/// typed in; what lands in the file is the same.
const ACCENT_COLOR: TextField = TextField {
    key: "accentColor",
    placeholder: "#rrggbb",
    prefix: "",
    read: |state| state.appearance().accent_color.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.appearance().clone();
        next.accent_color = Some(normalize_accent(value));
        SettingChange::Appearance(next)
    },
};

/// What the accent field stores for what was typed: a `#rrggbb` in lower
/// case, `#rgb` expanded to it, and anything else — the empty field
/// included — the empty string, which is what Obsidian's "Restore default"
/// writes and what it reads as "no accent set".
fn normalize_accent(typed: &str) -> String {
    let text = typed.trim().trim_start_matches('#');
    let hex = match text.len() {
        3 => text.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => text.to_string(),
        _ => return String::new(),
    };
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return String::new();
    }
    format!("#{}", hex.to_ascii_lowercase())
}

/// A colour the way the accent picker holds it: hue in degrees, then
/// saturation and value in `0..=1`, which is the square's geometry — left
/// to right is saturation, top to bottom is value falling to black.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hsv {
    h: f32,
    s: f32,
    v: f32,
}

impl Hsv {
    fn of_rgb(r: f32, g: f32, b: f32) -> Hsv {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d < 1e-6 {
            0.
        } else if max == r {
            60. * (((g - b) / d).rem_euclid(6.))
        } else if max == g {
            60. * ((b - r) / d + 2.)
        } else {
            60. * ((r - g) / d + 4.)
        };
        let s = if max < 1e-6 { 0. } else { d / max };
        Hsv { h, s, v: max }
    }

    /// From a `#rrggbb`, as [`normalize_accent`] writes it; `None` for
    /// anything else, the empty "no accent set" included.
    fn of_hex(hex: &str) -> Option<Hsv> {
        let hex = normalize_accent(hex);
        if hex.is_empty() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        let (r, g, b) = (byte(1)?, byte(3)?, byte(5)?);
        Some(Hsv::of_rgb(
            r as f32 / 255.,
            g as f32 / 255.,
            b as f32 / 255.,
        ))
    }

    fn of_hsla(c: Hsla) -> Hsv {
        let rgb = c.to_rgb();
        Hsv::of_rgb(rgb.r, rgb.g, rgb.b)
    }

    fn rgb(self) -> (f32, f32, f32) {
        let c = self.v * self.s;
        let hp = (self.h.rem_euclid(360.)) / 60.;
        let x = c * (1. - ((hp % 2.) - 1.).abs());
        let (r, g, b) = match hp as u32 {
            0 => (c, x, 0.),
            1 => (x, c, 0.),
            2 => (0., c, x),
            3 => (0., x, c),
            4 => (x, 0., c),
            _ => (c, 0., x),
        };
        let m = self.v - c;
        (r + m, g + m, b + m)
    }

    /// The `#rrggbb` the file gets.
    fn hex(self) -> String {
        let (r, g, b) = self.rgb();
        let byte = |v: f32| (v * 255.).round().clamp(0., 255.) as u8;
        format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
    }

    fn hsla(self) -> Hsla {
        let (r, g, b) = self.rgb();
        Rgba { r, g, b, a: 1. }.into()
    }

    /// The hue at full saturation and value: the square's right edge.
    fn pure(self) -> Hsla {
        hsla(self.h / 360., 1., 0.5, 1.)
    }
}

/// Which part of the picker a press began on, and so what the drag moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PickerPart {
    Square,
    Bar,
}

/// The accent picker open under the swatch: the colour it shows, which
/// the drag moves before the file is written, and where its two controls
/// were painted, so a drag that leaves them still maps to them.
#[derive(Debug)]
struct AccentPicker {
    hsv: Hsv,
    /// The stored hex the colour was read from, so a write that lands
    /// from elsewhere (the hex field, "Restore default") is picked up on
    /// the next render and a drag's own write is not.
    from: String,
    drag: Option<PickerPart>,
    square: Bounds<Pixels>,
    bar: Bounds<Pixels>,
}

impl AccentPicker {
    /// For the stored value, or `fallback` - the theme's own accent - when
    /// nothing is stored.
    fn new(stored: &str, fallback: Hsla) -> AccentPicker {
        AccentPicker {
            hsv: Hsv::of_hex(stored).unwrap_or_else(|| Hsv::of_hsla(fallback)),
            from: normalize_accent(stored),
            drag: None,
            square: Bounds::default(),
            bar: Bounds::default(),
        }
    }
}

/// The square's width and height, and the hue bar's height, in pixels.
const PICKER_SQUARE: (f32, f32) = (200., 140.);
const PICKER_BAR: f32 = 12.;

/// Saturation and value for a point over (or past) the square.
fn square_pick(square: Bounds<Pixels>, at: Point<Pixels>) -> (f32, f32) {
    let s = ((at.x - square.origin.x) / square.size.width).clamp(0., 1.);
    let down = ((at.y - square.origin.y) / square.size.height).clamp(0., 1.);
    (s, 1. - down)
}

/// Hue in degrees for a point over (or past) the bar.
fn bar_pick(bar: Bounds<Pixels>, at: Point<Pixels>) -> f32 {
    let along = ((at.x - bar.origin.x) / bar.size.width).clamp(0., 1.);
    // The bar runs 0 to 360, and its right edge is red again: hold the
    // pick a hair short so the far edge is not the same hue as the near.
    (along * 360.).min(359.)
}

/// The presets under the square: the theme's own accent first, written as
/// the empty "no accent set", then the marks' inks (`docs/scoria/Scoria.md`,
/// "Brand colors") and, last, Obsidian's purple for anyone who misses it.
const PICKER_PRESETS: &[(&str, &str)] = &[
    ("Scoria's own", ""),
    ("Body orange", "#ff5721"),
    ("Shadow orange", "#ee3300"),
    ("Rust", "#d55227"),
    ("Sienna", "#a6462c"),
    ("Brick", "#92391f"),
    ("Red", "#ca2b27"),
    ("Deep red", "#a82223"),
    ("Brown", "#86361e"),
    ("Obsidian's purple", "#8a5cf6"),
];

/// `baseFontSize` in `appearance.json`; Obsidian's slider offers 10 to 30.
const FONT_SIZE: Number = Number {
    key: "baseFontSize",
    min: 10,
    max: 30,
    read: |state| state.appearance().base_font_size.unwrap_or(16) as i64,
    write: |state, value| {
        let mut next = state.appearance().clone();
        next.base_font_size = Some(value as u32);
        SettingChange::Appearance(next)
    },
};

/// `tabSize` in `app.json`; Obsidian's slider offers 2 to 8, default 4.
const TAB_SIZE: Number = Number {
    key: "tabSize",
    min: 2,
    max: 8,
    read: |state| state.app().tab_size.unwrap_or(4) as i64,
    write: |state, value| {
        let mut next = state.app().clone();
        next.tab_size = Some(value as u8);
        SettingChange::App(next)
    },
};

const NEW_FILE_FOLDER: TextField = TextField {
    key: "newFileFolderPath",
    placeholder: "Example: folder 1/folder 2",
    prefix: "",
    read: |state| state.app().new_file_folder_path.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.app().clone();
        next.new_file_folder_path = Some(value.to_string());
        SettingChange::App(next)
    },
};

/// Obsidian writes the placeholder rather than an empty string when this
/// one is cleared, which the probe caught: emptying the field wrote
/// `Attachments`.
const ATTACHMENT_FOLDER: TextField = TextField {
    key: "attachmentFolderPath",
    placeholder: "Attachments",
    prefix: "",
    read: |state| {
        state
            .app()
            .attachment_folder_path
            .clone()
            .unwrap_or_default()
    },
    write: |state, value| {
        let mut next = state.app().clone();
        let value = if value.is_empty() {
            ATTACHMENT_FOLDER.placeholder
        } else {
            value
        };
        next.attachment_folder_path = Some(value.to_string());
        SettingChange::App(next)
    },
};

/// The other half of `attachmentFolderPath`: the name under the note's own
/// folder, which the subfolder option reveals a row for. Typing
/// `Attachments` into it stored `./Attachments`, so the row holds what
/// follows the `./` and writes the prefix back. Clearing this one is
/// unprobed; it writes the placeholder, as its sibling row does.
const SUBFOLDER_NAME: TextField = TextField {
    key: "attachmentFolderPath",
    placeholder: "Attachments",
    prefix: "./",
    read: |state| {
        state
            .app()
            .attachment_folder_path
            .as_deref()
            .and_then(|v| v.strip_prefix(SUBFOLDER_NAME.prefix))
            .unwrap_or_default()
            .to_string()
    },
    write: |state, value| {
        let mut next = state.app().clone();
        let value = if value.is_empty() {
            SUBFOLDER_NAME.placeholder
        } else {
            value
        };
        next.attachment_folder_path = Some(format!("{}{value}", SUBFOLDER_NAME.prefix));
        SettingChange::App(next)
    },
};

/// Every slider and text field that writes, for the tests that walk them.
#[cfg(test)]
const NUMBERS: &[Number] = &[FONT_SIZE, TAB_SIZE];
#[cfg(test)]
const TEXTS: &[TextField] = &[
    NEW_FILE_FOLDER,
    ATTACHMENT_FOLDER,
    SUBFOLDER_NAME,
    DAILY_CUSTOM_FORMAT,
    DAILY_FOLDER,
    DAILY_TEMPLATE,
    TEMPLATES_FOLDER,
    TEMPLATES_DATE,
    TEMPLATES_TIME,
    WEB_CUSTOM_SEARCH_ENGINE,
    ZK_FOLDER,
    ZK_TEMPLATE,
    ZK_FORMAT,
];

// ----- the Daily notes and Templates tabs ------------------------------------
//
// Observed on 1.14.5 on 2026-10-30 (`tools/obsidian-probe/plugin-options.mjs`,
// run in `results/2026-10-30-plugin-options.json`): each field writes its own
// key into the plugin's file as a verbatim string, an emptied field writes
// `""` rather than dropping the key, and the file is `{\n  "key": "value"\n}`
// with no trailing newline — the shape `write_config` already produces for
// `switcher.json`. There is no toggle on either tab: the Daily notes tab is a
// dropdown and three fields, the Templates tab three fields.

/// The format a daily note is named by, as the Date format dropdown and the
/// Custom format field both read it: the file's value, or Obsidian's default
/// when the key is absent. An empty string is kept as one — the probe's
/// cleared field read back empty over its `YYYY-MM-DD` placeholder.
fn daily_format(state: &VaultState) -> String {
    state
        .daily_notes()
        .format
        .clone()
        .unwrap_or_else(|| DAILY_NOTE_DEFAULT_FORMAT.to_string())
}

fn write_daily_format(state: &VaultState, value: &str) -> SettingChange {
    let mut next = state.daily_notes().clone();
    next.format = Some(value.to_string());
    SettingChange::DailyNotes(next)
}

/// The Daily notes tab's "Date format" dropdown. Its four presets are labelled
/// with today's date in that format (`2026-10-30`, `2026.10.30`, ...), which
/// [`Typed::label_of`] renders; picking one writes the pattern itself as
/// `format`. The fifth, "Custom", writes nothing and reveals the Custom format
/// field — it is a pick the view remembers, not a value the file can hold —
/// and a stored format that is none of the four presets selects it.
const DAILY_FORMAT: Typed = Typed {
    key: "format",
    // The static labels are the patterns; `label_of` shows today in each.
    options: &[
        ("YYYY-MM-DD", "YYYY-MM-DD"),
        ("YYYY.MM.DD", "YYYY.MM.DD"),
        ("YYYY/MM/DD", "YYYY/MM/DD"),
        ("YYYY/MM/YYYY-MM-DD", "YYYY/MM/YYYY-MM-DD"),
        ("Custom", DAILY_FORMAT_CUSTOM),
    ],
    read: daily_format,
    write: write_daily_format,
    shape: Some(|_| 4),
    label_of: Some(|value| {
        (value != DAILY_FORMAT_CUSTOM).then(|| templates::format(value, &crate::workspace::now()))
    }),
    local: Some(DAILY_FORMAT_CUSTOM),
};

/// The dropdown's fifth option: Obsidian's `<select>` carries it as `custom`.
const DAILY_FORMAT_CUSTOM: &str = "custom";

/// "Custom format": the same key as the dropdown, typed rather than picked.
const DAILY_CUSTOM_FORMAT: TextField = TextField {
    key: "format",
    placeholder: "YYYY-MM-DD",
    prefix: "",
    read: daily_format,
    write: write_daily_format,
};

/// "New file location" on the Daily notes tab.
const DAILY_FOLDER: TextField = TextField {
    key: "folder",
    placeholder: "Example: folder 1/folder 2",
    prefix: "",
    read: |state| state.daily_notes().folder.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.daily_notes().clone();
        next.folder = Some(value.to_string());
        SettingChange::DailyNotes(next)
    },
};

/// "Template file location" on the Daily notes tab. Obsidian's control is a
/// file suggester rather than a plain field, and its placeholder went
/// unprobed; the path typed here is what the suggester would store.
const DAILY_TEMPLATE: TextField = TextField {
    key: "template",
    placeholder: "",
    prefix: "",
    read: |state| state.daily_notes().template.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.daily_notes().clone();
        next.template = Some(value.to_string());
        SettingChange::DailyNotes(next)
    },
};

/// "Template folder location" on the Templates tab.
const TEMPLATES_FOLDER: TextField = TextField {
    key: "folder",
    placeholder: "Example: folder 1/folder 2",
    prefix: "",
    read: |state| state.templates().folder.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.templates().clone();
        next.folder = Some(value.to_string());
        SettingChange::Templates(next)
    },
};

/// "Date format" on the Templates tab: what `{{date}}` expands to.
const TEMPLATES_DATE: TextField = TextField {
    key: "dateFormat",
    placeholder: templates::DEFAULT_DATE_FORMAT,
    prefix: "",
    read: |state| state.templates().date_format.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.templates().clone();
        next.date_format = Some(value.to_string());
        SettingChange::Templates(next)
    },
};

/// "Time format" on the Templates tab: what `{{time}}` expands to.
const TEMPLATES_TIME: TextField = TextField {
    key: "timeFormat",
    placeholder: templates::DEFAULT_TIME_FORMAT,
    prefix: "",
    read: |state| state.templates().time_format.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.templates().clone();
        next.time_format = Some(value.to_string());
        SettingChange::Templates(next)
    },
};

// ----- the Unique note creator tab -------------------------------------------
//
// Observed on 1.15.0 on 2026-11-09 (`tools/obsidian-probe/results/
// 2026-11-09-note-composer-unique.json`, run `settings:s01-tab-zk`): each row
// writes its own key into `zk-prefixer.json` as a verbatim string, keys in the
// order first written, and a cleared field writes `""` rather than dropping
// the key -- the Daily notes tab's shape again.

/// "New file location" on the Unique note creator tab.
const ZK_FOLDER: TextField = TextField {
    key: "folder",
    placeholder: "Example: folder 1/folder 2",
    prefix: "",
    read: |state| state.zk_prefixer().folder.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.zk_prefixer().clone();
        next.folder = Some(value.to_string());
        SettingChange::ZkPrefixer(next)
    },
};

/// "Template file location". Obsidian's control is a file picker that
/// stores the picked note's path with its `.md` (`Templates/Unique.md`) and
/// shows `Example: folder1/note` while empty; here it is a field, and the
/// path typed is what is stored.
const ZK_TEMPLATE: TextField = TextField {
    key: "template",
    placeholder: "Example: folder1/note",
    prefix: "",
    read: |state| state.zk_prefixer().template.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.zk_prefixer().clone();
        next.template = Some(value.to_string());
        SettingChange::ZkPrefixer(next)
    },
};

/// "Unique prefix format": the moment format a unique note is named by.
const ZK_FORMAT: TextField = TextField {
    key: "format",
    placeholder: ZK_PREFIXER_DEFAULT_FORMAT,
    prefix: "",
    read: |state| state.zk_prefixer().format.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.zk_prefixer().clone();
        next.format = Some(value.to_string());
        SettingChange::ZkPrefixer(next)
    },
};

// ----- the File recovery tab -------------------------------------------------
//
// Observed on 1.15.0 on 2026-11-09 (`tools/obsidian-probe/results/
// 2026-11-09-file-recovery.json`, step `s05-settings-typed`, real keys in the
// Settings window): a field commits when it loses focus, and the first write
// puts the plugin's whole options in the file, `intervalMinutes` then
// `keepDays`, the untouched one at its default. "Snapshot interval" took 10,
// 0 and 2.5 and refused -1; "History length" took 30 and 7 and refused 0 and
// 0.5 — read here as "at least 1". A field emptied is left alone (inferred:
// Obsidian wrote nothing for an emptied interval already at 0, and wrote 0
// for an emptied history length, which this does not copy).

/// "Snapshot interval": minutes, any number from 0.
const RECOVERY_INTERVAL: TextField = TextField {
    key: "intervalMinutes",
    placeholder: "",
    prefix: "",
    read: |state| {
        number_text(
            state.file_recovery().interval_minutes.as_ref(),
            FILE_RECOVERY_DEFAULT_INTERVAL,
        )
    },
    write: |state, value| recovery_change(state, value, 0.0, |c, n| c.interval_minutes = Some(n)),
};

/// "History length": days, any number from 1.
const RECOVERY_KEEP_DAYS: TextField = TextField {
    key: "keepDays",
    placeholder: "",
    prefix: "",
    read: |state| {
        number_text(
            state.file_recovery().keep_days.as_ref(),
            FILE_RECOVERY_DEFAULT_KEEP_DAYS,
        )
    },
    write: |state, value| recovery_change(state, value, 1.0, |c, n| c.keep_days = Some(n)),
};

/// A stored number as its field shows it, or the default when absent.
fn number_text(value: Option<&serde_json::Number>, default: i64) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| default.to_string())
}

/// The File recovery options after `typed` lands in one field: both keys
/// filled in (the untouched one at its default), as Obsidian's first write
/// had them, or the options unchanged when the value is refused — which
/// [`VaultState::apply_setting`] then writes nothing for.
fn recovery_change(
    state: &VaultState,
    typed: &str,
    min: f64,
    set: fn(&mut FileRecoveryConfig, serde_json::Number),
) -> SettingChange {
    let current = state.file_recovery().clone();
    let parsed = typed
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= min)
        .and_then(js_number);
    let Some(number) = parsed else {
        return SettingChange::FileRecovery(current);
    };
    let mut next = current;
    next.interval_minutes = next
        .interval_minutes
        .or_else(|| js_number(FILE_RECOVERY_DEFAULT_INTERVAL as f64));
    next.keep_days = next
        .keep_days
        .or_else(|| js_number(FILE_RECOVERY_DEFAULT_KEEP_DAYS as f64));
    set(&mut next, number);
    SettingChange::FileRecovery(next)
}

/// The sample a format row shows under itself — "Your current syntax looks
/// like this: 2026-10-30" — rendered from today for the pattern in the file,
/// or for the placeholder when the field is empty, as Obsidian does.
fn format_sample(pattern: &str, placeholder: &str) -> String {
    let pattern = if pattern.is_empty() {
        placeholder
    } else {
        pattern
    };
    templates::format(pattern, &crate::workspace::now())
}

// ----- the other core-plugin options tabs ------------------------------------
//
// Backlinks, Canvas, Page preview, Note composer, Unique note creator, File
// recovery and Web viewer, observed on 1.14.5 on 2026-10-30
// (`tools/obsidian-probe/results/2026-10-30-editor-menus.json`, step
// `s04-plugin-tabs`): every row read, every toggle clicked twice, every
// dropdown walked through its options and back, and `.obsidian` diffed after
// each click. Every name, description, option label and stored string below
// is that run's, character for character. See
// [[Specification#Observed core-plugin options tabs, Obsidian 1.14.5
// (installer build 1.13.7)]].
//
// Some rows wrote nothing the probe could see, because the control is a file
// suggester, a textarea, a `type=number` field or a button and none of them
// was driven: the Canvas tab's folder name and zoom threshold, every row of
// the File recovery tab (the Unique note creator's were typed into on
// 2026-11-09, and write), and the Web viewer tab's
// homepage, saved-page folder, ad-blocking rules and update frequency. Those
// rows are drawn and tagged, and they write nothing at all — a key guessed
// from the row's name would be a key Obsidian never wrote.

/// The Canvas tab's "Default location for new canvas files". The same three
/// labels and strings as the Files and links tab's own `newFileLocation`
/// dropdown, in `canvas.json` rather than `app.json`.
const CANVAS_NEW_FILE_LOCATION: Typed = Typed {
    key: "newFileLocation",
    options: &[
        ("Vault folder", "root"),
        ("Same folder as current file", "current"),
        ("In the folder specified below", "folder"),
    ],
    read: |state| {
        state
            .canvas_options()
            .new_file_location
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "root".to_string())
    },
    write: |state, value| {
        let mut next = state.canvas_options().clone();
        next.new_file_location = Some(value.to_string());
        SettingChange::Canvas(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// "Default mouse wheel behavior".
const CANVAS_WHEEL_BEHAVIOR: Typed = Typed {
    key: "defaultWheelBehavior",
    options: &[("Pan", "pan"), ("Zoom", "zoom")],
    read: |state| {
        state
            .canvas_options()
            .default_wheel_behavior
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "pan".to_string())
    },
    write: |state, value| {
        let mut next = state.canvas_options().clone();
        next.default_wheel_behavior = Some(value.to_string());
        SettingChange::Canvas(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// "Default Ctrl + Drag behavior". The row is named with Ctrl on Linux and
/// Windows; the key and its six strings are the same everywhere.
const CANVAS_MOD_DRAG_BEHAVIOR: Typed = Typed {
    key: "defaultModDragBehavior",
    options: &[
        ("Show menu", "menu"),
        ("Add card", "card"),
        ("Add note from vault", "note"),
        ("Add media from vault", "media"),
        ("Add web page", "webpage"),
        ("Create group", "group"),
    ],
    read: |state| {
        state
            .canvas_options()
            .default_mod_drag_behavior
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "menu".to_string())
    },
    write: |state, value| {
        let mut next = state.canvas_options().clone();
        next.default_mod_drag_behavior = Some(value.to_string());
        SettingChange::Canvas(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// "Show card names".
const CANVAS_CARD_LABELS: Typed = Typed {
    key: "cardLabelVisibility",
    options: &[
        ("Always", "always"),
        ("On hover", "hover"),
        ("Never", "never"),
    ],
    read: |state| {
        state
            .canvas_options()
            .card_label_visibility
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "always".to_string())
    },
    write: |state, value| {
        let mut next = state.canvas_options().clone();
        next.card_label_visibility = Some(value.to_string());
        SettingChange::Canvas(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// The Note composer tab's "Text after extraction".
const COMPOSER_REPLACEMENT_TEXT: Typed = Typed {
    key: "replacementText",
    options: &[
        ("Link to new file", "link"),
        ("Embed new file", "embed"),
        ("None", "none"),
    ],
    read: |state| {
        state
            .note_composer()
            .replacement_text
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "link".to_string())
    },
    write: |state, value| {
        // The plugin saves its whole options object, so the first pick on
        // a vault with no file writes `askBeforeMerging: true` before the
        // key it set (2026-11-09).
        let mut next = state.note_composer().clone().with_defaults();
        next.replacement_text = Some(value.to_string());
        SettingChange::NoteComposer(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// The Note composer tab's "Template file location": the template's path
/// with `.md`, `""` once cleared (2026-11-09).
const COMPOSER_TEMPLATE: TextField = TextField {
    key: "template",
    placeholder: "Example: folder/note",
    prefix: "",
    read: |state| state.note_composer().template.clone().unwrap_or_default(),
    write: |state, value| {
        let mut next = state.note_composer().clone().with_defaults();
        next.template = Some(value.to_string());
        SettingChange::NoteComposer(next)
    },
};

/// The Web viewer tab's "Search engine". Picking `custom` also wrote
/// `customSearchEngine` in Obsidian, with whatever the field below held —
/// the DuckDuckGo URL, in the probe's fresh vault. Here the pick writes its
/// own key alone and the field below writes the other, which is the rule
/// every other control in this window follows: one row, one key.
const WEB_SEARCH_ENGINE: Typed = Typed {
    key: "searchEngine",
    options: &[
        ("DuckDuckGo", "duckduckgo"),
        ("Brave", "brave"),
        ("Bing", "bing"),
        ("Google", "google"),
        ("Custom", "custom"),
    ],
    read: |state| {
        state
            .webviewer()
            .search_engine
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "duckduckgo".to_string())
    },
    write: |state, value| {
        let mut next = state.webviewer().clone();
        next.search_engine = Some(value.to_string());
        SettingChange::Webviewer(next)
    },
    shape: None,
    label_of: None,
    local: None,
};

/// The Web viewer tab's "Custom search engine": the one row of these tabs
/// that is a plain text field with an observed key. Obsidian shows the
/// DuckDuckGo URL as both the value and the placeholder of an untouched
/// field, so an absent key reads as that URL.
const WEB_CUSTOM_SEARCH_ENGINE: TextField = TextField {
    key: "customSearchEngine",
    placeholder: WEBVIEWER_DEFAULT_SEARCH_URL,
    prefix: "",
    read: |state| {
        state
            .webviewer()
            .custom_search_engine
            .clone()
            .unwrap_or_else(|| WEBVIEWER_DEFAULT_SEARCH_URL.to_string())
    },
    write: |state, value| {
        let mut next = state.webviewer().clone();
        next.custom_search_engine = Some(value.to_string());
        SettingChange::Webviewer(next)
    },
};

/// A config file with one setting changed, for the workspace to write.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingChange {
    App(AppConfig),
    Appearance(AppearanceConfig),
    Switcher(SwitcherConfig),
    CorePlugins(CorePlugins),
    CommunityPlugins(Vec<String>),
    /// `hotkeys.json` whole: the map in the file's order with one entry
    /// added, changed or removed.
    Hotkeys(Hotkeys),
    /// `daily-notes.json`: the Daily notes tab's three rows.
    DailyNotes(DailyNotesConfig),
    /// `templates.json`: the Templates tab's three rows.
    Templates(TemplatesConfig),
    /// `backlink.json`: the Backlinks tab's one toggle.
    Backlink(BacklinkConfig),
    /// `canvas.json`: the Canvas tab's four dropdowns and two toggles.
    Canvas(CanvasConfig),
    /// `page-preview.json`: the Page preview tab's ten toggles.
    PagePreview(PagePreviewConfig),
    /// `note-composer.json`: the Note composer tab's dropdown and toggle.
    NoteComposer(NoteComposerConfig),
    /// `zk-prefixer.json`: the Unique note creator tab's three fields.
    ZkPrefixer(ZkPrefixerConfig),
    /// `file-recovery.json`: the File recovery tab's two number fields.
    FileRecovery(FileRecoveryConfig),
    /// `webviewer.json`: the Web viewer tab's two toggles, its search
    /// engine dropdown and the custom engine field.
    Webviewer(WebviewerConfig),
    /// The window frame style, which is not a vault's: it goes to the
    /// `frame` key of `scoria.json` beside the vault list, where Obsidian's
    /// `obsidian.json` keeps its own.
    Frame(String),
}

impl SettingChange {
    /// The file this change is written to, for an error message.
    pub fn file(&self) -> &'static str {
        match self {
            SettingChange::App(_) => "app.json",
            SettingChange::Appearance(_) => "appearance.json",
            SettingChange::Switcher(_) => "switcher.json",
            SettingChange::CorePlugins(_) => "core-plugins.json",
            SettingChange::CommunityPlugins(_) => "community-plugins.json",
            SettingChange::Hotkeys(_) => "hotkeys.json",
            SettingChange::DailyNotes(_) => "daily-notes.json",
            SettingChange::Templates(_) => "templates.json",
            SettingChange::Backlink(_) => "backlink.json",
            SettingChange::Canvas(_) => "canvas.json",
            SettingChange::PagePreview(_) => "page-preview.json",
            SettingChange::NoteComposer(_) => "note-composer.json",
            SettingChange::ZkPrefixer(_) => "zk-prefixer.json",
            SettingChange::FileRecovery(_) => "file-recovery.json",
            SettingChange::Webviewer(_) => "webviewer.json",
            SettingChange::Frame(_) => scoria_vault::registry::FILE_NAME,
        }
    }
}

/// The flag a toggle command flips: `editor:toggle-readable-line-length`
/// and `editor:toggle-line-numbers` are the Editor tab's "Readable line
/// length" and "Line numbers" rows run from the palette, and
/// `app:toggle-ribbon` is the Interface tab's "Show ribbon" (`showRibbon`
/// in `appearance.json`, appended and then kept over two runs, Obsidian
/// 1.15.0), so each writes through the very flag the row does, and the row
/// and the command cannot disagree about the key or about what its absence
/// means.
fn command_flag(id: &str) -> Option<Flag> {
    match id {
        "editor:toggle-readable-line-length" => Some(Flag::App(READABLE_LINE_LENGTH)),
        "editor:toggle-line-numbers" => Some(Flag::App(SHOW_LINE_NUMBER)),
        "editor:toggle-spellcheck" => Some(Flag::App(SPELLCHECK)),
        "app:toggle-ribbon" => Some(Flag::Appearance(SHOW_RIBBON)),
        _ => None,
    }
}

/// What a toggle command writes: its row's file with the flag flipped, or
/// `None` for an id that is not one of them. The config is read and let go
/// in here, so the caller holds no read guard when it writes the change.
pub fn toggle_command(id: &str, state: &VaultState) -> Option<SettingChange> {
    command_flag(id).map(|flag| flag.toggled(state))
}

/// `app:toggle-default-new-pane-mode` ("Toggle default mode for new
/// tabs"): `defaultViewMode` in `app.json` flipped between `source` and
/// `preview`, an absent key reading as `source` the way the dropdown reads
/// it, and the notice Obsidian 1.15.0 showed for each direction.
pub fn toggle_default_view_mode(app: &AppConfig) -> (SettingChange, &'static str) {
    let mut next = app.clone();
    let reading = app.default_view_mode.as_deref() == Some("preview");
    let (value, notice) = if reading {
        ("source", "Switched default view mode to editing view.")
    } else {
        ("preview", "Switched default view mode to reading view.")
    };
    next.default_view_mode = Some(value.to_string());
    (SettingChange::App(next), notice)
}

/// `theme:toggle-light-dark` over a base theme the file names: `obsidian`
/// (dark) and `moonstone` (light) swapped, the other keys kept. Under
/// `system`, a missing key (which Obsidian reads as `system`) or anything
/// else it writes nothing and answers `None`; the caller flips the
/// session's scheme instead (Obsidian 1.15.0, all four starting points).
pub fn toggle_light_dark(appearance: &AppearanceConfig) -> Option<AppearanceConfig> {
    let swapped = match appearance.theme.as_deref() {
        Some("obsidian") => "moonstone",
        Some("moonstone") => "obsidian",
        _ => return None,
    };
    let mut next = appearance.clone();
    next.theme = Some(swapped.to_string());
    Some(next)
}

impl Flag {
    fn is_on(&self, state: &VaultState) -> bool {
        match self {
            Flag::App(flag) => flag.is_on(&state.app()),
            Flag::Appearance(flag) => flag.is_on(&state.appearance()),
            Flag::Switcher(flag) => flag.is_on(&state.switcher()),
            Flag::Backlink(flag) => flag.is_on(&state.backlink()),
            Flag::Canvas(flag) => flag.is_on(&state.canvas_options()),
            Flag::PagePreview(flag) => flag.is_on(&state.page_preview()),
            Flag::NoteComposer(flag) => flag.is_on(&state.note_composer()),
            Flag::Webviewer(flag) => flag.is_on(&state.webviewer()),
            Flag::CorePlugin(id, default_on) => {
                core_plugin_enabled(state.core_plugins().as_ref(), id, *default_on)
            }
            Flag::CommunityPlugin(id) => state.community_plugins().contains(id),
            Flag::CssSnippet(name) => state
                .appearance()
                .enabled_css_snippets
                .iter()
                .flatten()
                .any(|s| s == name),
        }
    }

    /// The config file with this setting flipped.
    fn toggled(&self, state: &VaultState) -> SettingChange {
        match self {
            Flag::App(flag) => SettingChange::App(flag.toggled(&state.app())),
            Flag::Appearance(flag) => SettingChange::Appearance(flag.toggled(&state.appearance())),
            Flag::Switcher(flag) => SettingChange::Switcher(flag.toggled(&state.switcher())),
            Flag::Backlink(flag) => SettingChange::Backlink(flag.toggled(&state.backlink())),
            Flag::Canvas(flag) => SettingChange::Canvas(flag.toggled(&state.canvas_options())),
            Flag::PagePreview(flag) => {
                SettingChange::PagePreview(flag.toggled(&state.page_preview()))
            }
            Flag::NoteComposer(flag) => {
                SettingChange::NoteComposer(flag.toggled(&state.note_composer()))
            }
            Flag::Webviewer(flag) => SettingChange::Webviewer(flag.toggled(&state.webviewer())),
            Flag::CorePlugin(id, _) => SettingChange::CorePlugins(toggled_core_plugin(
                state.core_plugins().as_ref(),
                id,
                self.is_on(state),
            )),
            Flag::CommunityPlugin(id) => {
                let mut plugins = state.community_plugins().clone();
                match plugins.iter().position(|p| p == id) {
                    Some(ix) => {
                        plugins.remove(ix);
                    }
                    None => plugins.push(id.clone()),
                }
                SettingChange::CommunityPlugins(plugins)
            }
            // Obsidian appends the name on enable and takes it out on
            // disable, so the list is in the order the toggles were
            // switched on. The order the snippets are *applied* in is the
            // folder's, not this one (see `theme::sheets_of`).
            Flag::CssSnippet(name) => {
                let mut next = state.appearance().clone();
                let mut enabled = next.enabled_css_snippets.take().unwrap_or_default();
                match enabled.iter().position(|s| s == name) {
                    Some(ix) => {
                        enabled.remove(ix);
                    }
                    None => enabled.push(name.clone()),
                }
                next.enabled_css_snippets = Some(enabled);
                SettingChange::Appearance(next)
            }
        }
    }
}

/// Whether the core plugin `id` is enabled in this vault: the file's word,
/// or the fresh-vault default when the file does not mention it.
pub fn plugin_enabled(state: &VaultState, id: &str) -> bool {
    CORE_PLUGINS
        .iter()
        .find(|p| p.id == id)
        .is_some_and(|p| CorePlugins::is_enabled_or_default(state.core_plugins().as_ref(), p))
}

/// Whether `id` is on, with `default_on` for a vault that has no
/// `core-plugins.json` and for an id the file does not list.
fn core_plugin_enabled(config: Option<&CorePlugins>, id: &str, default_on: bool) -> bool {
    match config {
        Some(CorePlugins::Map(map)) => map.get(id).copied().unwrap_or(default_on),
        Some(CorePlugins::List(list)) => list.iter().any(|p| p == id),
        None => default_on,
    }
}

/// `core-plugins.json` with `id` flipped. A vault without the file gets one
/// holding every plugin at its default, the shape Obsidian writes, rather
/// than a single key that would read as "everything else off".
fn toggled_core_plugin(config: Option<&CorePlugins>, id: &str, was_on: bool) -> CorePlugins {
    match config {
        Some(CorePlugins::Map(map)) => {
            let mut map = map.clone();
            map.insert(id.to_string(), !was_on);
            CorePlugins::Map(map)
        }
        Some(CorePlugins::List(list)) => {
            let mut list = list.clone();
            match list.iter().position(|p| p == id) {
                Some(ix) => {
                    list.remove(ix);
                }
                None => list.push(id.to_string()),
            }
            CorePlugins::List(list)
        }
        None => {
            let mut map = CORE_PLUGINS
                .iter()
                .map(|p| (p.id.to_string(), p.default_on))
                .collect::<indexmap::IndexMap<String, bool>>();
            map.insert(id.to_string(), !was_on);
            CorePlugins::Map(map)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Control {
    None,
    Toggle(bool),
    Dropdown(SharedString),
    Text(SharedString),
    Slider(SharedString),
    Button(SharedString),
    /// The accent row: a swatch of the colour stored, empty for the
    /// default, behind a "Restore default" button. Obsidian's control is
    /// `<input type="color">` beside a `lucide-rotate-ccw` clickable icon;
    /// the swatch opens Scoria's own picker, since there is no native
    /// colour dialog to raise.
    Color(SharedString),
    /// A row that opens a page of its own, drawn as what it reads on the
    /// right and then a chevron: `.setting-item.mod-navigable.tappable`.
    Navigable(SharedString),
    /// "Custom app icon": a "Restore default" icon, shown only while an
    /// icon is chosen, a `Choose` button and a `mod-cta` `Relaunch`, shown
    /// once the icon has changed this session. Holds the chosen file's
    /// name, empty at the default.
    AppIcon(SharedString),
}

/// How far a setting gets in Scoria.
///
/// Every value here is written for the vault, which Obsidian reads too, so
/// a setting Scoria ignores is still worth setting. Saying which is which
/// is the honest part: a row with no tag changes Scoria as you click it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Honored {
    /// Read while the vault is open; the change shows at once.
    Live,
    /// Read when Scoria starts (the theme and the fonts).
    OnRestart,
    /// Not read at all yet: the value is written for Obsidian.
    No,
}

impl Honored {
    /// The tag shown beside the setting's name, if any.
    fn tag(self) -> Option<&'static str> {
        match self {
            Honored::Live => None,
            Honored::OnRestart => Some("on restart"),
            Honored::No => Some("not in Scoria yet"),
        }
    }
}

/// Whether an open dropdown swallows this click, which closes it and spends
/// the click: the one gesture does not also do what it landed on. The next
/// click, with nothing open, goes through.
fn popover_swallows_click(menu: &mut Option<&'static str>) -> bool {
    menu.take().is_some()
}

/// What Escape closes, innermost first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Escape {
    /// The chord capture waiting for the next key press.
    Capture,
    /// An open dropdown's option list.
    Popover,
    /// A navigable row's page, which backs out to the tab's own rows.
    Page,
    /// The modal itself, when nothing inside it is open.
    Modal,
}

/// Escape takes the innermost thing open: the chord capture, then an open
/// dropdown, then a page, and the modal only when none of them is.
/// Obsidian's dropdown is a native `<select>`, so Escape over an open
/// option list closes the list and leaves the dialog where it was; and
/// Escape on the CSS snippets page goes back to the Appearance tab with
/// Settings still up (probed 2026-11-02, `appearance-tab.mjs` step s18).
fn escape_closes(capturing: bool, popover_open: bool, page_open: bool) -> Escape {
    if capturing {
        Escape::Capture
    } else if popover_open {
        Escape::Popover
    } else if page_open {
        Escape::Page
    } else {
        Escape::Modal
    }
}

/// A dropdown whose options are the vault's rather than a fixed table:
/// the Themes row lists what is installed under `themes/`, so the options
/// are built per render and carry their own written values.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Picker {
    /// Identifies the open menu, and is the key the row writes.
    key: &'static str,
    /// Each option's label and the value picking it writes.
    options: Vec<(SharedString, String)>,
    selected: usize,
}

/// A page the Appearance tab opens in place of its rows. Obsidian's rows
/// for these are `.setting-item.mod-navigable.tappable`, and the page has
/// a titlebar of its own with a back chevron (probed 2026-11-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// "CSS snippets": the reload and folder buttons, a search box and one
    /// toggle per `.css` file under `snippets/`.
    CssSnippets,
    /// "Interface font", "Text font" or "Monospace font": the families
    /// applied, then a search box over the installed ones.
    Font(FontSlot),
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::CssSnippets => "CSS snippets",
            Page::Font(slot) => slot.title(),
        }
    }
}

/// Which of `appearance.json`'s three font fields a font page edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSlot {
    Interface,
    Text,
    Monospace,
}

impl FontSlot {
    fn title(self) -> &'static str {
        match self {
            FontSlot::Interface => "Interface font",
            FontSlot::Text => "Text font",
            FontSlot::Monospace => "Monospace font",
        }
    }

    fn desc(self) -> &'static str {
        match self {
            FontSlot::Interface => "Set base font for all of Obsidian.",
            FontSlot::Text => "Set font for editing and reading views.",
            FontSlot::Monospace => "Set font for places like code blocks and frontmatter.",
        }
    }

    fn field(self, a: &AppearanceConfig) -> Option<&str> {
        match self {
            FontSlot::Interface => a.interface_font_family.as_deref(),
            FontSlot::Text => a.text_font_family.as_deref(),
            FontSlot::Monospace => a.monospace_font_family.as_deref(),
        }
    }

    /// The families the field holds, in its order.
    fn read(self, a: &AppearanceConfig) -> Vec<String> {
        fonts::font_list(self.field(a).unwrap_or_default())
    }

    /// `a` with the field holding `list`, as Obsidian writes it: an empty
    /// list writes `""` rather than dropping the key.
    fn write(self, a: &AppearanceConfig, list: &[String]) -> AppearanceConfig {
        let mut next = a.clone();
        let value = Some(fonts::join_fonts(list));
        match self {
            FontSlot::Interface => next.interface_font_family = value,
            FontSlot::Text => next.text_font_family = value,
            FontSlot::Monospace => next.monospace_font_family = value,
        }
        next
    }
}

/// One edit a font page makes to its list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FontEdit {
    /// A row of the installed list, or the typed name, picked: it goes to
    /// the end of the list.
    Add(String),
    /// The Delete button of the applied row at this index.
    Remove(usize),
    /// The applied row at `from` dragged onto the row at `to`.
    Move { from: usize, to: usize },
}

impl FontEdit {
    /// `list` with the edit made; `None` when it changes nothing, so
    /// nothing is written.
    fn apply(&self, list: &[String]) -> Option<Vec<String>> {
        let mut next = list.to_vec();
        match self {
            FontEdit::Add(name) => {
                let name = name.trim();
                if name.is_empty() || list.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                    return None;
                }
                next.push(name.to_string());
            }
            FontEdit::Remove(ix) => {
                if *ix >= next.len() {
                    return None;
                }
                next.remove(*ix);
            }
            FontEdit::Move { from, to } => {
                if from == to || *from >= next.len() || *to >= next.len() {
                    return None;
                }
                let family = next.remove(*from);
                next.insert(*to, family);
            }
        }
        Some(next)
    }
}

/// What a font page's second card lists under its search box: the typed
/// name first, when it is neither an installed family nor one already
/// applied (Obsidian offers it whether the machine has it or not, warning
/// when it does not), then every installed family not yet applied whose
/// name contains the query, without case, in the installed order.
fn font_choices(
    installed: &[String],
    applied: &[String],
    query: &str,
) -> (Option<String>, Vec<String>) {
    let query = query.trim();
    let needle = query.to_lowercase();
    let applied_has = |f: &str| applied.iter().any(|a| a.eq_ignore_ascii_case(f));
    let rows: Vec<String> = installed
        .iter()
        .filter(|f| !applied_has(f))
        .filter(|f| needle.is_empty() || f.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    let typed = (!query.is_empty()
        && !applied_has(query)
        && !installed.iter().any(|f| f.eq_ignore_ascii_case(query)))
    .then(|| query.to_string());
    (typed, rows)
}

/// The payload of an applied font row's drag handle, drawn under the
/// pointer as the family's name in its own face.
#[derive(Debug, Clone)]
struct FontMove {
    from: usize,
    family: SharedString,
}

impl Render for FontMove {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .px_3()
            .py_1()
            .rounded_sm()
            .bg(theme.tokens.background_secondary)
            .border_1()
            .border_color(theme.tokens.background_modifier_border)
            .text_size(px(14.))
            .text_color(theme.tokens.text_normal)
            .font_family(self.family.clone())
            .child(self.family.clone())
    }
}

/// The custom app icon as the Appearance tab shows it: the copy recorded
/// in `scoria.json`, its picture for the row's description, and whether it
/// changed this session, which is when Obsidian shows "Relaunch".
#[derive(Default)]
struct AppIcon {
    name: Option<String>,
    preview: Option<Arc<gpui::Image>>,
    changed: bool,
}

impl AppIcon {
    /// What the registry records now, with the picture read off disk.
    fn read() -> AppIcon {
        let registry = scoria_vault::registry::VaultRegistry::open().ok().flatten();
        let name = registry.as_ref().and_then(|r| r.icon().map(str::to_string));
        let preview = registry
            .as_ref()
            .and_then(|r| r.icon_path())
            .and_then(|path| {
                let format = match path.extension()?.to_str()? {
                    "png" => gpui::ImageFormat::Png,
                    "ico" => gpui::ImageFormat::Ico,
                    _ => return None,
                };
                let bytes = std::fs::read(&path).ok()?;
                Some(Arc::new(gpui::Image::from_bytes(format, bytes)))
            });
        AppIcon {
            name,
            preview,
            changed: false,
        }
    }
}

#[derive(Debug, Clone)]
struct Item {
    heading: bool,
    name: SharedString,
    desc: SharedString,
    control: Control,
    /// What a click writes; `None` for the controls that are still
    /// read-only.
    flag: Option<Flag>,
    /// What a dropdown writes, for the controls that are not toggles.
    typed: Option<Typed>,
    /// What a dropdown built from the vault writes.
    picker: Option<Picker>,
    /// What a slider writes.
    number: Option<Number>,
    /// What a text field writes.
    field: Option<TextField>,
    /// The page a navigable row opens.
    page: Option<Page>,
    /// The `.obsidian` subfolder a `lucide-folder-open` button before the
    /// control opens: Obsidian puts one on the Themes row and one on the
    /// CSS snippets page.
    folder: Option<&'static str>,
    honored: Honored,
}

impl Item {
    /// Scoria reads this one when it starts.
    fn on_restart(self) -> Item {
        Item {
            honored: Honored::OnRestart,
            ..self
        }
    }

    /// Scoria does not read this one yet; it is written for Obsidian.
    fn not_yet(self) -> Item {
        Item {
            honored: Honored::No,
            ..self
        }
    }
}

fn heading(name: &str) -> Item {
    Item {
        heading: true,
        name: name.to_string().into(),
        desc: "".into(),
        control: Control::None,
        flag: None,
        typed: None,
        picker: None,
        number: None,
        field: None,
        page: None,
        folder: None,
        honored: Honored::Live,
    }
}

fn item(name: &str, desc: &str, control: Control) -> Item {
    Item {
        heading: false,
        name: name.to_string().into(),
        desc: desc.to_string().into(),
        control,
        flag: None,
        typed: None,
        picker: None,
        number: None,
        field: None,
        page: None,
        folder: None,
        honored: Honored::Live,
    }
}

/// A toggle drawn from the flag it writes, so its state and its click are
/// the same key.
fn toggle(name: &str, desc: &str, on: bool, flag: Flag) -> Item {
    Item {
        flag: Some(flag),
        ..item(name, desc, Control::Toggle(on))
    }
}

fn app_toggle(name: &str, desc: &str, app: &AppConfig, flag: AppFlag) -> Item {
    toggle(name, desc, flag.is_on(app), Flag::App(flag))
}

fn appearance_toggle(
    name: &str,
    desc: &str,
    appearance: &AppearanceConfig,
    flag: AppearanceFlag,
) -> Item {
    toggle(name, desc, flag.is_on(appearance), Flag::Appearance(flag))
}

fn switcher_toggle(name: &str, desc: &str, switcher: &SwitcherConfig, flag: SwitcherFlag) -> Item {
    toggle(name, desc, flag.is_on(switcher), Flag::Switcher(flag))
}

fn dropdown(name: &str, desc: &str, value: impl Into<SharedString>) -> Item {
    item(name, desc, Control::Dropdown(value.into()))
}

/// A slider that writes, reading and writing through the one accessor pair.
fn slider(name: &str, desc: &str, state: &VaultState, number: Number) -> Item {
    Item {
        number: Some(number),
        ..item(
            name,
            desc,
            Control::Slider((number.read)(state).to_string().into()),
        )
    }
}

/// A text field that writes.
fn field(name: &str, desc: &str, state: &VaultState, field: TextField) -> Item {
    Item {
        field: Some(field),
        ..item(name, desc, Control::Text((field.read)(state).into()))
    }
}

/// A dropdown whose options come from the vault, showing the one the file
/// names (or the first, which is the default, when it names none).
fn picker(
    name: &str,
    desc: &str,
    key: &'static str,
    options: Vec<(SharedString, String)>,
    value: &str,
) -> Item {
    let selected = options.iter().position(|(_, v)| v == value).unwrap_or(0);
    let shown = options
        .get(selected)
        .map(|(label, _)| label.clone())
        .unwrap_or_default();
    Item {
        picker: Some(Picker {
            key,
            options,
            selected,
        }),
        ..item(name, desc, Control::Dropdown(shown))
    }
}

/// A row that opens a page of its own, showing `value` on its way there.
fn navigable(name: &str, desc: &str, value: impl Into<SharedString>, page: Page) -> Item {
    Item {
        page: Some(page),
        ..item(name, desc, Control::Navigable(value.into()))
    }
}

/// A dropdown that writes: what it shows and what a pick writes both come
/// from `typed`, so a row cannot show one key and write another.
fn choice(name: &str, desc: &str, state: &VaultState, typed: Typed) -> Item {
    choice_at(name, desc, typed, typed.selected(state))
}

/// [`choice`] showing option `ix`, for the dropdown whose selection is not
/// only the file's: the Daily notes tab's "Custom" is picked, not stored.
fn choice_at(name: &str, desc: &str, typed: Typed, ix: usize) -> Item {
    Item {
        typed: Some(typed),
        ..item(name, desc, Control::Dropdown(typed.option_label(ix).into()))
    }
}

fn text(name: &str, desc: &str, value: impl Into<SharedString>) -> Item {
    item(name, desc, Control::Text(value.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyFilter {
    All,
    Assigned,
    Unassigned,
    Custom,
}

impl HotkeyFilter {
    const ALL: &'static [HotkeyFilter] = &[
        HotkeyFilter::All,
        HotkeyFilter::Assigned,
        HotkeyFilter::Unassigned,
        HotkeyFilter::Custom,
    ];

    fn label(self) -> &'static str {
        match self {
            HotkeyFilter::All => "All",
            HotkeyFilter::Assigned => "Assigned",
            HotkeyFilter::Unassigned => "Unassigned",
            HotkeyFilter::Custom => "Assigned by me",
        }
    }

    fn keeps(self, c: &Command) -> bool {
        match self {
            HotkeyFilter::All => true,
            HotkeyFilter::Assigned => !c.hotkeys.is_empty(),
            HotkeyFilter::Unassigned => c.hotkeys.is_empty(),
            HotkeyFilter::Custom => c.custom,
        }
    }
}

pub enum SettingsEvent {
    Close,
    /// A control changed a setting: the config file to write, with the one
    /// key changed. The workspace writes it and applies what it can. Boxed
    /// because a whole `AppConfig` dwarfs the other events.
    Changed(Box<SettingChange>),
    /// Something the modal did outside the vault failed and the person
    /// should be told: the one case is a folder button whose file manager
    /// would not start.
    Report(String),
}

/// What the vault holds under `themes/` and `snippets/`.
///
/// Read when the modal opens and whenever the vault's state is set again
/// — which a snippet toggle, a theme pick and the watcher all do — rather
/// than scanned per frame, since `items` is called on every render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CssFolders {
    /// Installed theme names, which are also their folder names.
    themes: Vec<String>,
    /// Snippet names without the `.css`, in the order they cascade in.
    snippets: Vec<String>,
}

impl CssFolders {
    fn read(state: &VaultState) -> CssFolders {
        let vault = state.vault();
        CssFolders {
            themes: vault.css_themes(),
            snippets: vault.css_snippets(),
        }
    }

    /// The enabled snippets whose file is still there, which is the count
    /// the collapsed row shows.
    fn enabled_count(&self, appearance: &AppearanceConfig) -> usize {
        appearance
            .enabled_css_snippets
            .iter()
            .flatten()
            .filter(|name| self.snippets.iter().any(|s| s == *name))
            .count()
    }
}

pub struct SettingsView {
    focus_handle: FocusHandle,
    state: Option<Arc<VaultState>>,
    tab: Tab,
    /// The page the Appearance tab has open in place of its rows.
    page: Option<Page>,
    /// What the vault has under `themes/` and `snippets/`.
    css: CssFolders,
    /// The CSS snippets page's "Search snippets..." box.
    snippet_search: Entity<TextInput>,
    /// A font page's "Enter font name..." box.
    font_search: Entity<TextInput>,
    /// The font page's list of installed families.
    font_scroll: UniformListScrollHandle,
    /// The custom app icon `scoria.json` records.
    app_icon: AppIcon,
    hotkey_search: Entity<TextInput>,
    hotkey_filter: HotkeyFilter,
    /// Every command the Hotkeys tab can list, sorted by name.
    all_commands: Vec<Command>,
    /// Indices into `all_commands` after search and filter.
    hotkey_rows: Vec<usize>,
    hotkey_scroll: UniformListScrollHandle,
    /// The open tab's rows. Switching tabs puts it back to the top: Obsidian
    /// rebuilds a tab's content when it is shown, so Appearance never opens
    /// where Editor was left (gate day 7, 2026-11-09).
    items_scroll: ScrollHandle,
    /// The command whose ⊕ was clicked: the next chord pressed is its new
    /// binding, Escape backs out.
    capturing: Option<String>,
    /// The dropdown whose options are showing, by the key it writes. The
    /// accent picker is a popover like the dropdowns, under the accent key.
    menu: Option<&'static str>,
    /// The accent picker's own state while `menu` is the accent key.
    picker: Option<AccentPicker>,
    /// Which window corners the panel's columns round, read off the window
    /// each render: the nav takes the bottom-left, the content the
    /// bottom-right, under the frame's title row.
    corners: WindowCorners,
    /// The text field being typed into, by the key it writes. The rows are
    /// rebuilt from the vault on every render, so the open field is the
    /// view's, not the row's.
    editing: Option<TextField>,
    field: Entity<TextInput>,
    /// The dropdown whose [`Typed::local`] option was picked, by the key it
    /// writes: a pick the file cannot hold (the Daily notes tab's "Custom"),
    /// kept here so the rows it reveals stay until another option is picked.
    local_pick: Option<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> SettingsView {
        let hotkey_search = cx.new(|cx| TextInput::new("Filter...", cx));
        let snippet_search = cx.new(|cx| TextInput::new("Search snippets...", cx));
        let font_search = cx.new(|cx| TextInput::new("Enter font name...", cx));
        let field = cx.new(|cx| TextInput::new("", cx));
        let subscriptions = vec![
            cx.subscribe(
                &snippet_search,
                |this, _, event: &InputEvent, cx| match event {
                    InputEvent::Changed => cx.notify(),
                    InputEvent::Cancel => this.close_page(cx),
                    _ => {}
                },
            ),
            // Enter in the box picks nothing: Obsidian's did not either
            // (step s03); a family is added by clicking its row.
            cx.subscribe(
                &font_search,
                |this, _, event: &InputEvent, cx| match event {
                    InputEvent::Changed => cx.notify(),
                    InputEvent::Cancel => this.close_page(cx),
                    _ => {}
                },
            ),
            cx.subscribe(
                &hotkey_search,
                |this, _, event: &InputEvent, cx| match event {
                    InputEvent::Changed => this.refresh_hotkeys(cx),
                    InputEvent::Cancel => cx.emit(SettingsEvent::Close),
                    _ => {}
                },
            ),
            // One field serves whichever text row is open, since the rows
            // themselves are rebuilt from the vault on every render.
            cx.subscribe(&field, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Submit => this.commit_field(cx),
                InputEvent::Cancel => {
                    this.editing = None;
                    cx.notify();
                }
                _ => {}
            }),
        ];
        let mut view = SettingsView {
            focus_handle: cx.focus_handle(),
            state: None,
            tab: Tab::General,
            page: None,
            css: CssFolders::default(),
            snippet_search,
            font_search,
            font_scroll: UniformListScrollHandle::new(),
            app_icon: AppIcon::read(),
            hotkey_search,
            hotkey_filter: HotkeyFilter::All,
            all_commands: Vec::new(),
            hotkey_rows: Vec::new(),
            hotkey_scroll: UniformListScrollHandle::new(),
            items_scroll: ScrollHandle::new(),
            capturing: None,
            menu: None,
            picker: None,
            corners: WindowCorners::default(),
            editing: None,
            local_pick: None,
            field,
            _subscriptions: subscriptions,
        };
        view.load_commands(cx);
        view
    }

    fn load_commands(&mut self, cx: &mut Context<Self>) {
        let registry = cx.global::<Commands>().0.clone();
        let mut commands: Vec<Command> = registry
            .iter()
            .filter(|c| c.is_named() || c.custom)
            .cloned()
            .collect();
        commands.sort_by_key(|c| c.name.to_lowercase());
        self.all_commands = commands;
        self.refresh_hotkeys(cx);
    }

    pub fn set_state(&mut self, state: Arc<VaultState>, cx: &mut Context<Self>) {
        self.css = CssFolders::read(&state);
        self.state = Some(state);
        self.load_commands(cx);
        cx.notify();
    }

    /// Re-reads `themes/` and `snippets/`, which the CSS snippets page's
    /// reload button does and which every way into the modal does too: a
    /// file added or deleted outside the app shows up on the next one.
    fn reload_css_folders(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.state.clone() {
            self.css = CssFolders::read(&state);
        }
        cx.notify();
    }

    /// Opens a navigable row's page.
    fn open_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_field(cx);
        self.menu = None;
        self.reload_css_folders(cx);
        self.snippet_search
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.font_search
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.font_scroll = UniformListScrollHandle::new();
        self.page = Some(page);
        self.focus_tab(window, cx);
        cx.notify();
    }

    /// Back out of a page to the tab's own rows.
    fn close_page(&mut self, cx: &mut Context<Self>) {
        self.page = None;
        cx.notify();
    }

    /// Shows the modal on `tab` and takes focus so Escape closes it.
    pub fn open(&mut self, tab: Option<Tab>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = tab {
            self.tab = tab;
        }
        // The view outlives the modal, so a dropdown or a text field left
        // open by the last session would come back expanded. Every way out
        // of the modal commits the field first, so there is nothing to write
        // here, only state to drop.
        self.menu = None;
        self.editing = None;
        self.capturing = None;
        // A page left open by the last session would come back instead of
        // the tab it belongs to.
        self.page = None;
        // A filter typed last time would otherwise be what the next thing
        // typed appends to.
        self.hotkey_search
            .update(cx, |input, cx| input.set_text("", cx));
        self.snippet_search
            .update(cx, |input, cx| input.set_text("", cx));
        self.font_search
            .update(cx, |input, cx| input.set_text("", cx));
        // A theme or a snippet added since the modal was last open, and the
        // icon another window may have chosen; "Relaunch" stays once shown.
        self.reload_css_folders(cx);
        let changed = self.app_icon.changed;
        self.app_icon = AppIcon::read();
        self.app_icon.changed = changed;
        self.focus_tab(window, cx);
        cx.notify();
    }

    /// Which tab is open and what it has in hand, for the control socket's
    /// state dump. A dropdown and a text field are reported by the config key
    /// they write, which is how the view itself identifies them.
    pub fn describe(&self, rows: usize, cx: &App) -> Value {
        let listed = self.described_rows(rows, cx);
        let total = self.row_count();
        // A font page: what it lists, so a driver can check a pick landed.
        let font_page = match (self.page, self.state.as_ref()) {
            (Some(Page::Font(slot)), Some(state)) => {
                let applied = slot.read(&state.appearance());
                let query = self.font_search.read(cx).text().to_string();
                let (typed, offered) = font_choices(fonts::installed(), &applied, &query);
                json!({
                    "applied": applied,
                    "search": query,
                    "typed": typed,
                    "offered": offered.iter().take(rows).collect::<Vec<_>>(),
                    "offered_total": offered.len(),
                })
            }
            _ => Value::Null,
        };
        json!({
            "tab": self.tab.id(),
            "page": self.page.map(Page::title),
            "font_page": font_page,
            "menu": self.menu,
            "editing": self.editing.map(|field| field.key),
            "hotkey_search": self.hotkey_search.read(cx).text(),
            "capturing": self.capturing,
            "rows": listed,
            "rows_total": (total > rows).then_some(total),
        })
    }

    /// How many rows the open tab has, listed or not.
    fn row_count(&self) -> usize {
        match self.tab {
            Tab::Hotkeys => self.hotkey_rows.len(),
            tab => self.items(tab).len(),
        }
    }

    /// The open tab's rows, for the control socket's state dump: what each
    /// one is called, what kind of control it carries and where that
    /// control stands, and the tag saying how far Scoria gets with it.
    ///
    /// The Hotkeys tab is built from the command registry rather than from
    /// `items`, so it reports commands: the id, the label and the chords
    /// bound to it, `custom` when they came from `hotkeys.json`.
    fn described_rows(&self, rows: usize, _cx: &App) -> Vec<Value> {
        if self.tab == Tab::Hotkeys {
            return self
                .hotkey_rows
                .iter()
                .take(rows)
                .filter_map(|ix| self.all_commands.get(*ix))
                .map(|c| {
                    json!({
                        "kind": "command",
                        "id": c.id,
                        "name": c.name,
                        "hotkeys": c.hotkeys.iter().map(hotkey_label).collect::<Vec<_>>(),
                        "custom": c.custom,
                    })
                })
                .collect();
        }
        self.items(self.tab)
            .into_iter()
            .take(rows)
            .map(|item| {
                let (control, value) = match &item.control {
                    Control::None => ("none", Value::Null),
                    Control::Toggle(on) => ("toggle", json!(on)),
                    Control::Dropdown(v) => ("dropdown", json!(v.as_ref())),
                    Control::Text(v) => ("text", json!(v.as_ref())),
                    Control::Slider(v) => ("slider", json!(v.as_ref())),
                    Control::Button(v) => ("button", json!(v.as_ref())),
                    Control::Color(v) => ("color", json!(v.as_ref())),
                    Control::Navigable(v) => ("navigable", json!(v.as_ref())),
                    Control::AppIcon(v) => ("app_icon", json!(v.as_ref())),
                };
                json!({
                    "kind": if item.heading { "heading" } else { "setting" },
                    "name": item.name.as_ref(),
                    "desc": item.desc.as_ref(),
                    "control": control,
                    "value": value,
                    "tag": item.honored.tag(),
                })
            })
            .collect()
    }

    fn focus_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab == Tab::Hotkeys {
            window.focus(&self.hotkey_search.focus_handle(cx), cx);
        } else {
            window.focus(&self.focus_handle, cx);
        }
    }

    fn select_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        // A row that is no longer on screen cannot be committed or closed by
        // the user, so leaving the tab is one of the moments that writes.
        self.dismiss_open_controls(cx);
        self.tab = tab;
        self.items_scroll.set_offset(point(px(0.), px(0.)));
        // A page belongs to the tab that opened it.
        self.page = None;
        self.focus_tab(window, cx);
        cx.notify();
    }

    fn close_action(&mut self, _: &Close, window: &mut Window, cx: &mut Context<Self>) {
        match escape_closes(
            self.capturing.is_some(),
            self.menu.is_some(),
            self.page.is_some(),
        ) {
            // Escape while a chord is being captured backs out of the
            // capture, not the modal.
            Escape::Capture => {
                self.capturing = None;
                self.focus_tab(window, cx);
                cx.notify();
            }
            Escape::Popover => {
                self.menu = None;
                cx.notify();
            }
            Escape::Page => {
                self.dismiss_open_controls(cx);
                self.close_page(cx);
                self.focus_tab(window, cx);
            }
            Escape::Modal => {
                self.dismiss_open_controls(cx);
                cx.emit(SettingsEvent::Close);
            }
        }
    }

    /// Closes the window from its frame, where the panel's own key handler
    /// is out of reach: an open field is written first, as every other way
    /// out writes it.
    fn close_from_frame(&mut self, cx: &mut Context<Self>) {
        self.dismiss_open_controls(cx);
        cx.emit(SettingsEvent::Close);
    }

    // ----- hotkeys ----------------------------------------------------------

    /// Waits for the next chord to bind to `id`. The keyboard goes to the
    /// view's own handle so the filter field does not eat it.
    fn start_capture(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.capturing = Some(id);
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// The chord pressed while capturing, as the binding for that command.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.capturing.clone() else {
            return;
        };
        cx.stop_propagation();
        let Some(hotkey) = hotkey_from_keystroke(&event.keystroke) else {
            return;
        };
        self.capturing = None;
        self.focus_tab(window, cx);
        self.change_hotkeys(
            cx,
            |overrides, command| {
                let list = overrides
                    .entry(command.id.clone())
                    .or_insert_with(|| command.default_hotkeys.clone());
                if !list.contains(&hotkey) {
                    list.push(hotkey);
                }
            },
            &id,
        );
    }

    /// Takes one binding off a command. A default binding taken off writes
    /// the remaining defaults as the entry, and an entry left empty unbinds.
    fn remove_hotkey(&mut self, id: &str, hotkey: Hotkey, cx: &mut Context<Self>) {
        self.change_hotkeys(
            cx,
            |overrides, command| {
                let list = overrides
                    .entry(command.id.clone())
                    .or_insert_with(|| command.default_hotkeys.clone());
                list.retain(|h| *h != hotkey);
            },
            id,
        );
    }

    /// Drops the command's entry, so its defaults are back.
    fn restore_default_hotkeys(&mut self, id: &str, cx: &mut Context<Self>) {
        self.change_hotkeys(
            cx,
            |overrides, command| {
                overrides.shift_remove(&command.id);
            },
            id,
        );
    }

    /// Reads `hotkeys.json` as it stands, lets `edit` change the entry for
    /// `id`, and asks the workspace to write it. The file's own order is
    /// kept, so a vault's bindings do not shuffle the first time Scoria
    /// writes one.
    fn change_hotkeys(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Hotkeys, &Command),
        id: &str,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let Some(command) = self.all_commands.iter().find(|c| c.id == id).cloned() else {
            return;
        };
        let mut overrides = match state.vault().hotkeys() {
            Ok(overrides) => overrides,
            Err(err) => {
                // An unreadable file must not be rewritten from defaults.
                eprintln!("hotkeys.json could not be read: {err}");
                return;
            }
        };
        edit(&mut overrides, &command);
        cx.emit(SettingsEvent::Changed(Box::new(SettingChange::Hotkeys(
            overrides,
        ))));
    }

    /// How many commands each chord is bound to, so a duplicate can be
    /// marked the way Obsidian marks one.
    fn chord_counts(&self) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        for c in &self.all_commands {
            for h in &c.hotkeys {
                if let Some(k) = keystroke(h) {
                    *counts.entry(k).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    /// What a click that lands anywhere but on the control itself leaves
    /// behind: nothing. Obsidian's dropdown is a native `<select>`, which no
    /// outside click can survive, and its text settings write on every
    /// keystroke, so moving on is never a way to lose what was typed.
    fn dismiss_open_controls(&mut self, cx: &mut Context<Self>) {
        let had_menu = self.menu.take().is_some();
        if self.editing.is_some() {
            self.commit_field(cx);
        } else if had_menu {
            cx.notify();
        }
    }

    /// Whether this click was spent closing an open dropdown, and so must
    /// not also do whatever it landed on.
    ///
    /// Obsidian's dropdown is a native `<select>`, whose option list is the
    /// platform's own window: the click that dismisses it never reaches the
    /// page, so the toggle under it does not flip. Scoria's popover is part
    /// of the same window, so the rule has to be spelled out — every
    /// clickable in the modal asks this first, the popover's own option rows
    /// excepted, since a click there is the pick.
    fn click_dismissed_a_popover(&mut self, cx: &mut Context<Self>) -> bool {
        if !popover_swallows_click(&mut self.menu) {
            return false;
        }
        cx.notify();
        true
    }

    /// Makes the change a click on a control asks for, after the text
    /// field that click also closed has made its own.
    ///
    /// Every [`SettingChange`] carries a whole config file read from the
    /// vault as it stands, and the workspace writes the file for each one.
    /// A click on a control while a field is open changes two settings, so
    /// emitting both from the one click would build both from the config as
    /// it was before either landed, and the second write would drop the
    /// first. Committing the field now and deferring the control's own
    /// change puts that change after the field's write, which is what it has
    /// to be built on top of.
    fn after_open_field(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        if self.editing.is_some() {
            self.commit_field(cx);
            cx.defer_in(window, |this, _, cx| change(this, cx));
            return;
        }
        change(self, cx);
    }

    /// A click on a toggle: ask the workspace to write the flipped value.
    /// The modal redraws from the vault's config once the write lands, so a
    /// failed write leaves the control where it was.
    fn toggle_flag(&mut self, flag: Flag, window: &Window, cx: &mut Context<Self>) {
        self.after_open_field(window, cx, move |this, cx| {
            let Some(state) = this.state.clone() else {
                return;
            };
            cx.emit(SettingsEvent::Changed(Box::new(flag.toggled(&state))));
        });
    }

    fn refresh_hotkeys(&mut self, cx: &mut Context<Self>) {
        let query = self.hotkey_search.read(cx).text().trim().to_string();
        let filter = self.hotkey_filter;
        self.hotkey_rows = self
            .all_commands
            .iter()
            .enumerate()
            .filter(|(_, c)| filter.keeps(c))
            .filter(|(_, c)| query.is_empty() || fuzzy_match(&query, &c.name).is_some())
            .map(|(ix, _)| ix)
            .collect();
        cx.notify();
    }

    // ----- tab contents -------------------------------------------------

    fn items(&self, tab: Tab) -> Vec<Item> {
        let Some(state) = self.state.as_ref() else {
            return vec![item("No vault is open", "", Control::None)];
        };
        match tab {
            Tab::General => Self::general_items(state),
            Tab::Appearance => {
                Self::appearance_items(state, &self.css, self.app_icon.name.as_deref())
            }
            Tab::Interface => Self::interface_items(state),
            Tab::Editor => Self::editor_items(state),
            Tab::Files => Self::files_items(state),
            Tab::Hotkeys => Vec::new(),
            Tab::CorePlugins => Self::core_plugin_items(state),
            Tab::CommunityPlugins => Self::community_plugin_items(state),
            Tab::Plugin(id) => Self::plugin_items(state, &self.all_commands, id, self.local_pick),
        }
    }

    fn general_items(state: &VaultState) -> Vec<Item> {
        let vault = state.vault();
        vec![
            item(
                &format!("Version {}", env!("CARGO_PKG_VERSION")),
                "Scoria reads this vault the way Obsidian 1.14.4 does.",
                Control::Button("Check for updates".into()),
            ),
            dropdown("Language", "Change the display language.", "English").not_yet(),
            item(
                "Help",
                "Learn how to use Obsidian and get help from the community.",
                Control::Button("Open".into()),
            ),
            heading("Vault"),
            text("Vault name", "", vault.name()),
            text("Vault folder", "", vault.root().display().to_string()),
        ]
    }

    /// The Appearance tab, row for row as Obsidian 1.14.5 builds it
    /// (probed 2026-11-02, `appearance-tab.mjs` step s01): the scheme, the
    /// accent, the themes picker with the row that counts them, the
    /// navigable CSS snippets row, then the Font heading and its four
    /// rows.
    ///
    /// The three fonts are navigable: each opens the page of its families
    /// (`render_font_page`), and the row reads nothing, the one family, or
    /// "*N* fonts". Then "Quick font size adjustment", and under Advanced
    /// "Custom app icon", whose file name `icon` is passed in because it is
    /// the machine's (`scoria.json`), not the vault's (probed 2026-11-04,
    /// `font-pages.mjs`).
    fn appearance_items(state: &VaultState, css: &CssFolders, icon: Option<&str>) -> Vec<Item> {
        let a = &state.appearance();
        let font = |slot: FontSlot| {
            navigable(
                slot.title(),
                slot.desc(),
                fonts::summary(&slot.read(a)),
                Page::Font(slot),
            )
        };
        // The icon is applied from the next start, and only where the
        // window system takes one from the app (X11); elsewhere it is the
        // executable's own and the choice is only kept.
        let icon_row = Item {
            desc: if icon.is_some() {
                "".into()
            } else {
                "Set a custom icon for the app".into()
            },
            ..item(
                "Custom app icon",
                "",
                Control::AppIcon(icon.unwrap_or_default().to_string().into()),
            )
        };
        let icon_row = if cfg!(target_os = "linux") {
            icon_row.on_restart()
        } else {
            icon_row.not_yet()
        };
        // "N enabled" counts the enabled snippets whose file is still
        // there: a name left in `enabledCssSnippets` after its `.css` was
        // deleted does not count, and zero reads "0 enabled".
        let enabled = css.enabled_count(a);
        let installed = css.themes.len();
        let mut options: Vec<(SharedString, String)> = vec![("Default".into(), String::new())];
        options.extend(
            css.themes
                .iter()
                .map(|t| (SharedString::from(t.clone()), t.clone())),
        );
        vec![
            choice(
                "Base color scheme",
                "Choose Obsidian's default color scheme.",
                state,
                BASE_COLOR_SCHEME,
            ),
            Item {
                field: Some(ACCENT_COLOR),
                ..item(
                    "Accent color",
                    "Choose the accent color used throughout the app.",
                    Control::Color((ACCENT_COLOR.read)(state).into()),
                )
            },
            Item {
                folder: Some("themes"),
                ..picker(
                    "Themes",
                    "Manage installed themes and browse community themes.",
                    "cssTheme",
                    options,
                    a.css_theme.as_deref().unwrap_or_default(),
                )
            },
            item(
                "Current community themes",
                &format!(
                    "You currently have {installed} theme{} installed.",
                    if installed == 1 { "" } else { "s" }
                ),
                // Obsidian drops the button when nothing is installed.
                if installed == 0 {
                    Control::None
                } else {
                    Control::Button("Check for updates".into())
                },
            )
            .not_yet(),
            navigable(
                "CSS snippets",
                "Manage your collection of CSS files for tweaking the app's appearance.",
                format!("{enabled} enabled"),
                Page::CssSnippets,
            ),
            heading("Font"),
            font(FontSlot::Interface),
            font(FontSlot::Text),
            font(FontSlot::Monospace),
            slider(
                "Font size",
                "Font size in pixels that affects editing and reading views.",
                state,
                FONT_SIZE,
            ),
            appearance_toggle(
                "Quick font size adjustment",
                "Adjust the font size using Ctrl + Scroll, or using the trackpad pinch-zoom gesture.",
                a,
                QUICK_FONT_SIZE,
            ),
            heading("Advanced"),
            icon_row,
        ]
    }

    /// The rows of the CSS snippets page: one toggle per `.css` file under
    /// `snippets/`, in the folder's order, filtered by what is typed in
    /// the page's search box.
    fn snippet_items(state: &VaultState, css: &CssFolders, query: &str) -> Vec<Item> {
        let query = query.trim().to_lowercase();
        css.snippets
            .iter()
            .filter(|name| query.is_empty() || name.to_lowercase().contains(&query))
            .map(|name| {
                let flag = Flag::CssSnippet(name.clone());
                toggle(name, "", flag.is_on(state), flag)
            })
            .collect()
    }

    fn interface_items(state: &VaultState) -> Vec<Item> {
        let a = &state.appearance();
        vec![
            appearance_toggle(
                "Show tab title bar",
                "Display the header at the top of every tab.",
                a,
                SHOW_VIEW_HEADER,
            ).not_yet(),
            appearance_toggle(
                "Show ribbon",
                "Display vertical toolbar on the side of the window.",
                a,
                SHOW_RIBBON,
            ),
            heading("Advanced"),
            appearance_toggle(
                "Native menus",
                "Menus throughout the app will match the operating system. They will not be affected by your theme.",
                a,
                NATIVE_MENUS,
            ).not_yet(),
            // Obsidian keeps this one outside the vault: changing it wrote
            // nothing under `.obsidian` (probed 2026-10-20), so the pick
            // goes to `scoria.json` beside the vault list, and the next
            // start reads it; `--frame` and `SCORIA_FRAME` override it for
            // one run.
            choice(
                "Window frame style",
                "Determines the styling of the title bar of Obsidian windows. Requires a full restart to take effect.",
                state,
                FRAME_STYLE,
            )
            .on_restart(),
        ]
    }

    fn editor_items(state: &VaultState) -> Vec<Item> {
        let app = &state.app();
        let languages = app
            .spellcheck_languages
            .as_ref()
            .map(|l| l.join(", "))
            .unwrap_or_default();
        let editor_status = match state.core_plugins().as_ref() {
            Some(CorePlugins::Map(m)) => m.get("editor-status").copied().unwrap_or(true),
            Some(CorePlugins::List(l)) => l.iter().any(|x| x == "editor-status"),
            None => true,
        };
        vec![
            choice(
                "Default view for new tabs",
                "The default view that a new Markdown tab gets opened in.",
                state,
                DEFAULT_VIEW_MODE,
            ),
            choice(
                "Default editing mode",
                "The default editing mode a new tab will start with.",
                state,
                LIVE_PREVIEW,
            ),
            toggle(
                "Show editing mode in status bar",
                "Show the editing mode toggle in the status bar.",
                editor_status,
                Flag::CorePlugin("editor-status", true),
            ).not_yet(),
            heading("Display"),
            app_toggle(
                "Inline title",
                "Display the filename as an editable title inline with the file contents.",
                app,
                SHOW_INLINE_TITLE,
            ).not_yet(),
            app_toggle(
                "Readable line length",
                "Limit maximum line length. Less content fits onscreen, but long blocks of text are more readable.",
                app,
                READABLE_LINE_LENGTH,
            ),
            app_toggle(
                "Strict line breaks",
                "Markdown specs ignore single line breaks in reading view. Turn this off to make single line breaks visible.",
                app,
                STRICT_LINE_BREAKS,
            ),
            choice(
                "Properties in document",
                "Choose how properties are displayed at the top of notes. Select “source” to show properties as raw YAML.",
                state,
                PROPERTIES_IN_DOCUMENT,
            ),
            app_toggle(
                "Fold heading",
                "Lets you fold all content under a heading.",
                app,
                FOLD_HEADING,
            ).not_yet(),
            app_toggle(
                "Fold indent",
                "Lets you fold part of an indentation, such as lists.",
                app,
                FOLD_INDENT,
            ).not_yet(),
            app_toggle(
                "Line numbers",
                "Show line numbers in the gutter.",
                app,
                SHOW_LINE_NUMBER,
            ).not_yet(),
            app_toggle(
                "Indentation guides",
                "Show vertical relationship lines between list items.",
                app,
                SHOW_INDENT_GUIDE,
            ).not_yet(),
            app_toggle(
                "Right-to-left (RTL)",
                "Sets the default text direction of notes to right-to-left.",
                app,
                RIGHT_TO_LEFT,
            ).not_yet(),
            heading("Behavior"),
            app_toggle(
                "Spellcheck",
                "Turn on the spellchecker.",
                app,
                SPELLCHECK,
            ).not_yet(),
            text(
                "Spellcheck languages",
                "Choose the languages for the spellchecker to use.",
                languages,
            ).not_yet(),
            app_toggle(
                "Auto-pair brackets",
                "Pair brackets and quotes automatically.",
                app,
                AUTO_PAIR_BRACKETS,
            ),
            app_toggle(
                "Auto-pair Markdown syntax",
                "Pair symbols automatically for bold, italic, code, and more.",
                app,
                AUTO_PAIR_MARKDOWN,
            ),
            app_toggle(
                "Smart lists",
                "Automatically set indentation and place list items correctly.",
                app,
                SMART_INDENT_LIST,
            ),
            app_toggle(
                "Indent using tabs",
                "Use tabs to indent by pressing the “Tab” key. Turn this off to indent using 4 spaces.",
                app,
                USE_TAB,
            ),
            slider(
                "Indent visual width",
                "Number of spaces a tab character will render as.",
                state,
                TAB_SIZE,
            ),
            heading("Advanced"),
            app_toggle(
                "Convert pasted HTML to Markdown",
                "Automatically convert HTML to Markdown when pasting and drag-and-drop from web pages.",
                app,
                AUTO_CONVERT_HTML,
            ).not_yet(),
            app_toggle(
                "Vim key bindings",
                "Use Vim key bindings when editing.",
                app,
                VIM_MODE,
            ).not_yet(),
        ]
    }

    fn files_items(state: &VaultState) -> Vec<Item> {
        let app = &state.app();
        let excluded = app
            .user_ignore_filters
            .as_ref()
            .filter(|f| !f.is_empty())
            .map(|f| f.join(", "))
            .unwrap_or_else(|| "None".into());
        let config_dir = state.vault().config_dir().to_string();
        let mut items = vec![
            app_toggle(
                "Always focus new tabs",
                "When you open a link in a new tab, switch to it immediately.",
                app,
                FOCUS_NEW_TAB,
            )
            .not_yet(),
            choice(
                "Default location for new notes",
                "Where newly created notes are placed.",
                state,
                NEW_FILE_LOCATION,
            ),
        ];
        // Both location dropdowns reveal a row for the option that needs a
        // name typed into it, and hide it for the options that do not —
        // keyed off the selected option rather than off the stored string,
        // so the row appears the moment the option is picked.
        //
        // The attachment rows are read: `scoria_vault::ops::attachment_folder`
        // sends Insert attachment's copy and Download attachments' files where
        // each of the four shapes says, as Obsidian 1.15.0 did with them.
        if NEW_FILE_LOCATION.value(state) == "folder" {
            items.push(field(
                "Folder to create new notes in",
                "Newly created notes will appear in this folder.",
                state,
                NEW_FILE_FOLDER,
            ));
        }
        items.push(choice(
            "Default location for new attachments",
            "Where newly added attachments are placed.",
            state,
            ATTACHMENT_LOCATION,
        ));
        match ATTACHMENT_LOCATION.value(state) {
            "Attachments" => items.push(
                field(
                    "Attachment folder path",
                    "Place newly created attachment files, such as images created via drag-and-drop or audio recordings, in this folder.",
                    state,
                    ATTACHMENT_FOLDER,
                ),
            ),
            "./Attachments" => items.push(
                field(
                    "Subfolder name",
                    "If your file is in “vault/folder”, and you set subfolder name to “attachments”, attachments will be saved to “vault/folder/attachments”.",
                    state,
                    SUBFOLDER_NAME,
                ),
            ),
            _ => {}
        }
        items.extend([
            heading("Links"),
            choice(
                "New link format",
                "What links to insert when auto-generating internal links.",
                state,
                NEW_LINK_FORMAT,
            ),
            app_toggle(
                "Automatically update internal links",
                "Turn off to be prompted to update links after renaming a file.",
                app,
                ALWAYS_UPDATE_LINKS,
            ),
            app_toggle(
                "Use [[Wikilinks]]",
                "Auto-generate Wikilinks for [[links]] and ![[images]] instead of Markdown links and images.",
                app,
                USE_WIKILINKS,
            ).not_yet(),
            app_toggle(
                "Show all file types",
                "Show files with any extension even if Obsidian can't open them natively, so you can link to them and see them in the file explorer.",
                app,
                SHOW_UNSUPPORTED_FILES,
            ),
            heading("Trash"),
            app_toggle(
                "Confirm before deleting files",
                "Avoid accidentally deleting files.",
                app,
                PROMPT_DELETE,
            ),
            choice(
                "Deleted files",
                "What happens to a file after you delete it.",
                state,
                TRASH_OPTION,
            ),
            heading("Advanced"),
            text(
                "Excluded files",
                "Excluded files will be hidden in Search, Graph View, and Unlinked Mentions, less noticeable in Quick Switcher and link suggestions.",
                excluded,
            ).not_yet(),
            text(
                "Override config folder",
                "Use a different config folder than the default one. Must start with a dot.",
                if config_dir == ".obsidian" {
                    String::new()
                } else {
                    config_dir
                },
            ).not_yet(),
        ]);
        items
    }

    fn core_plugin_items(state: &VaultState) -> Vec<Item> {
        CORE_PLUGINS
            .iter()
            .map(|p| {
                toggle(
                    p.name,
                    p.description,
                    CorePlugins::is_enabled_or_default(state.core_plugins().as_ref(), p),
                    Flag::CorePlugin(p.id, p.default_on),
                )
                // Scoria's own Files, Search, Graph and the rest do not
                // turn off with the plugin that owns them in Obsidian; the
                // one thing that follows is which plugin tabs this modal
                // shows.
                .not_yet()
            })
            .collect()
    }

    fn community_plugin_items(state: &VaultState) -> Vec<Item> {
        let n = state.plugin_manifests.len();
        let mut items = vec![
            item(
                "Community plugins",
                "Browse and install community plugins made by the community.",
                Control::Button("Browse".into()),
            ),
            item(
                "Current plugins",
                &match n {
                    0 => "You currently have no plugins installed.".to_string(),
                    1 => "You currently have 1 plugin installed.".to_string(),
                    n => format!("You currently have {n} plugins installed."),
                },
                Control::Button("Check for updates".into()),
            ),
        ];
        if n > 0 {
            items.push(heading("Installed plugins"));
            for m in &state.plugin_manifests {
                let mut title = m.name.clone();
                if !m.version.is_empty() {
                    title.push_str(&format!(" v{}", m.version));
                }
                if !m.author.is_empty() {
                    title.push_str(&format!(" By {}", m.author));
                }
                items.push(
                    toggle(
                        &title,
                        &m.description,
                        state.community_plugins().contains(&m.id),
                        Flag::CommunityPlugin(m.id.clone()),
                    )
                    // Scoria runs no plugins; the list is Obsidian's.
                    .not_yet(),
                );
            }
        }
        items
    }

    fn plugin_items(
        state: &VaultState,
        commands: &[Command],
        id: &str,
        local_pick: Option<&'static str>,
    ) -> Vec<Item> {
        match id {
            // The two tabs' rows, labels and placeholders are 1.14.5's, see
            // the note above `DAILY_FORMAT`.
            "daily-notes" => {
                // "Custom" is selected when it was picked, or when the file
                // holds a format that is none of the four presets.
                let custom = local_pick == Some(DAILY_FORMAT.key)
                    || DAILY_FORMAT.value(state) == DAILY_FORMAT_CUSTOM;
                let mut items = vec![choice_at(
                    "Date format",
                    "Choose how daily note are named in your vault.",
                    DAILY_FORMAT,
                    if custom {
                        4
                    } else {
                        DAILY_FORMAT.selected(state)
                    },
                )];
                if custom {
                    items.push(field(
                        "Custom format",
                        &format!(
                            "For more syntax, refer to format reference\nYour current syntax looks like this: {}",
                            format_sample(&daily_format(state), DAILY_CUSTOM_FORMAT.placeholder)
                        ),
                        state,
                        DAILY_CUSTOM_FORMAT,
                    ));
                }
                items.push(field(
                    "New file location",
                    "New daily notes will be placed here.",
                    state,
                    DAILY_FOLDER,
                ));
                items.push(field(
                    "Template file location",
                    "Choose the file to use as a template.",
                    state,
                    DAILY_TEMPLATE,
                ));
                items
            }
            "templates" => {
                let t = state.templates();
                let date = t.date_format.clone().unwrap_or_default();
                let time = t.time_format.clone().unwrap_or_default();
                drop(t);
                vec![
                    field(
                        "Template folder location",
                        "Files in this folder will be available as templates.",
                        state,
                        TEMPLATES_FOLDER,
                    ),
                    field(
                        "Date format",
                        &format!(
                            "{{{{date}}}} in the template file will be replaced with this value.\nYou can also use {{{{date:YYYY-MM-DD}}}} to override the format once.\nFor more syntax, refer to format reference\nYour current syntax looks like this: {}",
                            format_sample(&date, TEMPLATES_DATE.placeholder)
                        ),
                        state,
                        TEMPLATES_DATE,
                    ),
                    field(
                        "Time format",
                        &format!(
                            "{{{{time}}}} in the template file will be replaced with this value.\nYou can also use {{{{time:HH:mm}}}} to override the format once.\nFor more syntax, refer to format reference\nYour current syntax looks like this: {}",
                            format_sample(&time, TEMPLATES_TIME.placeholder)
                        ),
                        state,
                        TEMPLATES_TIME,
                    ),
                ]
            }
            "switcher" => {
                let s = &state.switcher();
                vec![
                    switcher_toggle(
                        "Show existing only",
                        "Only show results from existing files. Links to files that are not yet created will be hidden.",
                        s,
                        SHOW_EXISTING_ONLY,
                    ),
                    switcher_toggle(
                        "Show attachments",
                        "Show attachment files like images, videos, and PDFs.",
                        s,
                        SHOW_ATTACHMENTS,
                    ),
                    switcher_toggle(
                        "Show all file types",
                        "Show all files including ones that Obsidian can't open. The file will be opened with the default app for that file type.",
                        s,
                        SHOW_ALL_FILE_TYPES,
                    ),
                ]
            }
            "command-palette" => {
                let registry = commands;
                let mut items = vec![heading("Pinned commands")];
                if state.palette.pinned.is_empty() {
                    items.push(item("No commands found.", "", Control::None));
                }
                for id in &state.palette.pinned {
                    let name = registry
                        .iter()
                        .find(|c| &c.id == id)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| id.clone());
                    items.push(item(&name, id, Control::None));
                }
                items
            }
            // The one row of the Backlinks tab: `backlinkInDocument`, which
            // shows the backlinks block at the foot of every leaf that has
            // not been turned off by `backlink:toggle-backlinks-in-document`
            // (the 2026-11-09 panes probe, `blsetting`).
            "backlink" => {
                let b = &state.backlink();
                vec![toggle(
                    "Show backlinks at the bottom of notes",
                    "Make backlinks visible in new tabs by default.",
                    BACKLINK_IN_DOCUMENT.is_on(b),
                    Flag::Backlink(BACKLINK_IN_DOCUMENT),
                )]
            }
            "canvas" => {
                let c = &state.canvas_options();
                let mut items = vec![choice(
                    "Default location for new canvas files",
                    "",
                    state,
                    CANVAS_NEW_FILE_LOCATION,
                )
                .not_yet()];
                // Obsidian hides this row for the other two options and
                // reveals it for `folder`, as its twin on the Files and
                // links tab does. No key was observed for it, so it shows
                // what the file holds -- nothing -- and writes nothing.
                if CANVAS_NEW_FILE_LOCATION.value(state) == "folder" {
                    items.push(
                        text(
                            "Folder to create new canvas files in",
                            "Newly created canvas files will appear under this folder.",
                            "",
                        )
                        .not_yet(),
                    );
                }
                items.push(
                    choice(
                        "Default mouse wheel behavior",
                        "",
                        state,
                        CANVAS_WHEEL_BEHAVIOR,
                    )
                    .not_yet(),
                );
                items.push(
                    choice(
                        "Default Ctrl + Drag behavior",
                        "",
                        state,
                        CANVAS_MOD_DRAG_BEHAVIOR,
                    )
                    .not_yet(),
                );
                items.push(choice("Show card names", "", state, CANVAS_CARD_LABELS).not_yet());
                items.push(
                    toggle(
                        "Snap to grid",
                        "Snap cards to the background grid when moving and resizing.",
                        SNAP_TO_GRID.is_on(c),
                        Flag::Canvas(SNAP_TO_GRID),
                    )
                    .not_yet(),
                );
                items.push(
                    toggle(
                        "Snap to objects",
                        "Snap cards to nearby objects when moving and resizing.",
                        SNAP_TO_OBJECTS.is_on(c),
                        Flag::Canvas(SNAP_TO_OBJECTS),
                    )
                    .not_yet(),
                );
                // Obsidian's slider runs -0.7 to 2.4 in steps of 0.01 at 0.
                // No key was observed, so the row shows that 0 and does
                // nothing; `Number` is whole numbers anyway.
                items.push(
                    item(
                        "Zoom threshold for hiding card content",
                        "Lower values will increase performance but hide card content sooner when zooming out.",
                        Control::Slider("0".into()),
                    )
                    .not_yet(),
                );
                items
            }
            // One toggle per surface the hover preview pops up over, under a
            // heading with no control of its own. Scoria has no hover
            // preview yet, so every row is tagged.
            "page-preview" => {
                let p = &state.page_preview();
                let row = |name: &str, flag: PagePreviewFlag| {
                    toggle(name, "", flag.is_on(p), Flag::PagePreview(flag)).not_yet()
                };
                vec![
                    heading("Require Ctrl to trigger page preview on hover"),
                    row("Search, Backlinks, and Outgoing links", PREVIEW_SEARCH),
                    row("Reading view", PREVIEW_READING),
                    row("Editing view", PREVIEW_EDITING),
                    row("Tab header", PREVIEW_TAB_HEADER),
                    row("Files", PREVIEW_FILES),
                    row("Properties view", PREVIEW_PROPERTIES),
                    row("Bookmarks", PREVIEW_BOOKMARKS),
                    row("Outline", PREVIEW_OUTLINE),
                    row("Bases", PREVIEW_BASES),
                    row("Graph view", PREVIEW_GRAPH),
                ]
            }
            "note-composer" => {
                let c = &state.note_composer();
                vec![
                    choice(
                        "Text after extraction",
                        "What to show in place of the selected text after extracting it.",
                        state,
                        COMPOSER_REPLACEMENT_TEXT,
                    ),
                    // A file suggester in Obsidian (`combobox-button
                    // mod-file`), drawn as a plain text field the way the
                    // Daily notes tab draws its own template path; what is
                    // typed is what the suggester stores, the path with
                    // `.md` (2026-11-09).
                    field(
                        "Template file location",
                        "Template file to use when merging or extracting. Available variables: {{content}}, {{fromTitle}}, {{newTitle}}, {{date:FORMAT}}, e.g. {{date:YYYY-MM-DD}}.",
                        state,
                        COMPOSER_TEMPLATE,
                    ),
                    toggle(
                        "Confirm file merge",
                        "Prompt before merge two files.",
                        ASK_BEFORE_MERGING.is_on(c),
                        Flag::NoteComposer(ASK_BEFORE_MERGING),
                    ),
                ]
            }
            // Three fields, each writing its own key (2026-11-09). The
            // format row's description ends in the sample Obsidian renders
            // from the pattern in the field, or from the default when it is
            // empty.
            "zk-prefixer" => {
                let format = (ZK_FORMAT.read)(state);
                vec![
                    field(
                        "New file location",
                        "The folder path to create the new unique note in.",
                        state,
                        ZK_FOLDER,
                    ),
                    field(
                        "Template file location",
                        "The file path to use as template.",
                        state,
                        ZK_TEMPLATE,
                    ),
                    field(
                        "Unique prefix format",
                        &format!(
                            "For more syntax, refer to format reference\nYour current syntax looks like this: {}",
                            format_sample(&format, ZK_FORMAT.placeholder)
                        ),
                        state,
                        ZK_FORMAT,
                    ),
                ]
            }
            // Two number fields, which write `file-recovery.json` (observed
            // 2026-11-09), and two buttons over the snapshots, which Scoria
            // does not keep: inert, as every button in this window is. The
            // fields are tagged too, Scoria taking no snapshots by them.
            "file-recovery" => vec![
                field(
                    "Snapshot interval",
                    "Minimal interval in minutes between two snapshots.",
                    state,
                    RECOVERY_INTERVAL,
                )
                .not_yet(),
                field(
                    "History length",
                    "Number of days the snapshots are kept for.",
                    state,
                    RECOVERY_KEEP_DAYS,
                )
                .not_yet(),
                item(
                    "Snapshots",
                    "View and restore saved snapshots.",
                    Control::Button("View".into()),
                )
                .not_yet(),
                item(
                    "Clear history",
                    "Delete all snapshots.",
                    Control::Button("Clear".into()),
                )
                .not_yet(),
            ],
            "webviewer" => {
                let w = &state.webviewer();
                vec![
                    toggle(
                        "Open external links",
                        "Open links in Obsidian rather than the system default.",
                        OPEN_EXTERNAL_URLS.is_on(w),
                        Flag::Webviewer(OPEN_EXTERNAL_URLS),
                    )
                    .not_yet(),
                    // Obsidian's row has no placeholder at all, and the key
                    // went unobserved.
                    text(
                        "Homepage",
                        "What page to view when a web viewer is opened",
                        "",
                    )
                    .not_yet(),
                    text(
                        "Saved page folder",
                        "Websites saved as markdown will be placed here.",
                        "",
                    )
                    .not_yet(),
                    choice(
                        "Search engine",
                        "Search engine used when searching from the address bar.",
                        state,
                        WEB_SEARCH_ENGINE,
                    )
                    .not_yet(),
                    field(
                        "Custom search engine",
                        "Search engine used when searching from the address bar.",
                        state,
                        WEB_CUSTOM_SEARCH_ENGINE,
                    )
                    .not_yet(),
                    toggle(
                        "Enable ad blocker",
                        "If enabled, the filter lists below will be applied to all web viewer tabs.",
                        ENABLE_ADBLOCKING.is_on(w),
                        Flag::Webviewer(ENABLE_ADBLOCKING),
                    )
                    .not_yet(),
                    // A textarea in Obsidian, holding its two EasyList URLs
                    // one per line. No key was observed and this window has
                    // no multi-line control, so the row shows the lists it
                    // ships with and writes nothing.
                    text(
                        "Ad blocking rules",
                        "One ad block list URL per line. Shared by all vaults.",
                        "https://easylist.to/easylist/easylist.txt\nhttps://easylist.to/easylist/easyprivacy.txt",
                    )
                    .not_yet(),
                    item(
                        "Ad block update frequency",
                        "Days between updating filter lists. 0 indicates disabled. Shared by all vaults.",
                        Control::Slider("4".into()),
                    )
                    .not_yet(),
                    item("Web viewer data", "", Control::Button("Clear web viewer data".into()))
                        .not_yet(),
                ]
            }
            // Obsidian's Sync tab had no `.setting-item` rows at all in the
            // probe, because the vault was not signed in: its options are
            // the remote vault's, and there was none. Scoria has no Sync, so
            // there is nothing to draw either way -- but the placeholder
            // ("Scoria does not read this plugin's settings file yet") would
            // say the wrong thing, since there is no settings file and no
            // row to read from one. This says what is actually true.
            "sync" => vec![item(
                "Sync",
                "Obsidian Sync is a paid service of Obsidian's, and Scoria does not speak it. Obsidian shows this tab's options only once a vault is signed in.",
                Control::None,
            )
            .not_yet()],
            _ => vec![item(
                "Options not shown yet",
                "Scoria does not read this plugin's settings file yet.",
                Control::None,
            )],
        }
    }

    // ----- rendering ----------------------------------------------------

    fn render_nav(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let group_title = |label: &'static str, theme: &Theme| {
            div()
                .px_3()
                .pt_3()
                .pb_1()
                .text_size(px(11.))
                .text_color(theme.tokens.text_faint)
                .child(label.to_uppercase())
        };
        let mut nav = div()
            .id("settings-nav")
            .flex()
            .flex_col()
            // A short window clips the tabs below Core plugins, and the
            // root is `overflow_hidden`: the nav scrolls for the same
            // reason the content list does.
            .overflow_y_scroll()
            .w(px(220.))
            .h_full()
            .flex_shrink_0()
            // `.vertical-tab-header`: `--modal-sidebar-background`, the
            // modal's own ground rather than the sidebar colour.
            .bg(theme.tokens.modal_sidebar_background)
            .map(|d| self.corners.left().apply(d))
            .border_r_1()
            .border_color(theme.tokens.background_modifier_border)
            .py_2()
            // Obsidian's sidebar opens straight on "Options"; the mark and
            // the version above it are ours, and say which program wrote
            // the `.obsidian/` the rest of this modal is editing.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .pt_1()
                    .pb_2()
                    .child(logo(52.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                // The product name as the Vaults window sets
                                // it: the display face at its one weight.
                                div()
                                    .font_family(DISPLAY_FONT_FAMILY)
                                    .font_weight(FontWeight::BLACK)
                                    .text_size(px(22.))
                                    .line_height(px(24.))
                                    .text_color(theme.tokens.text_normal)
                                    .child("Scoria"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme.tokens.text_faint)
                                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                            ),
                    ),
            )
            .child(group_title("Options", theme));
        for tab in Tab::OPTIONS {
            nav = nav.child(self.nav_item(*tab, theme, cx));
        }
        let plugin_tabs: Vec<&'static str> = match &self.state {
            Some(state) => PLUGIN_TABS
                .iter()
                .filter(|(id, _)| {
                    CORE_PLUGINS.iter().find(|p| p.id == *id).is_some_and(|p| {
                        CorePlugins::is_enabled_or_default(state.core_plugins().as_ref(), p)
                    })
                })
                .map(|(id, _)| *id)
                .collect(),
            None => Vec::new(),
        };
        if !plugin_tabs.is_empty() {
            nav = nav.child(group_title("Core plugins", theme));
            for id in plugin_tabs {
                nav = nav.child(self.nav_item(Tab::Plugin(id), theme, cx));
            }
        }
        nav
    }

    fn nav_item(
        &self,
        tab: Tab,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let active = self.tab == tab;
        div()
            .id(SharedString::from(format!("settings-nav-{}", tab.name())))
            .mx_2()
            .px_2()
            .py(px(5.))
            .rounded_sm()
            .cursor_pointer()
            .text_size(px(13.))
            // `.vertical-tab-nav-item`: the text colour whether active or
            // not; the active one sits on the hover wash (probe of
            // 2026-11-02, `settings.nav-item`).
            .text_color(theme.tokens.text_normal)
            .when(active, |d| d.bg(theme.tokens.background_modifier_hover))
            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.click_dismissed_a_popover(cx) {
                    return;
                }
                this.select_tab(tab, window, cx)
            }))
            .child(tab.name())
    }

    fn render_control(
        &self,
        item: &Item,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match &item.control {
            Control::None => div().into_any_element(),
            Control::Toggle(on) => {
                let on = *on;
                let toggle = div()
                    .relative()
                    .w(px(36.))
                    .h(px(20.))
                    .rounded_full()
                    // `.checkbox-container`: `--background-modifier-border-hover`,
                    // `--interactive-accent` while on (probe of 2026-11-02).
                    .bg(if on {
                        theme.tokens.interactive_accent
                    } else {
                        theme.tokens.background_modifier_border_hover
                    })
                    .border_1()
                    .border_color(if on {
                        theme.tokens.interactive_accent
                    } else {
                        theme.tokens.background_modifier_border_hover
                    })
                    .child(
                        div()
                            .absolute()
                            .top(px(2.))
                            .left(px(if on { 17. } else { 2. }))
                            .size(px(14.))
                            .rounded_full()
                            .bg(theme.tokens.toggle_thumb_color),
                    );
                let Some(flag) = item.flag.clone() else {
                    return toggle.into_any_element();
                };
                div()
                    .id(SharedString::from(format!("settings-toggle-{}", item.name)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.click_dismissed_a_popover(cx) {
                            return;
                        }
                        this.toggle_flag(flag.clone(), window, cx)
                    }))
                    .child(toggle)
                    .into_any_element()
            }
            // A dropdown whose options are the vault's: the same control
            // as `Control::Dropdown` with a `Typed`, reading and writing
            // through the row's `Picker` instead of a static table.
            Control::Dropdown(value) if item.picker.is_some() => {
                let pick = item.picker.clone().expect("checked above");
                let shown = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py(px(3.))
                    .rounded_sm()
                    .bg(theme.tokens.dropdown_background)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .text_size(px(12.))
                    .text_color(theme.tokens.text_normal)
                    .child(value.clone())
                    .child(div().text_color(theme.tokens.text_muted).child("▾"));
                let open = self.menu == Some(pick.key);
                let selected = pick.selected;
                div()
                    .relative()
                    .child(
                        div()
                            .id(SharedString::from(format!("settings-menu-{}", pick.key)))
                            .cursor_pointer()
                            .on_click(cx.listener({
                                let key = pick.key;
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    if this.click_dismissed_a_popover(cx) {
                                        return;
                                    }
                                    this.commit_field(cx);
                                    this.menu = Some(key);
                                    cx.notify();
                                }
                            }))
                            .child(shown),
                    )
                    .when(open, |d| {
                        d.child(deferred(
                            div()
                                .absolute()
                                .top(px(26.))
                                .right(px(0.))
                                .min_w(px(200.))
                                .flex()
                                .flex_col()
                                .bg(theme.tokens.background_secondary)
                                .border_1()
                                .border_color(theme.tokens.background_modifier_border)
                                .rounded_md()
                                .shadow_lg()
                                .py_1()
                                .children(pick.options.iter().enumerate().map(
                                    |(ix, (label, value))| {
                                        let value = value.clone();
                                        div()
                                            .id(SharedString::from(format!(
                                                "settings-option-{}-{ix}",
                                                pick.key
                                            )))
                                            .px_3()
                                            .py_1()
                                            .text_size(px(12.))
                                            .cursor_pointer()
                                            .when(ix == selected, |d| {
                                                d.bg(theme.tokens.background_modifier_hover)
                                            })
                                            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                                            .text_color(theme.tokens.text_normal)
                                            .child(label.clone())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.pick_theme(&value, window, cx)
                                            }))
                                    },
                                )),
                        ))
                    })
                    .into_any_element()
            }
            Control::Navigable(value) => {
                let Some(page) = item.page else {
                    return div().into_any_element();
                };
                div()
                    .id(SharedString::from(format!("settings-page-{}", item.name)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(theme.tokens.text_muted)
                    .child(value.clone())
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(theme.tokens.text_faint)
                            .child("›"),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if this.click_dismissed_a_popover(cx) {
                            return;
                        }
                        this.open_page(page, window, cx)
                    }))
                    .into_any_element()
            }
            // The accent row, open for typing with no picker up: the same
            // field every other text row uses. With the picker up the
            // field is inside it instead.
            Control::Color(_)
                if self.menu != Some(ACCENT_COLOR.key)
                    && item
                        .field
                        .zip(self.editing)
                        .is_some_and(|(a, b)| a.same(&b)) =>
            {
                div()
                    .id("settings-accent-open")
                    .min_w(px(160.))
                    .max_w(px(220.))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(self.field.clone())
                    .into_any_element()
            }
            Control::Color(value) => {
                let stored = value.clone();
                let swatch =
                    crate::theme::accent_swatch(&stored).unwrap_or(theme.tokens.interactive_accent);
                let at_default = stored.is_empty();
                let open = self.menu == Some(ACCENT_COLOR.key);
                let popover = open.then(|| self.render_picker(theme, cx));
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .children(popover)
                    // "Restore default", a `lucide-rotate-ccw` clickable
                    // icon that Obsidian disables while nothing is set.
                    .child(
                        div()
                            .id("settings-accent-reset")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.))
                            .rounded_sm()
                            .text_color(if at_default {
                                theme.tokens.text_faint
                            } else {
                                theme.tokens.icon_color
                            })
                            .when(!at_default, |d| {
                                d.cursor_pointer()
                                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                            })
                            .child(crate::assets::icon(
                                "rotate-ccw",
                                16.,
                                theme.tokens.icon_color,
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                if this.click_dismissed_a_popover(cx) {
                                    return;
                                }
                                this.reset_accent(window, cx)
                            })),
                    )
                    .child(
                        div()
                            .id("settings-accent-swatch")
                            .size(px(22.))
                            .rounded_full()
                            .bg(swatch)
                            .border_1()
                            .border_color(theme.tokens.background_modifier_border)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                if this.click_dismissed_a_popover(cx) {
                                    return;
                                }
                                this.open_picker(window, cx)
                            })),
                    )
                    .into_any_element()
            }
            Control::Dropdown(value) => {
                let shown = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py(px(3.))
                    .rounded_sm()
                    // `select.dropdown`: `--text-normal` on
                    // `--dropdown-background`.
                    .bg(theme.tokens.dropdown_background)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .text_size(px(12.))
                    .text_color(theme.tokens.text_normal)
                    .child(value.clone())
                    .child(div().text_color(theme.tokens.text_muted).child("▾"));
                let Some(typed) = item.typed else {
                    return shown.into_any_element();
                };
                let selected = self.selected_of(typed);
                let open = self.menu == Some(typed.key);
                div()
                    .relative()
                    .child(
                        div()
                            .id(SharedString::from(format!("settings-menu-{}", typed.key)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                // The root closes whatever is open on any
                                // click; this one has to survive it, or the
                                // menu would shut again as the click bubbles.
                                cx.stop_propagation();
                                // A click with a popover open is spent
                                // closing it — this one's own second click
                                // included, which is how the property type
                                // menu closes too.
                                if this.click_dismissed_a_popover(cx) {
                                    return;
                                }
                                this.commit_field(cx);
                                this.menu = Some(typed.key);
                                cx.notify();
                            }))
                            .child(shown),
                    )
                    .when(open, |d| {
                        // Deferred, or the rows after this one paint over
                        // the options: a later sibling wins otherwise.
                        d.child(deferred(
                            div()
                                .absolute()
                                .top(px(26.))
                                .right(px(0.))
                                .min_w(px(200.))
                                .flex()
                                .flex_col()
                                .bg(theme.tokens.background_secondary)
                                .border_1()
                                .border_color(theme.tokens.background_modifier_border)
                                .rounded_md()
                                .shadow_lg()
                                .py_1()
                                .children(typed.options.iter().enumerate().map(|(ix, _)| {
                                    div()
                                        .id(SharedString::from(format!(
                                            "settings-option-{}-{ix}",
                                            typed.key
                                        )))
                                        .px_3()
                                        .py_1()
                                        .text_size(px(12.))
                                        .cursor_pointer()
                                        .when(ix == selected, |d| {
                                            d.bg(theme.tokens.background_modifier_hover)
                                        })
                                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                                        .text_color(theme.tokens.text_normal)
                                        .child(SharedString::from(typed.option_label(ix)))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.pick(typed, ix, window, cx)
                                        }))
                                })),
                        ))
                    })
                    .into_any_element()
            }
            Control::Text(_)
                if item
                    .field
                    .zip(self.editing)
                    .is_some_and(|(a, b)| a.same(&b)) =>
            {
                div()
                    .id("settings-field-open")
                    .min_w(px(220.))
                    .max_w(px(320.))
                    // A click inside the open input is not a click away from
                    // it, which is what the root would read it as.
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(self.field.clone())
                    .into_any_element()
            }
            Control::Text(value) if item.field.is_some() => {
                let field = item.field.expect("checked above");
                div()
                    .id(SharedString::from(format!("settings-field-{}", field.key)))
                    .min_w(px(160.))
                    .max_w(px(320.))
                    .px_2()
                    .py(px(3.))
                    .rounded_sm()
                    // A text input: `--background-modifier-form-field` inside
                    // `--background-modifier-border`.
                    .bg(theme.tokens.background_modifier_form_field)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .text_size(px(12.))
                    .cursor_text()
                    .text_color(if value.is_empty() {
                        theme.tokens.text_faint
                    } else {
                        theme.tokens.text_normal
                    })
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(if value.is_empty() {
                        SharedString::from(field.placeholder)
                    } else {
                        value.clone()
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        // Without this the click would bubble to the root and
                        // be read as a click away from the field it just
                        // opened, closing it again on the same frame.
                        cx.stop_propagation();
                        if this.click_dismissed_a_popover(cx) {
                            return;
                        }
                        this.edit_field(field, window, cx)
                    }))
                    .into_any_element()
            }
            Control::Text(value) => div()
                .min_w(px(160.))
                .max_w(px(320.))
                .px_2()
                .py(px(3.))
                .rounded_sm()
                .bg(theme.tokens.background_modifier_form_field)
                .border_1()
                .border_color(theme.tokens.background_modifier_border)
                .text_size(px(12.))
                .text_color(if value.is_empty() {
                    theme.tokens.text_faint
                } else {
                    theme.tokens.text_normal
                })
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(if value.is_empty() {
                    SharedString::from("Empty")
                } else {
                    value.clone()
                })
                .into_any_element(),
            Control::Slider(value) => {
                let Some(number) = item.number else {
                    return div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(120.))
                                .h(px(4.))
                                .rounded_full()
                                .bg(theme.tokens.slider_track_background),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.tokens.text_muted)
                                .child(value.clone()),
                        )
                        .into_any_element();
                };
                let now = self.state.as_ref().map(|s| (number.read)(s)).unwrap_or(0);
                // One cell per step: the ranges are small, so a click lands
                // on a value without measuring the track.
                let steps = (number.min..=number.max).collect::<Vec<_>>();
                let width = (120. / steps.len() as f32).max(3.);
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().flex().flex_row().items_center().h(px(14.)).children(
                        steps.into_iter().map(|step| {
                            div()
                                .id(SharedString::from(format!(
                                    "settings-step-{}-{step}",
                                    number.key
                                )))
                                .w(px(width))
                                .h(px(4.))
                                .cursor_pointer()
                                .bg(if step <= now {
                                    theme.tokens.slider_fill_background
                                } else {
                                    theme.tokens.slider_track_background
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if this.click_dismissed_a_popover(cx) {
                                        return;
                                    }
                                    this.set_number(number, step, window, cx)
                                }))
                        }),
                    ))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.tokens.text_muted)
                            .child(value.clone()),
                    )
                    .into_any_element()
            }
            // A button: `--text-normal` on `--interactive-normal`.
            Control::Button(label) => div()
                .px_3()
                .py(px(3.))
                .rounded_sm()
                .bg(theme.tokens.interactive_normal)
                .border_1()
                .border_color(theme.tokens.background_modifier_border)
                .text_size(px(12.))
                .text_color(theme.tokens.text_normal)
                .child(label.clone())
                .into_any_element(),
            Control::AppIcon(name) => {
                let chosen = !name.is_empty();
                let button = |id: &'static str, label: &'static str, cta: bool| {
                    div()
                        .id(id)
                        .px_3()
                        .py(px(3.))
                        .rounded_sm()
                        .cursor_pointer()
                        .bg(if cta {
                            theme.tokens.interactive_accent
                        } else {
                            theme.tokens.interactive_normal
                        })
                        .border_1()
                        .border_color(if cta {
                            theme.tokens.interactive_accent
                        } else {
                            theme.tokens.background_modifier_border
                        })
                        .text_size(px(12.))
                        .text_color(if cta {
                            theme.tokens.text_on_accent
                        } else {
                            theme.tokens.text_normal
                        })
                        .child(label)
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    // Hidden, not disabled, at the default (step s08).
                    .when(chosen, |d| {
                        d.child(
                            div()
                                .id("settings-icon-reset")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(24.))
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                                .tooltip(crate::canvas::menu::tooltip("Restore default"))
                                .child(crate::assets::icon(
                                    "rotate-ccw",
                                    16.,
                                    theme.tokens.icon_color,
                                ))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    if this.click_dismissed_a_popover(cx) {
                                        return;
                                    }
                                    this.restore_app_icon(cx)
                                })),
                        )
                    })
                    .child(
                        button("settings-icon-choose", "Choose", false).on_click(cx.listener(
                            |this, _, window, cx| {
                                cx.stop_propagation();
                                if this.click_dismissed_a_popover(cx) {
                                    return;
                                }
                                this.choose_app_icon(window, cx)
                            },
                        )),
                    )
                    .when(self.app_icon.changed, |d| {
                        d.child(button("settings-icon-relaunch", "Relaunch", true).on_click(
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.relaunch(cx)
                            }),
                        ))
                    })
                    .into_any_element()
            }
        }
    }

    /// "Choose" on the Custom app icon row: the desktop's file dialog, then
    /// the file copied beside `scoria.json` as `icon.<ext>` and recorded
    /// there, the way Obsidian copies it into its own user-data folder. Its
    /// dialog filters to PNG, ICO and ICNS; gpui's has no filter, so another
    /// kind is refused after the pick.
    fn choose_app_icon(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Custom app icon".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let source = match picked.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.remove(0),
                Ok(Err(err)) => {
                    this.update(cx, |_, cx| {
                        cx.emit(SettingsEvent::Report(format!(
                            "Could not open the file dialog: {err}"
                        )))
                    })
                    .ok();
                    return;
                }
                _ => return,
            };
            let result = scoria_vault::registry::VaultRegistry::open()
                .and_then(|r| {
                    r.ok_or_else(|| {
                        std::io::Error::other("there is no configuration folder to keep it in")
                    })
                })
                .and_then(|mut r| r.install_icon(&source));
            this.update(cx, |this, cx| match result {
                Ok(_) => {
                    this.app_icon = AppIcon::read();
                    this.app_icon.changed = true;
                    cx.notify();
                }
                Err(err) => cx.emit(SettingsEvent::Report(format!(
                    "The app icon could not be set: {err}"
                ))),
            })
            .ok();
        })
        .detach();
    }

    /// "Restore default" on the Custom app icon row: the copy deleted and
    /// the key dropped from `scoria.json`.
    fn restore_app_icon(&mut self, cx: &mut Context<Self>) {
        let result = scoria_vault::registry::VaultRegistry::open().and_then(|r| match r {
            Some(mut r) => r.clear_icon(),
            None => Ok(()),
        });
        match result {
            Ok(()) => {
                self.app_icon = AppIcon::read();
                self.app_icon.changed = true;
            }
            Err(err) => cx.emit(SettingsEvent::Report(format!(
                "The app icon could not be restored: {err}"
            ))),
        }
        cx.notify();
    }

    /// "Relaunch": a new Scoria on this vault, in a window of its own so it
    /// does not hand itself back to this one, then this one quits.
    fn relaunch(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let spawned = std::env::current_exe().and_then(|exe| {
            std::process::Command::new(exe)
                .arg(&state.root)
                .arg("--new-window")
                .spawn()
        });
        match spawned {
            Ok(_) => cx.quit(),
            Err(err) => cx.emit(SettingsEvent::Report(format!(
                "Scoria could not be relaunched: {err}"
            ))),
        }
    }

    /// Opens the dropdown that writes `key`, for `--menu`: clicking one is
    /// what does it in the app.
    pub fn open_menu(&mut self, key: &str, cx: &mut Context<Self>) {
        self.menu = TYPED_ALL.iter().find(|t| t.key == key).map(|t| t.key);
        cx.notify();
    }

    /// Writes a slider's value, which the control also reads through.
    /// Opens a text field for typing, with what the vault holds in it.
    /// Opens the accent picker under the swatch, with the hex field inside
    /// it holding what the vault stores.
    fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let accent = cx.global::<Theme>().tokens.interactive_accent;
        let stored = (ACCENT_COLOR.read)(&state);
        self.picker = Some(AccentPicker::new(&stored, accent));
        // The field closes any menu; the picker is the menu, so it is
        // named after the field is up.
        self.edit_field(ACCENT_COLOR, window, cx);
        self.menu = Some(ACCENT_COLOR.key);
        cx.notify();
    }

    /// Writes the picker's colour: the hex goes to the file and into the
    /// hex field, so the field's own commit later finds nothing to change.
    fn write_picked(&mut self, hex: String, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        if let Some(picker) = &mut self.picker {
            picker.from = normalize_accent(&hex);
        }
        self.field
            .update(cx, |input, cx| input.set_text(hex.clone(), cx));
        if (ACCENT_COLOR.read)(&state) != hex {
            cx.emit(SettingsEvent::Changed(Box::new((ACCENT_COLOR.write)(
                &state, &hex,
            ))));
        }
        cx.notify();
    }

    /// A press on the square or the bar: the colour moves to the point at
    /// once and follows the mouse until it is let go.
    fn picker_press(&mut self, part: PickerPart, at: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(picker) = &mut self.picker {
            picker.drag = Some(part);
        }
        self.picker_move(at, cx);
    }

    fn picker_move(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        match picker.drag {
            Some(PickerPart::Square) => {
                let (s, v) = square_pick(picker.square, at);
                picker.hsv.s = s;
                picker.hsv.v = v;
            }
            Some(PickerPart::Bar) => picker.hsv.h = bar_pick(picker.bar, at),
            None => return,
        }
        cx.notify();
    }

    /// Letting go is the write: one file per pick rather than one per
    /// pixel the mouse crossed.
    fn picker_release(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        if picker.drag.take().is_none() {
            return;
        }
        let hex = picker.hsv.hex();
        self.write_picked(hex, cx);
    }

    /// A preset: the empty one is "Restore default", the rest are hexes.
    fn pick_preset(&mut self, hex: &'static str, window: &Window, cx: &mut Context<Self>) {
        if hex.is_empty() {
            self.reset_accent(window, cx);
            return;
        }
        if let Some(picker) = &mut self.picker {
            if let Some(hsv) = Hsv::of_hex(hex) {
                picker.hsv = hsv;
            }
        }
        self.write_picked(hex.to_string(), cx);
    }

    /// The picker: a saturation and value square under a hue bar, the
    /// presets, and the hex field beside a swatch of the colour as it
    /// stands. Deferred and absolute like the dropdowns' option lists, and
    /// every press in it stops at it, so the root's click does not close
    /// it and the drag's own mouse-up is the write.
    fn render_picker(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(picker) = &self.picker else {
            return div().into_any_element();
        };
        let hsv = picker.hsv;
        let (w, h) = PICKER_SQUARE;
        let view = cx.entity().clone();
        let square_bounds = view.clone();
        let bar_bounds = view;
        let black = hsla(0., 0., 0., 1.);
        let clear = hsla(0., 0., 0., 0.);
        let white = hsla(0., 0., 1., 1.);
        let ring = div()
            .absolute()
            .size(px(12.))
            .rounded_full()
            .border_2()
            .border_color(white)
            .shadow_sm()
            .left(px(hsv.s * w - 6.))
            .top(px((1. - hsv.v) * h - 6.));
        let square = div()
            .id("settings-accent-square")
            .relative()
            .w(px(w))
            .h(px(h))
            .rounded_sm()
            .overflow_hidden()
            .bg(linear_gradient(
                90.,
                linear_color_stop(white, 0.),
                linear_color_stop(hsv.pure(), 1.),
            ))
            .cursor(gpui::CursorStyle::Crosshair)
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(clear, 0.),
                linear_color_stop(black, 1.),
            )))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        square_bounds.update(cx, |this, _| {
                            if let Some(p) = &mut this.picker {
                                p.square = bounds;
                            }
                        })
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(ring)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.picker_press(PickerPart::Square, e.position, cx)
                }),
            );
        let bar = div()
            .id("settings-accent-bar")
            .relative()
            .w(px(w))
            .h(px(PICKER_BAR))
            .rounded_sm()
            .overflow_hidden()
            .flex()
            .flex_row()
            .cursor(gpui::CursorStyle::Crosshair)
            // Six two-stop runs make the wheel: gpui's gradient takes two
            // colours, and red to red through five more is six of them.
            .children((0..6).map(|i| {
                div().flex_1().h_full().bg(linear_gradient(
                    90.,
                    linear_color_stop(hsla(i as f32 / 6., 1., 0.5, 1.), 0.),
                    linear_color_stop(hsla((i + 1) as f32 / 6., 1., 0.5, 1.), 1.),
                ))
            }))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        bar_bounds.update(cx, |this, _| {
                            if let Some(p) = &mut this.picker {
                                p.bar = bounds;
                            }
                        })
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(
                div()
                    .absolute()
                    .top(px(-1.))
                    .w(px(4.))
                    .h(px(PICKER_BAR + 2.))
                    .rounded_sm()
                    .border_2()
                    .border_color(white)
                    .left(px((hsv.h / 360. * w - 2.).clamp(0., w - 4.))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.picker_press(PickerPart::Bar, e.position, cx)
                }),
            );
        let presets = div().flex().flex_row().flex_wrap().gap_1().children(
            PICKER_PRESETS.iter().enumerate().map(|(ix, (name, hex))| {
                let colour = Hsv::of_hex(hex)
                    .map(Hsv::hsla)
                    .unwrap_or(theme.tokens.interactive_accent);
                div()
                    .id(SharedString::from(format!("settings-accent-preset-{ix}")))
                    .size(px(18.))
                    .rounded_full()
                    .bg(colour)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .cursor_pointer()
                    .hover(|s| s.border_color(theme.tokens.text_normal))
                    .tooltip(crate::canvas::menu::tooltip(*name))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.pick_preset(hex, window, cx)
                    }))
            }),
        );
        let live = div()
            .size(px(22.))
            .rounded_full()
            .bg(hsv.hsla())
            .border_1()
            .border_color(theme.tokens.background_modifier_border);
        deferred(
            div()
                .id("settings-accent-picker")
                // Occluding, so the row under it is not hovered through
                // it: the Themes folder button's tooltip rose through
                // the square otherwise.
                .occlude()
                .absolute()
                .top(px(28.))
                .right(px(0.))
                .w(px(w + 24.))
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .bg(theme.tokens.background_secondary)
                .border_1()
                .border_color(theme.tokens.background_modifier_border)
                .rounded_md()
                .shadow_lg()
                .on_click(|_, _, cx| cx.stop_propagation())
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                    if e.pressed_button == Some(MouseButton::Left) {
                        this.picker_move(e.position, cx)
                    }
                }))
                .capture_any_mouse_up(
                    cx.listener(|this, _: &MouseUpEvent, _, cx| this.picker_release(cx)),
                )
                .child(square)
                .child(bar)
                .child(presets)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(self.field.clone()))
                        .child(live),
                ),
        )
        .into_any_element()
    }

    fn edit_field(&mut self, field: TextField, window: &mut Window, cx: &mut Context<Self>) {
        // One input serves every row, so opening the next one is the last
        // moment the one before it can be written.
        self.commit_field(cx);
        let Some(state) = self.state.clone() else {
            return;
        };
        let value = (field.read)(&state);
        self.editing = Some(field);
        self.menu = None;
        self.field.update(cx, |input, cx| {
            input.set_placeholder(field.placeholder);
            input.set_text(value, cx);
        });
        window.focus(&self.field.focus_handle(cx), cx);
        cx.notify();
    }

    /// Writes what was typed, which the row then reads back.
    fn commit_field(&mut self, cx: &mut Context<Self>) {
        let Some(field) = self.editing.take() else {
            return;
        };
        let Some(state) = self.state.clone() else {
            return;
        };
        let typed = self.field.read(cx).text().to_string();
        if (field.read)(&state) != typed {
            cx.emit(SettingsEvent::Changed(Box::new((field.write)(
                &state, &typed,
            ))));
        }
        cx.notify();
    }

    fn set_number(&mut self, number: Number, value: i64, window: &Window, cx: &mut Context<Self>) {
        self.after_open_field(window, cx, move |this, cx| {
            let Some(state) = this.state.clone() else {
                return;
            };
            let value = value.clamp(number.min, number.max);
            if (number.read)(&state) != value {
                cx.emit(SettingsEvent::Changed(Box::new((number.write)(
                    &state, value,
                ))));
            }
            cx.notify();
        });
    }

    /// The option a dropdown shows: the one picked and remembered when it
    /// is the control's local option, else the one the file names.
    fn selected_of(&self, typed: Typed) -> usize {
        if self.local_pick == Some(typed.key) {
            if let Some(ix) = typed
                .options
                .iter()
                .position(|(_, v)| Some(*v) == typed.local)
            {
                return ix;
            }
        }
        self.state.as_ref().map(|s| typed.selected(s)).unwrap_or(0)
    }

    /// Picks the theme a Themes row names, writing `cssTheme`. Obsidian
    /// writes the empty string for Default rather than dropping the key.
    fn pick_theme(&mut self, name: &str, window: &Window, cx: &mut Context<Self>) {
        self.menu = None;
        let name = name.to_string();
        self.after_open_field(window, cx, move |this, cx| {
            let Some(state) = this.state.clone() else {
                return;
            };
            if state.appearance().css_theme.as_deref().unwrap_or_default() == name {
                cx.notify();
                return;
            }
            let mut next = state.appearance().clone();
            next.css_theme = Some(name);
            cx.emit(SettingsEvent::Changed(Box::new(SettingChange::Appearance(
                next,
            ))));
            cx.notify();
        });
    }

    /// "Restore default" beside the accent swatch: Obsidian writes the
    /// empty string, which is how it says "no accent set".
    fn reset_accent(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.after_open_field(window, cx, move |this, cx| {
            let Some(state) = this.state.clone() else {
                return;
            };
            if (ACCENT_COLOR.read)(&state).is_empty() {
                return;
            }
            cx.emit(SettingsEvent::Changed(Box::new((ACCENT_COLOR.write)(
                &state, "",
            ))));
            cx.notify();
        });
    }

    /// Writes the option a dropdown's row names. The value comes from the
    /// same table the control reads through, so the two cannot drift.
    fn pick(&mut self, typed: Typed, option: usize, window: &Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.after_open_field(window, cx, move |this, cx| {
            let Some(state) = this.state.clone() else {
                return;
            };
            let Some((_, value)) = typed.options.get(option) else {
                return;
            };
            // The pick the file cannot hold: remembered, not written.
            if typed.local == Some(*value) {
                this.local_pick = Some(typed.key);
                cx.notify();
                return;
            }
            this.local_pick = None;
            if (typed.read)(&state) != *value {
                cx.emit(SettingsEvent::Changed(Box::new((typed.write)(
                    &state, value,
                ))));
            }
            cx.notify();
        });
    }

    fn render_item(&self, item: &Item, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        if item.heading {
            return div()
                .pt_5()
                .pb_1()
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.tokens.text_normal)
                .child(item.name.clone())
                .into_any_element();
        }
        // A navigable row is `tappable`: the whole row opens the page, not
        // only the chevron at its end.
        let page = item.page;
        div()
            .id(SharedString::from(format!("settings-row-{}", item.name)))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_6()
            .py(px(10.))
            .border_t_1()
            .border_color(theme.tokens.background_modifier_border)
            .when_some(page, |d, page| {
                d.cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.click_dismissed_a_popover(cx) {
                            return;
                        }
                        this.open_page(page, window, cx)
                    }))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(theme.tokens.text_normal)
                                    .child(item.name.clone()),
                            )
                            .children(item.honored.tag().map(|tag| {
                                div()
                                    .flex_shrink_0()
                                    .px(px(5.))
                                    .py(px(1.))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(theme.tokens.background_modifier_border)
                                    .text_size(px(10.))
                                    .text_color(theme.tokens.text_faint)
                                    .child(tag)
                            })),
                    )
                    .when(!item.desc.is_empty(), |d| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.tokens.text_muted)
                                .child(item.desc.clone()),
                        )
                    })
                    // A chosen app icon stands in for its row's description,
                    // 64 px square (an `<img width=64 height=64>` there).
                    .when_some(
                        matches!(item.control, Control::AppIcon(_))
                            .then(|| self.app_icon.preview.clone())
                            .flatten(),
                        |d, image| d.child(gpui::img(image).size(px(64.))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .children(item.folder.map(|name| {
                        self.folder_button(
                            "settings-folder-themes",
                            name,
                            "Open themes folder",
                            theme,
                            cx,
                        )
                    }))
                    .child(self.render_control(item, theme, cx)),
            )
            .into_any_element()
    }

    /// A `lucide-folder-open` clickable icon that opens one of the
    /// `.obsidian` subfolders in the desktop's file manager.
    fn folder_button(
        &self,
        id: &'static str,
        folder: &'static str,
        label: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.))
            .rounded_sm()
            .cursor_pointer()
            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            .child(crate::assets::icon(
                "folder-open",
                16.,
                theme.tokens.icon_color,
            ))
            .tooltip(crate::canvas::menu::tooltip(label))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                if this.click_dismissed_a_popover(cx) {
                    return;
                }
                this.open_css_folder(folder, cx)
            }))
    }

    /// The CSS snippets page: the titlebar's back chevron is drawn by the
    /// modal's own header, so this is the two buttons, the search box and
    /// one toggle row per snippet. Obsidian's reload button re-reads the
    /// folder and its folder button opens it in the file manager.
    fn render_snippets_page(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let query = self.snippet_search.read(cx).text().to_string();
        let rows: Vec<Item> = match self.state.as_ref() {
            Some(state) => Self::snippet_items(state, &self.css, &query),
            None => Vec::new(),
        };
        let button =
            |id: &'static str,
             icon: &'static str,
             label: &'static str,
             theme: &Theme,
             cx: &mut Context<Self>,
             on_click: fn(&mut Self, &mut Window, &mut Context<Self>)| {
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .child(crate::assets::icon(icon, 16., theme.tokens.icon_color))
                    .tooltip(crate::canvas::menu::tooltip(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if this.click_dismissed_a_popover(cx) {
                            return;
                        }
                        on_click(this, window, cx)
                    }))
            };
        let mut list = div()
            .id("settings-snippets")
            .overflow_y_scroll()
            .size_full();
        if rows.is_empty() {
            list = list.child(
                div()
                    .py_4()
                    .text_size(px(12.))
                    .text_color(theme.tokens.text_muted)
                    .child(if query.trim().is_empty() {
                        "No snippets found."
                    } else {
                        "No snippets match."
                    }),
            );
        }
        for item in &rows {
            list = list.child(self.render_item(item, theme, cx));
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .pb_2()
                    .child(button(
                        "settings-snippets-reload",
                        "refresh-cw",
                        "Reload snippets",
                        theme,
                        cx,
                        |this, _, cx| this.reload_css_folders(cx),
                    ))
                    .child(self.folder_button(
                        "settings-snippets-folder",
                        "snippets",
                        "Open snippets folder",
                        theme,
                        cx,
                    )),
            )
            .child(div().pb_2().child(self.snippet_search.clone()))
            .child(div().flex_1().min_h_0().child(list))
    }

    /// Writes one edit of a font page's list to `appearance.json`; an edit
    /// that changes nothing writes nothing.
    fn edit_fonts(&mut self, slot: FontSlot, edit: FontEdit, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let current = state.appearance().clone();
        let Some(next) = edit.apply(&slot.read(&current)) else {
            return;
        };
        cx.emit(SettingsEvent::Changed(Box::new(SettingChange::Appearance(
            slot.write(&current, &next),
        ))));
    }

    /// A font page, as Obsidian 1.14.5 draws one (probed 2026-11-04,
    /// `font-pages.mjs`): a line saying what the list does, the families
    /// applied — each in its own face, with a Delete button and a drag
    /// handle, and a warning under one the machine lacks — then the
    /// "Enter font name..." box over the installed families, each drawn in
    /// itself. Picking one appends it to the list and takes it out of the
    /// lower card. The installed list is a `uniform_list`, so only the rows
    /// on screen load their face.
    fn render_font_page(&self, slot: FontSlot, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let applied = self
            .state
            .as_ref()
            .map(|s| slot.read(&s.appearance()))
            .unwrap_or_default();
        let query = self.font_search.read(cx).text().to_string();
        let (typed, offered) = font_choices(fonts::installed(), &applied, &query);
        let border = theme.tokens.background_modifier_border;
        let hover = theme.tokens.background_modifier_hover;
        let warning = theme.tokens.text_warning;
        let normal = theme.tokens.text_normal;
        let missing = move |family: &str| {
            (!fonts::is_installed(family)).then(|| {
                div()
                    .text_size(px(12.))
                    .text_color(warning)
                    .child("This font is not detected on your system.")
            })
        };
        let family_name = move |family: &str| {
            div()
                .text_size(px(14.))
                .text_color(normal)
                .font_family(SharedString::from(family.to_string()))
                .child(SharedString::from(family.to_string()))
        };
        let icon_button = |id: (&'static str, usize), icon: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .size(px(26.))
                .rounded_sm()
                .hover(move |s| s.bg(hover))
                .child(crate::assets::icon(icon, 16., theme.tokens.icon_color))
                .tooltip(crate::canvas::menu::tooltip(label))
        };
        let mut applied_list = div().flex().flex_col();
        for (ix, family) in applied.iter().enumerate() {
            let drag = FontMove {
                from: ix,
                family: family.clone().into(),
            };
            applied_list = applied_list.child(
                div()
                    .id(("settings-font-applied", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py(px(8.))
                    .border_t_1()
                    .border_color(border)
                    .drag_over::<FontMove>(move |s, _, _, _| s.bg(hover))
                    .on_drop(cx.listener(move |this, drag: &FontMove, _, cx| {
                        this.edit_fonts(
                            slot,
                            FontEdit::Move {
                                from: drag.from,
                                to: ix,
                            },
                            cx,
                        )
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .min_w_0()
                            .child(family_name(family))
                            .children(missing(family)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .flex_shrink_0()
                            .child(
                                icon_button(("settings-font-delete", ix), "x", "Delete")
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.edit_fonts(slot, FontEdit::Remove(ix), cx)
                                    })),
                            )
                            .child(
                                icon_button(
                                    ("settings-font-drag", ix),
                                    "menu",
                                    "Drag to rearrange",
                                )
                                .cursor_grab()
                                .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone())),
                            ),
                    ),
            );
        }
        // The typed name, offered first: a family Scoria cannot see may
        // still be one a theme or another machine has.
        let typed_row = typed.map(|name| {
            div()
                .id("settings-font-typed")
                .flex()
                .flex_col()
                .gap(px(2.))
                .py(px(8.))
                .border_t_1()
                .border_color(border)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(family_name(&name))
                .children(missing(&name))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_fonts(slot, FontEdit::Add(name.clone()), cx)
                }))
        });
        let rows = Arc::new(offered);
        let count = rows.len();
        let list = uniform_list(
            "settings-font-installed",
            count,
            cx.processor(move |_, range: Range<usize>, _, cx| {
                range
                    .filter_map(|ix| rows.get(ix).cloned())
                    .map(|family| {
                        let name: SharedString = family.clone().into();
                        div()
                            .id(name.clone())
                            .flex()
                            .items_center()
                            .w_full()
                            .h(px(36.))
                            .border_t_1()
                            .border_color(border)
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(normal)
                                    .font_family(name.clone())
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(name),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_fonts(slot, FontEdit::Add(family.clone()), cx)
                            }))
                    })
                    .collect()
            }),
        )
        .track_scroll(&self.font_scroll)
        .flex_1()
        .min_h_0();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .pb_3()
                    .text_size(px(14.))
                    .text_color(theme.tokens.text_muted)
                    .child(if applied.is_empty() {
                        "No custom font is applied right now. Add one below."
                    } else {
                        "The first font from this list that is available on your system will be applied."
                    }),
            )
            .when(!applied.is_empty(), |d| d.child(applied_list.pb_4()))
            .child(div().pb_2().child(self.font_search.clone()))
            .children(typed_row)
            .child(list)
    }

    /// Opens `themes/` or `snippets/` in the desktop's file manager,
    /// creating it first: Obsidian's two folder buttons do that, and a
    /// vault that has never had a snippet has no folder to open.
    fn open_css_folder(&mut self, name: &'static str, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let dir = state.root.join(state.config_dir()).join(name);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    std::fs::create_dir_all(&dir)
                        .map_err(|e| format!("{}: {e}", dir.display()))
                        .and_then(|()| crate::file_menu::open_folder(&dir))
                })
                .await;
            if let Err(err) = result {
                this.update(cx, |_, cx| {
                    cx.emit(SettingsEvent::Report(format!(
                        "The {name} folder could not be opened: {err}"
                    )))
                })
                .ok();
            }
        })
        .detach();
    }

    fn render_hotkey_rows(
        &mut self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Div> {
        let theme = cx.global::<Theme>().clone();
        let counts = self.chord_counts();
        let danger = theme.callout_color("danger");
        range
            .filter_map(|ix| {
                let c = self.all_commands.get(*self.hotkey_rows.get(ix)?)?;
                let capturing = self.capturing.as_deref() == Some(c.id.as_str());
                let id = c.id.clone();
                let chips: Vec<gpui::AnyElement> = c
                    .hotkeys
                    .iter()
                    .enumerate()
                    .map(|(hix, h)| {
                        let duplicate = keystroke(h)
                            .and_then(|k| counts.get(&k))
                            .is_some_and(|n| *n > 1);
                        let mut chip = hotkey_chip(hotkey_label(h), &theme)
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1();
                        if duplicate {
                            chip = chip.border_color(danger).text_color(danger);
                        } else if c.custom {
                            chip = chip
                                .border_color(theme.tokens.interactive_accent)
                                .text_color(theme.tokens.text_normal);
                        }
                        let id = id.clone();
                        let hotkey = h.clone();
                        chip.child(
                            div()
                                .id(SharedString::from(format!("hotkey-remove-{}-{hix}", c.id)))
                                .cursor_pointer()
                                .text_color(theme.tokens.text_faint)
                                .hover(|s| s.text_color(theme.tokens.text_normal))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_hotkey(&id, hotkey.clone(), cx)
                                }))
                                .child("×"),
                        )
                        .into_any_element()
                    })
                    .collect();
                let icon = |name: String, glyph: &'static str, theme: &Theme| {
                    div()
                        .id(SharedString::from(name))
                        .w(px(22.))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(theme.tokens.text_muted)
                        .hover(|s| {
                            s.bg(theme.tokens.background_modifier_hover)
                                .text_color(theme.tokens.text_normal)
                        })
                        .child(glyph)
                };
                let add = {
                    let id = c.id.clone();
                    icon(format!("hotkey-add-{}", c.id), "⊕", &theme).on_click(cx.listener(
                        move |this, _, window, cx| this.start_capture(id.clone(), window, cx),
                    ))
                };
                let restore = c.custom.then(|| {
                    let id = c.id.clone();
                    icon(format!("hotkey-restore-{}", c.id), "↺", &theme).on_click(
                        cx.listener(move |this, _, _, cx| this.restore_default_hotkeys(&id, cx)),
                    )
                });
                Some(
                    div()
                        .w_full()
                        .h(px(36.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .border_t_1()
                        .border_color(theme.tokens.background_modifier_border)
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme.tokens.text_normal)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(SharedString::from(c.name.clone())),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .flex_shrink_0()
                                .gap_1()
                                .when(chips.is_empty() && !capturing, |d| {
                                    d.child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(theme.tokens.text_faint)
                                            .child("Blank"),
                                    )
                                })
                                .children(chips)
                                .when(capturing, |d| {
                                    d.child(
                                        hotkey_chip("Press hotkey...".into(), &theme)
                                            .border_color(theme.tokens.interactive_accent)
                                            .text_color(theme.tokens.text_normal),
                                    )
                                })
                                .children(restore)
                                .child(add),
                        ),
                )
            })
            .collect()
    }

    fn render_hotkeys(&mut self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let filter = self.hotkey_filter;
        let count = self.hotkey_rows.len();
        let mut filters = div().flex().flex_row().gap_1().py_2();
        for f in HotkeyFilter::ALL {
            let f = *f;
            let active = f == filter;
            filters = filters.child(
                div()
                    .id(SharedString::from(format!("hotkey-filter-{}", f.label())))
                    .px_2()
                    .py(px(2.))
                    .rounded_sm()
                    .cursor_pointer()
                    .text_size(px(11.))
                    .text_color(if active {
                        theme.tokens.text_normal
                    } else {
                        theme.tokens.text_muted
                    })
                    .when(active, |d| d.bg(theme.tokens.background_modifier_hover))
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.hotkey_filter = f;
                        this.refresh_hotkeys(cx);
                    }))
                    .child(f.label()),
            );
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.hotkey_search.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(filters)
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.tokens.text_muted)
                            .child(SharedString::from(format!("{count} commands"))),
                    ),
            )
            .child(
                uniform_list("hotkeys", count, cx.processor(Self::render_hotkey_rows))
                    .track_scroll(&self.hotkey_scroll)
                    .flex_1(),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A committed or cancelled field stops being rendered while gpui
        // still has the focus on it, and a focus on an element no frame
        // contains dispatches nothing: every later keystroke, Escape
        // included, falls outside every key context and the window goes
        // keyboard-dead. The events that close a field arrive through a
        // subscription, which has no window to hand `focus_tab`, so the
        // frame that drops the input is where the focus goes back.
        if self.editing.is_none() && self.field.focus_handle(cx).is_focused(window) {
            self.focus_tab(window, cx);
        }
        let theme = cx.global::<Theme>().clone();
        self.corners = WindowCorners::of(window);
        // A write that landed from the hex field or "Restore default"
        // moves the open accent picker; a drag's own write does not, since
        // the picker already shows it.
        if self.menu == Some(ACCENT_COLOR.key) {
            if let (Some(picker), Some(state)) = (&mut self.picker, &self.state) {
                let stored = (ACCENT_COLOR.read)(state);
                if picker.drag.is_none() && picker.from != normalize_accent(&stored) {
                    *picker = AccentPicker::new(&stored, theme.tokens.interactive_accent);
                    // The hex field follows too, or dismissing the picker
                    // would commit the value it was showing before.
                    self.field
                        .update(cx, |input, cx| input.set_text(stored, cx));
                }
            }
        }
        let tab = self.tab;
        let page = self.page;
        let body: gpui::AnyElement = match page {
            Some(Page::CssSnippets) => self.render_snippets_page(&theme, cx).into_any_element(),
            Some(Page::Font(slot)) => self.render_font_page(slot, &theme, cx).into_any_element(),
            None if tab == Tab::Hotkeys => self.render_hotkeys(&theme, cx).into_any_element(),
            None => {
                let items = self.items(tab);
                let mut list = div()
                    .id("settings-items")
                    .overflow_y_scroll()
                    .track_scroll(&self.items_scroll)
                    .size_full();
                for item in &items {
                    list = list.child(self.render_item(item, &theme, cx));
                }
                list.into_any_element()
            }
        };
        // The title row sits over the content column alone, as the vault
        // manager's does over its splash: the nav runs the full height and
        // meets the top-left corner itself. The row is the content's own
        // ground rather than a bar, and its close goes through the panel,
        // so an open field is written first.
        let view = cx.entity().downgrade();
        let title_row = window_title_row(
            "Settings",
            theme.tokens.modal_background,
            &theme,
            window,
            move |_, cx| {
                view.update(cx, |view, cx| view.close_from_frame(cx)).ok();
            },
        );
        div()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close_action))
            .on_key_down(cx.listener(Self::key_down))
            .on_click(cx.listener(|this, _, _, cx| this.dismiss_open_controls(cx)))
            // The backdrop is outside the panel, and it closes the modal on
            // mouse up; writing on the way down is what keeps a click there
            // from losing what was typed.
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.dismiss_open_controls(cx)))
            .flex()
            .flex_row()
            // The panel fills its window, which the desktop sizes: no
            // border, rounding or shadow of its own, because the frame
            // around it is the real one. The nav is `flex_shrink_0` and the
            // content column `flex_1().min_w_0()`, so a narrow window takes
            // the difference out of the content column rather than pushing
            // it past the window's edge.
            .size_full()
            .bg(theme.tokens.modal_background)
            .map(|d| self.corners.apply(d))
            .overflow_hidden()
            .child(self.render_nav(&theme, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .map(|d| self.corners.right().apply(d))
                    .children(title_row)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .px_6()
                            .pb_4()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .pt_2()
                            .pb_2()
                            .child(
                                // `.setting-page-titlebar`: a page replaces
                                // the tab's name with a back chevron and
                                // its own.
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .when_some(page, |d, page| {
                                        d.child(
                                            div()
                                                .id("settings-page-back")
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(24.))
                                                .rounded_sm()
                                                .cursor_pointer()
                                                .hover(|s| {
                                                    s.bg(theme.tokens.background_modifier_hover)
                                                })
                                                .child(crate::assets::icon(
                                                    "chevron-left",
                                                    18.,
                                                    theme.tokens.icon_color,
                                                ))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    if this.click_dismissed_a_popover(cx) {
                                                        return;
                                                    }
                                                    let _ = page;
                                                    this.close_page(cx)
                                                })),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.tokens.text_normal)
                                            .child(match page {
                                                Some(page) => page.title(),
                                                None => tab.name(),
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .mb_2()
                            .px_3()
                            .py(px(6.))
                            .rounded_sm()
                            .bg(theme.tokens.background_modifier_hover)
                            .text_size(px(11.))
                            .text_color(theme.tokens.text_muted)
                            .child("Every control that writes does so to the vault's .obsidian folder as you use it. A tagged setting is still written for Obsidian, but Scoria does not read it yet, or reads it only when it starts."),
                    )
                    .child(div().flex_1().min_h_0().child(body)),
                    ),
            )
    }
}

/// The chord a key press names, in `hotkeys.json`'s terms: Obsidian's
/// `KeyboardEvent.key` spelling (`P`, `ArrowLeft`, `Enter`, ` ` for space)
/// and modifiers in alphabetical order. `Mod` is the platform's primary
/// modifier: on Linux and Windows Obsidian records Ctrl as `Mod` and the
/// Super key as `Meta`; on macOS it records Cmd as `Mod` and Ctrl as
/// `Ctrl`, which is what `scoria_commands::keystroke` reads back. A
/// modifier on its own is not a chord.
fn hotkey_from_keystroke(stroke: &Keystroke) -> Option<Hotkey> {
    let key = match stroke.key.as_str() {
        "control" | "ctrl" | "shift" | "alt" | "platform" | "cmd" | "super" | "function" | "fn"
        | "" => return None,
        "left" => "ArrowLeft".to_string(),
        "right" => "ArrowRight".to_string(),
        "up" => "ArrowUp".to_string(),
        "down" => "ArrowDown".to_string(),
        "enter" => "Enter".to_string(),
        "escape" => "Escape".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "insert" => "Insert".to_string(),
        "space" => " ".to_string(),
        k if k.len() > 1 && k.starts_with('f') && k[1..].chars().all(|c| c.is_ascii_digit()) => {
            k.to_uppercase()
        }
        k if k.chars().count() == 1 => k.to_uppercase(),
        _ => return None,
    };
    let m = stroke.modifiers;
    let mut modifiers: Vec<String> = Vec::new();
    let macos = cfg!(target_os = "macos");
    if m.alt {
        modifiers.push("Alt".into());
    }
    if m.control && macos {
        modifiers.push("Ctrl".into());
    }
    if m.platform && !macos {
        modifiers.push("Meta".into());
    }
    if (m.control && !macos) || (m.platform && macos) {
        modifiers.push("Mod".into());
    }
    if m.shift {
        modifiers.push("Shift".into());
    }
    Some(Hotkey { modifiers, key })
}

/// The Settings window's size. Obsidian's modal is `min(90vw, 1100px)`
/// wide and `80vh` tall; a window of its own opens at that width and a
/// height to match, centered on the screen, and the desktop takes it from
/// there.
pub const WINDOW_SIZE: (f32, f32) = (1100., 720.);

/// The Settings window's root view: the frame around [`SettingsView`].
///
/// The panel itself belongs to the workspace, which keeps it between
/// openings and listens to it for the settings it writes, so this holds
/// nothing but the handle to it.
pub struct SettingsWindow {
    view: Entity<SettingsView>,
    focus_handle: FocusHandle,
    /// Takes this window down with the workspace that opened it. The panel
    /// writes through the workspace, so one left behind by a closed
    /// workspace would show a vault that is gone and write nothing.
    _workspace_gone: Option<Subscription>,
}

impl SettingsWindow {
    /// Opens the window on `tab`, with the workspace's own panel inside it.
    pub fn open(
        view: Entity<SettingsView>,
        tab: Option<Tab>,
        workspace: WeakEntity<Workspace>,
        cx: &mut App,
    ) -> anyhow::Result<WindowHandle<SettingsWindow>> {
        let (w, h) = WINDOW_SIZE;
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        let handle = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(640.), px(480.))),
                // The same frame the main window has, with a title row of
                // its own for the window buttons; its close goes through
                // the panel, so an open field is written first.
                ..frame_window_options("Settings")
            },
            move |window, cx| {
                view.update(cx, |view, cx| view.open(tab, window, cx));
                // A text field is written on the way out of it, and the
                // title bar's close is a way out the panel never sees.
                let open_field = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    open_field
                        .update(cx, |view, cx| view.dismiss_open_controls(cx))
                        .ok();
                    true
                });
                cx.new(|cx| SettingsWindow {
                    view,
                    focus_handle: cx.focus_handle(),
                    _workspace_gone: None,
                })
            },
        )?;
        if let Some(workspace) = workspace.upgrade() {
            let window = handle;
            let gone = cx.observe_release(&workspace, move |_, cx| {
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .ok();
            });
            handle
                .update(cx, |root, _, _| root._workspace_gone = Some(gone))
                .ok();
        }
        Ok(handle)
    }

    /// Escape with nothing inside the panel focused still closes the
    /// window, through the panel so an open field is written first.
    fn close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        self.view.update(cx, |view, cx| view.close_from_frame(cx));
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let corners = WindowCorners::of(window);
        // The title row is the panel's, drawn over its content column.
        let root = div()
            .id("settings-window")
            .key_context("Settings")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.tokens.modal_background)
            .map(|d| corners.apply(d))
            .font_family(theme.ui_font.clone())
            .text_size(px(crate::theme::FONT_UI_MEDIUM))
            .text_color(theme.tokens.text_normal)
            .child(div().flex_1().min_h_0().w_full().child(self.view.clone()));
        client_frame(root.into_any_element(), &theme, window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(s: &str) -> Keystroke {
        Keystroke::parse(s).unwrap()
    }

    /// The primary modifier is `Mod` on every platform: Ctrl on Linux and
    /// Windows, Cmd on macOS, where Ctrl stays `Ctrl`.
    #[test]
    fn a_chord_is_written_the_way_obsidian_spells_it() {
        let (primary, other, other_name) = if cfg!(target_os = "macos") {
            ("cmd", "ctrl", "Ctrl")
        } else {
            ("ctrl", "cmd", "Meta")
        };
        let hk = hotkey_from_keystroke(&stroke(&format!("{primary}-shift-f"))).unwrap();
        assert_eq!(hk.modifiers, ["Mod", "Shift"]);
        assert_eq!(hk.key, "F");
        let hk = hotkey_from_keystroke(&stroke(&format!("alt-{primary}-left"))).unwrap();
        assert_eq!(hk.modifiers, ["Alt", "Mod"]);
        assert_eq!(hk.key, "ArrowLeft");
        assert_eq!(hotkey_from_keystroke(&stroke("f5")).unwrap().key, "F5");
        assert_eq!(hotkey_from_keystroke(&stroke("space")).unwrap().key, " ");
        assert_eq!(
            hotkey_from_keystroke(&stroke(&format!("{other}-k")))
                .unwrap()
                .modifiers,
            [other_name]
        );
        assert!(hotkey_from_keystroke(&stroke("shift")).is_none());
    }

    #[test]
    fn a_captured_chord_round_trips_through_the_registry_keystroke() {
        for name in [
            "ctrl-p",
            "ctrl-shift-f",
            "alt-enter",
            "f2",
            "ctrl-alt-right",
            "ctrl-,",
            "cmd-p",
            "cmd-shift-f",
        ] {
            let hk = hotkey_from_keystroke(&stroke(name)).unwrap();
            let back = keystroke(&hk).unwrap();
            assert_eq!(stroke(&back), stroke(name), "{name} came back as {back}");
        }
    }

    /// Every toggle of every file writes the key Obsidian writes, flips
    /// only that key, and then reads back the other way round.
    #[test]
    fn every_toggle_writes_its_own_key_and_nothing_else() {
        macro_rules! check {
            ($ty:ident, $config:ty) => {
                for flag in $ty::ALL {
                    let before = <$config>::default();
                    let after = flag.toggled(&before);
                    let json = serde_json::to_value(&after).unwrap();
                    let object = json.as_object().expect("an object");
                    assert_eq!(
                        object.len(),
                        1,
                        "{} changed more than its own key: {json}",
                        flag.key
                    );
                    assert_eq!(
                        object.get(flag.key),
                        Some(&serde_json::Value::Bool(!flag.default)),
                        "{} wrote the wrong key or value: {json}",
                        flag.key
                    );
                    assert_eq!(
                        flag.is_on(&after),
                        !flag.is_on(&before),
                        "{} did not flip",
                        flag.key
                    );
                }
            };
        }
        check!(AppFlag, AppConfig);
        check!(AppearanceFlag, AppearanceConfig);
        check!(SwitcherFlag, SwitcherConfig);
    }

    #[test]
    fn every_dropdown_writes_its_own_key_and_nothing_else() {
        let (_dir, state) = vault();
        for typed in TYPED_ALL {
            for (label, value) in typed.options {
                if typed.local == Some(*value) {
                    continue;
                }
                let change = (typed.write)(&state, value);
                let json = match &change {
                    SettingChange::App(c) => serde_json::to_value(c).unwrap(),
                    SettingChange::Appearance(c) => serde_json::to_value(c).unwrap(),
                    SettingChange::DailyNotes(c) => serde_json::to_value(c).unwrap(),
                    SettingChange::Templates(c) => serde_json::to_value(c).unwrap(),
                    SettingChange::Canvas(c) => serde_json::to_value(c).unwrap(),
                    SettingChange::NoteComposer(c) => serde_json::to_value(c).unwrap(),
                    SettingChange::Webviewer(c) => serde_json::to_value(c).unwrap(),
                    other => panic!("{} wrote {other:?}", typed.key),
                };
                let mut object = json.as_object().expect("an object").clone();
                // The Note composer saves its whole options object, so its
                // first write carries `askBeforeMerging: true` too
                // (2026-11-09) -- observed, not a stray key.
                if typed.key == "replacementText" {
                    assert_eq!(object.remove("askBeforeMerging"), Some(json!(true)));
                }
                assert_eq!(
                    object.len(),
                    1,
                    "{} changed more than its own key for {label}: {json}",
                    typed.key
                );
                let written = object.get(typed.key).expect("its own key");
                // `livePreview` is a boolean behind a two-option dropdown;
                // every other one writes the string its option names.
                let want = match *value {
                    "true" => serde_json::Value::Bool(true),
                    "false" => serde_json::Value::Bool(false),
                    v => serde_json::Value::String(v.to_string()),
                };
                assert_eq!(
                    written, &want,
                    "{} wrote the wrong value for {label}: {json}",
                    typed.key
                );
            }
        }
    }

    #[test]
    fn a_dropdown_reads_back_what_it_wrote() {
        // The control's label and its write go through the same table, so
        // picking an option must leave that option showing.
        let (dir, mut state) = vault();
        for typed in TYPED_ALL {
            for (ix, (label, value)) in typed.options.iter().enumerate() {
                if typed.local == Some(*value) {
                    continue;
                }
                state.apply_setting(&(typed.write)(&state, value)).unwrap();
                state = VaultState::load(&dir.path().to_path_buf()).unwrap();
                assert_eq!(
                    typed.selected(&state),
                    ix,
                    "{} did not read back {label}",
                    typed.key
                );
                assert_eq!(typed.label(&state), *label);
            }
        }
    }

    #[test]
    fn every_slider_and_field_writes_its_own_key_and_nothing_else() {
        let (_dir, state) = vault();
        let one_key = |change: &SettingChange, key: &str, what: &str| {
            let json = match change {
                SettingChange::App(c) => serde_json::to_value(c).unwrap(),
                SettingChange::Appearance(c) => serde_json::to_value(c).unwrap(),
                SettingChange::DailyNotes(c) => serde_json::to_value(c).unwrap(),
                SettingChange::Templates(c) => serde_json::to_value(c).unwrap(),
                SettingChange::Canvas(c) => serde_json::to_value(c).unwrap(),
                SettingChange::NoteComposer(c) => serde_json::to_value(c).unwrap(),
                SettingChange::ZkPrefixer(c) => serde_json::to_value(c).unwrap(),
                SettingChange::Webviewer(c) => serde_json::to_value(c).unwrap(),
                other => panic!("{what} wrote {other:?}"),
            };
            let object = json.as_object().expect("an object");
            assert_eq!(object.len(), 1, "{what} changed more than its key: {json}");
            assert!(object.contains_key(key), "{what} wrote {json}");
            object.get(key).cloned().expect("its own key")
        };
        for number in NUMBERS {
            for value in [number.min, number.max] {
                let written = one_key(&(number.write)(&state, value), number.key, number.key);
                assert_eq!(written, serde_json::json!(value));
            }
        }
        for field in TEXTS {
            let written = one_key(&(field.write)(&state, "Journal"), field.key, field.key);
            // Every field but the Subfolder name row is the whole value;
            // that one is the half after the `./` it writes back.
            assert_eq!(
                written,
                serde_json::json!(format!("{}Journal", field.prefix)),
                "{}",
                field.key
            );
        }
    }

    #[test]
    fn a_slider_and_a_field_read_back_what_they_wrote() {
        let (dir, mut state) = vault();
        for number in NUMBERS {
            for value in [number.min, number.max, number.min + 1] {
                state.apply_setting(&(number.write)(&state, value)).unwrap();
                state = VaultState::load(&dir.path().to_path_buf()).unwrap();
                assert_eq!((number.read)(&state), value, "{}", number.key);
            }
        }
        for field in TEXTS {
            state
                .apply_setting(&(field.write)(&state, "Folder With Space"))
                .unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!((field.read)(&state), "Folder With Space", "{}", field.key);
        }
    }

    /// The Daily notes and Templates tabs, as 1.14.5 shows them
    /// (`tools/obsidian-probe/results/2026-10-30-plugin-options.json`): the
    /// row names in order, the Custom format row hidden behind a preset and
    /// shown for a stored format that is none of them, the placeholders,
    /// and a preset pick writing `format` alone.
    #[test]
    fn the_daily_notes_and_templates_tabs_show_obsidians_rows() {
        let (dir, mut state) = vault();
        let names = |items: &[Item]| items.iter().map(|i| i.name.to_string()).collect::<Vec<_>>();

        let daily = SettingsView::plugin_items(&state, &[], "daily-notes", None);
        assert_eq!(
            names(&daily),
            ["Date format", "New file location", "Template file location"]
        );
        assert_eq!(
            daily[0].control,
            Control::Dropdown(templates::format("YYYY-MM-DD", &crate::workspace::now()).into())
        );
        assert_eq!(
            daily[1].field.map(|f| f.placeholder),
            Some("Example: folder 1/folder 2")
        );
        assert!(daily.iter().all(|i| i.honored == Honored::Live));

        // "Custom" picked: the row appears, nothing is written.
        let picked = SettingsView::plugin_items(&state, &[], "daily-notes", Some(DAILY_FORMAT.key));
        assert_eq!(
            names(&picked),
            [
                "Date format",
                "Custom format",
                "New file location",
                "Template file location"
            ]
        );
        assert_eq!(picked[0].control, Control::Dropdown("Custom".into()));
        assert_eq!(picked[1].field.map(|f| f.placeholder), Some("YYYY-MM-DD"));
        assert!(picked[1]
            .desc
            .ends_with(&templates::format("YYYY-MM-DD", &crate::workspace::now())));
        assert!(!dir.path().join(".obsidian/daily-notes.json").exists());

        // A preset written: `format` alone, and the row hides again.
        state
            .apply_setting(&(DAILY_FORMAT.write)(&state, "YYYY.MM.DD"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".obsidian/daily-notes.json")).unwrap(),
            "{\n  \"format\": \"YYYY.MM.DD\"\n}"
        );
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!(DAILY_FORMAT.selected(&state), 1);
        assert_eq!(
            names(&SettingsView::plugin_items(
                &state,
                &[],
                "daily-notes",
                None
            ))
            .len(),
            3
        );

        // A format typed that is none of the presets selects Custom on its own.
        state
            .apply_setting(&(DAILY_CUSTOM_FORMAT.write)(&state, "YYYY-MM-DD dddd"))
            .unwrap();
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!(DAILY_FORMAT.selected(&state), 4);
        let shown = SettingsView::plugin_items(&state, &[], "daily-notes", None);
        assert_eq!(shown[1].name.as_ref(), "Custom format");
        assert_eq!(shown[1].control, Control::Text("YYYY-MM-DD dddd".into()));

        let templates = SettingsView::plugin_items(&state, &[], "templates", None);
        assert_eq!(
            names(&templates),
            ["Template folder location", "Date format", "Time format"]
        );
        assert_eq!(
            templates[1].field.map(|f| f.placeholder),
            Some("YYYY-MM-DD")
        );
        assert_eq!(templates[2].field.map(|f| f.placeholder), Some("HH:mm"));
        state
            .apply_setting(&(TEMPLATES_TIME.write)(&state, "HH:mm:ss"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".obsidian/templates.json")).unwrap(),
            "{\n  \"timeFormat\": \"HH:mm:ss\"\n}"
        );
    }

    /// The row names of a plugin tab, in order.
    fn plugin_rows(state: &VaultState, id: &str) -> Vec<String> {
        SettingsView::plugin_items(state, &[], id, None)
            .into_iter()
            .map(|i| i.name.to_string())
            .collect()
    }

    /// Every toggle of a tab written through [`VaultState::apply_setting`],
    /// one at a time, each from a freshly loaded state so the row's own read
    /// decides what the click writes. Returns the file as it ended up.
    fn flip_every_toggle(dir: &tempfile::TempDir, id: &str, file: &str) -> String {
        let mut state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let count = SettingsView::plugin_items(&state, &[], id, None).len();
        for ix in 0..count {
            let items = SettingsView::plugin_items(&state, &[], id, None);
            let Some(flag) = items[ix].flag.clone() else {
                continue;
            };
            let was = flag.is_on(&state);
            state.apply_setting(&flag.toggled(&state)).unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            let items = SettingsView::plugin_items(&state, &[], id, None);
            assert_eq!(
                items[ix].control,
                Control::Toggle(!was),
                "{} did not read back its own click",
                items[ix].name
            );
        }
        std::fs::read_to_string(dir.path().join(".obsidian").join(file)).unwrap()
    }

    /// The Backlinks tab: one toggle, off by default, writing
    /// `backlinkInDocument` into `backlink.json`, which the backlinks
    /// block at the foot of a note follows.
    #[test]
    fn the_backlinks_tab_shows_one_toggle_that_writes_backlink_in_document() {
        let (dir, state) = vault();
        let items = SettingsView::plugin_items(&state, &[], "backlink", None);
        assert_eq!(
            plugin_rows(&state, "backlink"),
            ["Show backlinks at the bottom of notes"]
        );
        assert_eq!(
            items[0].desc.as_ref(),
            "Make backlinks visible in new tabs by default."
        );
        assert_eq!(items[0].control, Control::Toggle(false), "off by default");
        assert_eq!(items[0].honored, Honored::Live);
        assert!(!dir.path().join(".obsidian/backlink.json").exists());
        assert_eq!(
            flip_every_toggle(&dir, "backlink", "backlink.json"),
            "{\n  \"backlinkInDocument\": true\n}"
        );
    }

    /// The Canvas tab: Obsidian's eight rows in order, with the folder row
    /// hidden until `folder` is picked, each dropdown writing the string
    /// Obsidian writes, and both snap toggles writing their own key.
    #[test]
    fn the_canvas_tab_shows_obsidians_rows_and_writes_its_observed_keys() {
        let (dir, mut state) = vault();
        assert_eq!(
            plugin_rows(&state, "canvas"),
            [
                "Default location for new canvas files",
                "Default mouse wheel behavior",
                "Default Ctrl + Drag behavior",
                "Show card names",
                "Snap to grid",
                "Snap to objects",
                "Zoom threshold for hiding card content",
            ]
        );
        let items = SettingsView::plugin_items(&state, &[], "canvas", None);
        assert_eq!(items[0].control, Control::Dropdown("Vault folder".into()));
        assert_eq!(items[1].control, Control::Dropdown("Pan".into()));
        assert_eq!(items[2].control, Control::Dropdown("Show menu".into()));
        assert_eq!(items[3].control, Control::Dropdown("Always".into()));
        assert_eq!(
            items[4].control,
            Control::Toggle(true),
            "snap to grid is on"
        );
        assert_eq!(items[5].control, Control::Toggle(true));
        // The slider has no key, so it has no `Number` and writes nothing.
        assert_eq!(items[6].control, Control::Slider("0".into()));
        assert!(items[6].number.is_none());
        assert!(items.iter().all(|i| i.honored == Honored::No));

        // Every option of every dropdown: written, reloaded, read back, and
        // the one key it wrote is the one Obsidian wrote.
        for typed in [
            CANVAS_NEW_FILE_LOCATION,
            CANVAS_WHEEL_BEHAVIOR,
            CANVAS_MOD_DRAG_BEHAVIOR,
            CANVAS_CARD_LABELS,
        ] {
            for (label, value) in typed.options {
                state.apply_setting(&(typed.write)(&state, value)).unwrap();
                state = VaultState::load(&dir.path().to_path_buf()).unwrap();
                assert_eq!(typed.label(&state), *label, "{}", typed.key);
                assert_eq!((typed.read)(&state), *value, "{}", typed.key);
            }
        }
        // `folder` is the option that reveals the folder row, as its twin on
        // the Files and links tab does.
        state
            .apply_setting(&(CANVAS_NEW_FILE_LOCATION.write)(&state, "folder"))
            .unwrap();
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let rows = plugin_rows(&state, "canvas");
        assert_eq!(rows[1], "Folder to create new canvas files in");
        assert_eq!(rows.len(), 8);
        let revealed = SettingsView::plugin_items(&state, &[], "canvas", None);
        assert!(revealed[1].field.is_none(), "no key was observed for it");
        assert_eq!(
            revealed[1].desc.as_ref(),
            "Newly created canvas files will appear under this folder."
        );

        let file = flip_every_toggle(&dir, "canvas", "canvas.json");
        let json: Value = serde_json::from_str(&file).unwrap();
        assert_eq!(json["snapToGrid"], json!(false));
        assert_eq!(json["snapToObjects"], json!(false));
        assert_eq!(json["newFileLocation"], json!("folder"));
        assert_eq!(json["defaultWheelBehavior"], json!("zoom"));
        assert_eq!(json["defaultModDragBehavior"], json!("group"));
        assert_eq!(json["cardLabelVisibility"], json!("never"));
        assert_eq!(
            json.as_object().map(|o| o.len()),
            Some(6),
            "a row with no observed key wrote one anyway: {file}"
        );
    }

    /// The Page preview tab: a heading with no control and ten toggles, each
    /// writing the key named after its surface rather than after its row.
    #[test]
    fn the_page_preview_tab_writes_one_key_per_surface() {
        let (dir, state) = vault();
        let items = SettingsView::plugin_items(&state, &[], "page-preview", None);
        assert_eq!(
            plugin_rows(&state, "page-preview"),
            [
                "Require Ctrl to trigger page preview on hover",
                "Search, Backlinks, and Outgoing links",
                "Reading view",
                "Editing view",
                "Tab header",
                "Files",
                "Properties view",
                "Bookmarks",
                "Outline",
                "Bases",
                "Graph view",
            ]
        );
        assert!(items[0].heading, "the first row is the heading");
        assert_eq!(items[0].control, Control::None);
        // Reading view is the one surface that is off when its key is absent.
        assert_eq!(items[2].control, Control::Toggle(false));
        for row in &items[1..] {
            assert_eq!(row.honored, Honored::No, "{}", row.name);
            assert!(row.desc.is_empty(), "{} has a description", row.name);
        }

        let file = flip_every_toggle(&dir, "page-preview", "page-preview.json");
        let json: Value = serde_json::from_str(&file).unwrap();
        for (key, want) in [
            ("search", false),
            ("preview", true),
            ("editor", false),
            ("tab-header", false),
            ("file-explorer", false),
            ("properties", false),
            ("bookmarks", false),
            ("outline", false),
            ("bases", false),
            ("graph", false),
        ] {
            assert_eq!(json[key], json!(want), "{key} in {file}");
        }
        assert_eq!(json.as_object().map(|o| o.len()), Some(10));
    }

    /// The Note composer tab: the dropdown writes `replacementText`, the
    /// field `template` and the toggle `askBeforeMerging`, and the first
    /// write on a vault with no file carries `askBeforeMerging: true`
    /// before its own key, as Obsidian's did (2026-11-09).
    #[test]
    fn the_note_composer_tab_writes_its_three_keys_the_way_obsidian_does() {
        let (dir, state) = vault();
        let file = dir.path().join(".obsidian/note-composer.json");
        state
            .apply_setting(&(COMPOSER_REPLACEMENT_TEXT.write)(&state, "embed"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "{\n  \"askBeforeMerging\": true,\n  \"replacementText\": \"embed\"\n}"
        );
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        state
            .apply_setting(&(COMPOSER_TEMPLATE.write)(&state, "Templates/Composer.md"))
            .unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!((COMPOSER_TEMPLATE.read)(&state), "Templates/Composer.md");
        state
            .apply_setting(&(COMPOSER_TEMPLATE.write)(&state, ""))
            .unwrap();
        let json: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(json["template"], json!(""), "cleared, not removed");
        let items = SettingsView::plugin_items(&state, &[], "note-composer", None);
        assert!(items.iter().all(|i| i.honored == Honored::Live));
        assert!(items[1].field.is_some());
    }

    #[test]
    fn the_note_composer_tabs_dropdown_and_toggle_round_trip() {
        let (dir, mut state) = vault();
        assert_eq!(
            plugin_rows(&state, "note-composer"),
            [
                "Text after extraction",
                "Template file location",
                "Confirm file merge",
            ]
        );
        let items = SettingsView::plugin_items(&state, &[], "note-composer", None);
        assert_eq!(
            items[0].control,
            Control::Dropdown("Link to new file".into())
        );
        assert_eq!(
            items[1].desc.as_ref(),
            "Template file to use when merging or extracting. Available variables: {{content}}, {{fromTitle}}, {{newTitle}}, {{date:FORMAT}}, e.g. {{date:YYYY-MM-DD}}."
        );
        assert_eq!(items[2].control, Control::Toggle(true), "on by default");

        for (label, value) in COMPOSER_REPLACEMENT_TEXT.options {
            state
                .apply_setting(&(COMPOSER_REPLACEMENT_TEXT.write)(&state, value))
                .unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!(COMPOSER_REPLACEMENT_TEXT.label(&state), *label);
        }
        let file = flip_every_toggle(&dir, "note-composer", "note-composer.json");
        let json: Value = serde_json::from_str(&file).unwrap();
        assert_eq!(json["askBeforeMerging"], json!(false));
        assert_eq!(json["replacementText"], json!("none"));
        assert_eq!(json.as_object().map(|o| o.len()), Some(2));
    }

    /// The Unique note creator tab as 1.15.0 showed it, and the probe's own
    /// sequence of edits -- folder, template, template cleared, format,
    /// format cleared, folder cleared -- leaving `zk-prefixer.json` byte for
    /// byte as Obsidian left it after each (`settings:s01-tab-zk`).
    #[test]
    fn the_unique_note_creator_tab_writes_zk_prefixer_json_as_obsidian_did() {
        let (dir, mut state) = vault();
        assert_eq!(
            plugin_rows(&state, "zk-prefixer"),
            [
                "New file location",
                "Template file location",
                "Unique prefix format",
            ]
        );
        let sample = |pattern: &str| {
            format!(
                "For more syntax, refer to format reference\nYour current syntax looks like this: {}",
                templates::format(pattern, &crate::workspace::now())
            )
        };
        let zk = SettingsView::plugin_items(&state, &[], "zk-prefixer", None);
        assert_eq!(zk[2].desc.as_ref(), sample("YYYYMMDDHHmm"));
        let fields: Vec<TextField> = zk.iter().map(|row| row.field.expect("a field")).collect();
        assert_eq!(
            fields.iter().map(|f| f.placeholder).collect::<Vec<_>>(),
            [
                "Example: folder 1/folder 2",
                "Example: folder1/note",
                "YYYYMMDDHHmm"
            ]
        );
        assert!(!dir.path().join(".obsidian/zk-prefixer.json").exists());
        let steps: [(TextField, &str, &str); 6] = [
            (ZK_FOLDER, "Zettel", "{\n  \"folder\": \"Zettel\"\n}"),
            (
                ZK_TEMPLATE,
                "Templates/Unique.md",
                "{\n  \"folder\": \"Zettel\",\n  \"template\": \"Templates/Unique.md\"\n}",
            ),
            (
                ZK_TEMPLATE,
                "",
                "{\n  \"folder\": \"Zettel\",\n  \"template\": \"\"\n}",
            ),
            (
                ZK_FORMAT,
                "YYYY-MM-DD-HHmmss",
                "{\n  \"folder\": \"Zettel\",\n  \"template\": \"\",\n  \"format\": \"YYYY-MM-DD-HHmmss\"\n}",
            ),
            (
                ZK_FORMAT,
                "",
                "{\n  \"folder\": \"Zettel\",\n  \"template\": \"\",\n  \"format\": \"\"\n}",
            ),
            (
                ZK_FOLDER,
                "",
                "{\n  \"folder\": \"\",\n  \"template\": \"\",\n  \"format\": \"\"\n}",
            ),
        ];
        for (field, value, bytes) in steps {
            state.apply_setting(&(field.write)(&state, value)).unwrap();
            let file =
                std::fs::read_to_string(dir.path().join(".obsidian/zk-prefixer.json")).unwrap();
            assert_eq!(file, bytes, "after {} = {value:?}", field.key);
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!((field.read)(&state), value);
            if field.same(&ZK_FORMAT) {
                // The sample follows the field, and goes back to the
                // default's when it is cleared.
                let zk = SettingsView::plugin_items(&state, &[], "zk-prefixer", None);
                let pattern = if value.is_empty() {
                    "YYYYMMDDHHmm"
                } else {
                    value
                };
                assert_eq!(zk[2].desc.as_ref(), sample(pattern));
            }
        }
    }

    /// The File recovery tab: the two number fields write
    /// `file-recovery.json` byte for byte as Obsidian 1.15.0 did while the
    /// probe typed into them (`2026-11-09-file-recovery.json`,
    /// `s05-settings-typed`), refused values writing nothing; the two
    /// buttons stay inert, Scoria keeping no snapshots. Every row is tagged.
    #[test]
    fn the_file_recovery_tab_writes_file_recovery_json_as_obsidian_did() {
        let (dir, mut state) = vault();
        assert_eq!(
            plugin_rows(&state, "file-recovery"),
            [
                "Snapshot interval",
                "History length",
                "Snapshots",
                "Clear history",
            ]
        );
        let recovery = SettingsView::plugin_items(&state, &[], "file-recovery", None);
        assert_eq!(recovery[0].control, Control::Text("5".into()));
        assert_eq!(recovery[1].control, Control::Text("7".into()));
        assert_eq!(recovery[2].control, Control::Button("View".into()));
        assert_eq!(recovery[3].control, Control::Button("Clear".into()));
        for row in recovery.iter() {
            assert_eq!(row.honored, Honored::No, "{}", row.name);
        }
        assert_eq!(recovery[0].field.map(|f| f.key), Some("intervalMinutes"));
        assert_eq!(recovery[1].field.map(|f| f.key), Some("keepDays"));
        assert!(recovery[2].field.is_none() && recovery[3].field.is_none());
        let path = dir.path().join(".obsidian/file-recovery.json");
        assert!(!path.exists());
        let both =
            |i: &str, k: &str| format!("{{\n  \"intervalMinutes\": {i},\n  \"keepDays\": {k}\n}}");
        // The probe's sequence, less the two emptied fields (see the tab's
        // note): each value typed and the file after it, `None` where
        // Obsidian wrote nothing.
        let steps: [(TextField, &str, Option<String>); 9] = [
            (RECOVERY_INTERVAL, "10", Some(both("10", "7"))),
            (RECOVERY_KEEP_DAYS, "30", Some(both("10", "30"))),
            (RECOVERY_INTERVAL, "0", Some(both("0", "30"))),
            (RECOVERY_INTERVAL, "-1", None),
            (RECOVERY_INTERVAL, "2.5", Some(both("2.5", "30"))),
            (RECOVERY_KEEP_DAYS, "0", None),
            (RECOVERY_KEEP_DAYS, "0.5", None),
            (RECOVERY_INTERVAL, "5", Some(both("5", "30"))),
            (RECOVERY_KEEP_DAYS, "7", Some(both("5", "7"))),
        ];
        let mut expected = None;
        for (field, value, bytes) in steps {
            state.apply_setting(&(field.write)(&state, value)).unwrap();
            if bytes.is_some() {
                expected = bytes;
            }
            let file = std::fs::read_to_string(&path).ok();
            assert_eq!(file, expected, "after {} = {value:?}", field.key);
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        }
        assert_eq!((RECOVERY_INTERVAL.read)(&state), "5");
        let recovery = SettingsView::plugin_items(&state, &[], "file-recovery", None);
        assert_eq!(recovery[1].control, Control::Text("7".into()));
        // A refused value before anything was written creates no file.
        let (dir, state) = vault();
        state
            .apply_setting(&(RECOVERY_INTERVAL.write)(&state, "abc"))
            .unwrap();
        assert!(!dir.path().join(".obsidian/file-recovery.json").exists());
    }

    /// The Web viewer tab: Obsidian's nine rows, the four keys the probe saw
    /// written, and the five rows it did not see a key for drawn and inert.
    #[test]
    fn the_web_viewer_tab_writes_the_four_keys_the_probe_saw() {
        let (dir, mut state) = vault();
        assert_eq!(
            plugin_rows(&state, "webviewer"),
            [
                "Open external links",
                "Homepage",
                "Saved page folder",
                "Search engine",
                "Custom search engine",
                "Enable ad blocker",
                "Ad blocking rules",
                "Ad block update frequency",
                "Web viewer data",
            ]
        );
        let items = SettingsView::plugin_items(&state, &[], "webviewer", None);
        assert_eq!(items[0].control, Control::Toggle(true));
        assert_eq!(items[3].control, Control::Dropdown("DuckDuckGo".into()));
        // An untouched field reads as the URL Obsidian shows in it.
        assert_eq!(
            items[4].control,
            Control::Text(WEBVIEWER_DEFAULT_SEARCH_URL.into())
        );
        assert_eq!(items[5].control, Control::Toggle(true));
        assert_eq!(
            items[6].control,
            Control::Text(
                "https://easylist.to/easylist/easylist.txt\nhttps://easylist.to/easylist/easyprivacy.txt".into()
            )
        );
        assert_eq!(items[7].control, Control::Slider("4".into()));
        assert_eq!(
            items[8].control,
            Control::Button("Clear web viewer data".into())
        );
        for ix in [1, 2, 6, 7, 8] {
            assert!(
                items[ix].field.is_none() && items[ix].number.is_none(),
                "{} writes a key nobody observed",
                items[ix].name
            );
        }
        assert!(items.iter().all(|i| i.honored == Honored::No));

        for (label, value) in WEB_SEARCH_ENGINE.options {
            state
                .apply_setting(&(WEB_SEARCH_ENGINE.write)(&state, value))
                .unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!(WEB_SEARCH_ENGINE.label(&state), *label);
        }
        state
            .apply_setting(&(WEB_CUSTOM_SEARCH_ENGINE.write)(
                &state,
                "https://example.com/?q=%s",
            ))
            .unwrap();
        let file = flip_every_toggle(&dir, "webviewer", "webviewer.json");
        let json: Value = serde_json::from_str(&file).unwrap();
        assert_eq!(json["openExternalURLs"], json!(false));
        assert_eq!(json["enableAdblocking"], json!(false));
        assert_eq!(json["searchEngine"], json!("custom"));
        assert_eq!(
            json["customSearchEngine"],
            json!("https://example.com/?q=%s")
        );
        assert_eq!(json.as_object().map(|o| o.len()), Some(4), "{file}");
    }

    /// Sync's tab had no rows at all in Obsidian, because the probe's vault
    /// was not signed in. The placeholder arm would claim Scoria does not
    /// read a settings file this plugin has not got, so the tab says what is
    /// true instead — and is tagged, like every row that changes nothing.
    #[test]
    fn the_sync_tab_says_why_it_has_no_options() {
        let (_dir, state) = vault();
        let items = SettingsView::plugin_items(&state, &[], "sync", None);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name.as_ref(), "Sync");
        assert_eq!(items[0].control, Control::None);
        assert_eq!(items[0].honored, Honored::No);
        // Every tab Obsidian lists now builds rows of its own; the
        // placeholder is left for a plugin that grows an options tab later.
        for (id, _) in PLUGIN_TABS {
            let rows = plugin_rows(&state, id);
            assert!(
                rows != ["Options not shown yet"],
                "{id} still shows the placeholder"
            );
        }
        assert_eq!(
            plugin_rows(&state, "not-a-plugin"),
            ["Options not shown yet"]
        );
    }

    /// Obsidian writes the placeholder rather than an empty string when the
    /// attachment folder is cleared, which the phase 3 probe caught.
    #[test]
    fn clearing_the_attachment_folder_writes_its_placeholder() {
        let (_dir, state) = vault();
        let change = (ATTACHMENT_FOLDER.write)(&state, "");
        let SettingChange::App(app) = &change else {
            panic!("wrote the wrong file");
        };
        assert_eq!(app.attachment_folder_path.as_deref(), Some("Attachments"));
        // The Subfolder name row is the same key behind a `./`, so an empty
        // one leaves the value the subfolder option itself writes. Obsidian
        // clearing this row is unprobed; the sibling's rule is the guess.
        let change = (SUBFOLDER_NAME.write)(&state, "");
        let SettingChange::App(app) = &change else {
            panic!("wrote the wrong file");
        };
        assert_eq!(app.attachment_folder_path.as_deref(), Some("./Attachments"));
        // The other field keeps an empty string, as Obsidian does for it.
        let change = (NEW_FILE_FOLDER.write)(&state, "");
        let SettingChange::App(app) = &change else {
            panic!("wrote the wrong file");
        };
        assert_eq!(app.new_file_folder_path.as_deref(), Some(""));
    }

    /// Every row the Files and links tab builds, as a name and what its
    /// control shows, for the tests that check which dependent row an
    /// option reveals.
    fn files_rows(state: &VaultState) -> Vec<(String, String)> {
        SettingsView::files_items(state)
            .into_iter()
            .map(|item| {
                let value = match &item.control {
                    Control::Dropdown(v)
                    | Control::Text(v)
                    | Control::Slider(v)
                    | Control::Button(v) => v.to_string(),
                    Control::Color(v) | Control::Navigable(v) | Control::AppIcon(v) => {
                        v.to_string()
                    }
                    Control::Toggle(on) => on.to_string(),
                    Control::None => String::new(),
                };
                (item.name.to_string(), value)
            })
            .collect()
    }

    fn row(state: &VaultState, name: &str) -> Option<String> {
        files_rows(state)
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    /// The spec's option table, row by row: the label each of the four
    /// options shows, the `attachmentFolderPath` it writes, and the one
    /// dependent row it reveals.
    #[test]
    fn each_attachment_location_writes_its_shape_and_reveals_its_row() {
        let (dir, mut state) = vault();
        let table = [
            ("Vault folder", "/", None),
            ("Same folder as current file", "./", None),
            (
                "In subfolder under current folder",
                "./Attachments",
                Some(("Subfolder name", "Attachments")),
            ),
            (
                "In the folder specified below",
                "Attachments",
                Some(("Attachment folder path", "Attachments")),
            ),
        ];
        assert_eq!(
            ATTACHMENT_LOCATION.options.len(),
            table.len(),
            "the dropdown has four options"
        );
        for (ix, (label, value, dependent)) in table.into_iter().enumerate() {
            assert_eq!(ATTACHMENT_LOCATION.options[ix], (label, value));
            state
                .apply_setting(&(ATTACHMENT_LOCATION.write)(&state, value))
                .unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!(
                state.app().attachment_folder_path.as_deref(),
                Some(value),
                "{label} wrote the wrong value"
            );
            assert_eq!(
                row(&state, "Default location for new attachments").as_deref(),
                Some(label),
                "{label} is not what the row shows"
            );
            for name in ["Attachment folder path", "Subfolder name"] {
                let shown = row(&state, name);
                match dependent {
                    Some((revealed, text)) if revealed == name => {
                        assert_eq!(shown.as_deref(), Some(text), "{label} revealed {name}")
                    }
                    _ => assert!(shown.is_none(), "{label} revealed {name}: {shown:?}"),
                }
            }
        }
    }

    /// The three dependent rows are the ones Obsidian builds but does not
    /// show at the default settings, and a vault with no `app.json` is that
    /// default.
    #[test]
    fn the_dependent_rows_are_hidden_until_an_option_reveals_them() {
        let (dir, state) = vault();
        assert_eq!(ATTACHMENT_LOCATION.label(&state), "Vault folder");
        assert_eq!(NEW_FILE_LOCATION.label(&state), "Vault folder");
        for hidden in [
            "Attachment folder path",
            "Subfolder name",
            "Folder to create new notes in",
        ] {
            assert!(row(&state, hidden).is_none(), "{hidden} was built");
        }
        // An absent key is not the `/` the first option writes — which is
        // what makes the first pick of that option write at all, rather
        // than read as the value already there.
        assert_eq!((ATTACHMENT_LOCATION.read)(&state), "");
        assert_ne!(
            (ATTACHMENT_LOCATION.read)(&state),
            ATTACHMENT_LOCATION.options[0].1
        );
        state
            .apply_setting(&(ATTACHMENT_LOCATION.write)(&state, "/"))
            .unwrap();
        let written = std::fs::read_to_string(dir.path().join(".obsidian/app.json")).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&written).unwrap(),
            json!({ "attachmentFolderPath": "/" }),
            "the first pick left {written}"
        );
    }

    /// A name typed into either dependent row leaves a value that is none
    /// of the four the options write, and that value has to come back as
    /// the option it was typed under.
    #[test]
    fn a_typed_attachment_path_reads_back_into_the_row_it_came_from() {
        let (dir, mut state) = vault();
        let table = [
            (
                ATTACHMENT_FOLDER,
                "att/deep/new",
                "att/deep/new",
                "In the folder specified below",
                "Attachment folder path",
            ),
            (
                SUBFOLDER_NAME,
                "Pictures",
                "./Pictures",
                "In subfolder under current folder",
                "Subfolder name",
            ),
        ];
        for (field, typed, stored, label, name) in table {
            state.apply_setting(&(field.write)(&state, typed)).unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!(state.app().attachment_folder_path.as_deref(), Some(stored));
            assert_eq!(
                row(&state, "Default location for new attachments").as_deref(),
                Some(label),
                "{stored} showed the wrong option"
            );
            assert_eq!(row(&state, name).as_deref(), Some(typed));
        }
    }

    /// "Default location for new notes" is the three-option dropdown over
    /// `newFileLocation` the 2026-10-25 capture recorded — `root`, `current`
    /// and `folder` — and only `folder` reveals the folder row. The spec's
    /// Files-and-links table calls it "the same four values as attachments",
    /// which the same capture contradicts.
    #[test]
    fn each_new_note_location_reveals_only_its_own_row() {
        let (dir, mut state) = vault();
        assert_eq!(
            NEW_FILE_LOCATION.options,
            &[
                ("Vault folder", "root"),
                ("Same folder as current file", "current"),
                ("In the folder specified below", "folder"),
            ][..]
        );
        for (label, value) in NEW_FILE_LOCATION.options {
            state
                .apply_setting(&(NEW_FILE_LOCATION.write)(&state, value))
                .unwrap();
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            assert_eq!(
                row(&state, "Default location for new notes").as_deref(),
                Some(*label)
            );
            assert_eq!(
                row(&state, "Folder to create new notes in").is_some(),
                *value == "folder",
                "{label} and its folder row"
            );
        }
    }

    /// A click with a dropdown open is spent closing it: Obsidian's native
    /// `<select>` swallows the first outside click, so the toggle under the
    /// option list does not flip.
    #[test]
    fn a_click_with_a_dropdown_open_is_spent_closing_it() {
        let mut menu = Some(ATTACHMENT_LOCATION.key);
        assert!(popover_swallows_click(&mut menu));
        assert_eq!(menu, None, "the click closed the popover");
        assert!(
            !popover_swallows_click(&mut menu),
            "the next click goes through"
        );
    }

    /// Escape takes the innermost thing open, so a dropdown closes without
    /// taking the modal with it and a page backs out to its tab.
    #[test]
    fn escape_closes_the_innermost_thing_open() {
        assert_eq!(escape_closes(false, false, false), Escape::Modal);
        assert_eq!(escape_closes(false, true, false), Escape::Popover);
        assert_eq!(escape_closes(true, false, false), Escape::Capture);
        assert_eq!(escape_closes(true, true, false), Escape::Capture);
        // A page is outside a dropdown it holds and inside the modal.
        assert_eq!(escape_closes(false, false, true), Escape::Page);
        assert_eq!(escape_closes(false, true, true), Escape::Popover);
        assert_eq!(escape_closes(true, false, true), Escape::Capture);
    }

    #[test]
    fn a_setting_at_its_default_is_written_rather_than_removed() {
        // Obsidian keeps the key with its explicit value, which the phase 3
        // probe settled for every kind of value; Scoria has no way of
        // removing a config key and does not need one.
        let (dir, state) = vault();
        let default = NEW_LINK_FORMAT.options[0].1;
        state
            .apply_setting(&(NEW_LINK_FORMAT.write)(&state, default))
            .unwrap();
        let written = std::fs::read_to_string(dir.path().join(".obsidian/app.json")).unwrap();
        assert!(
            written.contains("\"newLinkFormat\": \"shortest\""),
            "the default was not written: {written}"
        );
    }

    /// A click on a control while a text field is open changes two settings
    /// at once, and every change carries the whole config file. Both built
    /// from the one reading of the vault, the second write puts back what
    /// the first had just changed — which is why the view commits the field
    /// and defers the control's own change until that write has landed.
    #[test]
    fn a_control_changed_with_a_field_open_has_to_be_built_after_it() {
        let (_dir, state) = vault();
        state
            .apply_setting(&(NEW_FILE_FOLDER.write)(&state, "Before"))
            .unwrap();
        let from_one_reading = Flag::App(FOCUS_NEW_TAB).toggled(&state);
        state
            .apply_setting(&(NEW_FILE_FOLDER.write)(&state, "Notes"))
            .unwrap();
        state.apply_setting(&from_one_reading).unwrap();
        assert_eq!(
            state.app().new_file_folder_path.as_deref(),
            Some("Before"),
            "a change built before the write it lands after no longer \
             puts back what that write changed"
        );

        let (_dir, state) = vault();
        state
            .apply_setting(&(NEW_FILE_FOLDER.write)(&state, "Before"))
            .unwrap();
        state
            .apply_setting(&(NEW_FILE_FOLDER.write)(&state, "Notes"))
            .unwrap();
        let after_the_field = Flag::App(FOCUS_NEW_TAB).toggled(&state);
        state.apply_setting(&after_the_field).unwrap();
        assert_eq!(
            state.app().new_file_folder_path.as_deref(),
            Some("Notes"),
            "the field's value was dropped by the control's write"
        );
        assert_eq!(
            state.app().focus_new_tab,
            Some(false),
            "the toggle's own value was dropped"
        );
    }

    /// `editor:toggle-readable-line-length` and `editor:toggle-line-numbers`
    /// are the Editor tab's two rows run as commands: each builds exactly
    /// the change a click on its row builds, and writes only its own key.
    #[test]
    fn the_toggle_commands_write_what_their_editor_rows_write() {
        for (id, row, key) in [
            (
                "editor:toggle-readable-line-length",
                "Readable line length",
                "readableLineLength",
            ),
            (
                "editor:toggle-line-numbers",
                "Line numbers",
                "showLineNumber",
            ),
        ] {
            let (dir, _) = vault();
            let path = dir.path().join(".obsidian/app.json");
            std::fs::write(&path, "{\n  \"tabSize\": 2\n}").unwrap();
            let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            let flag = SettingsView::editor_items(&state)
                .into_iter()
                .find(|i| i.name.as_ref() == row)
                .and_then(|i| i.flag)
                .unwrap_or_else(|| panic!("no {row} row"));
            let before = flag.is_on(&state);
            let change = toggle_command(id, &state).expect(id);
            assert_eq!(change, flag.toggled(&state), "{id} and its row differ");
            state.apply_setting(&change).unwrap();
            assert_eq!(flag.is_on(&state), !before, "{id} did not flip {key}");
            let written = std::fs::read_to_string(&path).unwrap();
            let json: serde_json::Value = serde_json::from_str(&written).unwrap();
            let keys: Vec<&str> = json
                .as_object()
                .unwrap()
                .keys()
                .map(|k| k.as_str())
                .collect();
            assert_eq!(keys, ["tabSize", key], "{id} wrote {written}");
            assert_eq!(json[key], serde_json::Value::Bool(!before));
            // Run again, it puts the value back and keeps the key.
            // `toggle_command` lets its read guard go before returning, so
            // the write can take the lock.
            let again = toggle_command(id, &state).unwrap();
            state.apply_setting(&again).unwrap();
            assert_eq!(flag.is_on(&state), before);
        }
        let (_dir, state) = vault();
        assert_eq!(toggle_command("editor:toggle-bold", &state), None);
    }

    /// `editor:toggle-spellcheck` over two runs, from no key and from
    /// `false`: `app.json`'s bytes after each are Obsidian 1.15.0's (the
    /// 2026-11-09 `attachments-app-info` probe, `s01`), the key written
    /// even when it goes back to the default.
    #[test]
    fn toggle_spellcheck_writes_obsidians_bytes_over_two_runs() {
        for (start, first, second) in [
            (
                "{}",
                "{\n  \"spellcheck\": false\n}",
                "{\n  \"spellcheck\": true\n}",
            ),
            (
                "{\n  \"spellcheck\": false\n}",
                "{\n  \"spellcheck\": true\n}",
                "{\n  \"spellcheck\": false\n}",
            ),
        ] {
            let (dir, _) = vault();
            let path = dir.path().join(".obsidian/app.json");
            std::fs::write(&path, start).unwrap();
            let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
            let change = toggle_command("editor:toggle-spellcheck", &state).unwrap();
            state.apply_setting(&change).unwrap();
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                first,
                "from {start}"
            );
            let change = toggle_command("editor:toggle-spellcheck", &state).unwrap();
            state.apply_setting(&change).unwrap();
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                second,
                "from {start}"
            );
        }
    }

    /// `app:toggle-ribbon` is the Interface tab's "Show ribbon" row: over
    /// two runs it appends `showRibbon: false` after the keys already there
    /// and then writes `true`, keeping the key, which are Obsidian 1.15.0's
    /// bytes; `workspace.json` and `app.json` are not touched.
    #[test]
    fn toggle_ribbon_writes_show_ribbon_after_the_keys_already_there() {
        let (dir, _) = vault();
        let config = dir.path().join(".obsidian");
        let path = config.join("appearance.json");
        std::fs::write(&path, "{\n  \"baseFontSizeAction\": true\n}").unwrap();
        let app_before = std::fs::read(config.join("app.json")).ok();
        let layout_before = std::fs::read(config.join("workspace.json")).ok();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let flag = SettingsView::interface_items(&state)
            .into_iter()
            .find(|i| i.name.as_ref() == "Show ribbon")
            .and_then(|i| i.flag)
            .expect("a Show ribbon row");
        let change = toggle_command("app:toggle-ribbon", &state).unwrap();
        assert_eq!(
            change,
            flag.toggled(&state),
            "the command and its row differ"
        );
        state.apply_setting(&change).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"baseFontSizeAction\": true,\n  \"showRibbon\": false\n}"
        );
        let again = toggle_command("app:toggle-ribbon", &state).unwrap();
        state.apply_setting(&again).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"baseFontSizeAction\": true,\n  \"showRibbon\": true\n}"
        );
        assert_eq!(std::fs::read(config.join("app.json")).ok(), app_before);
        assert_eq!(
            std::fs::read(config.join("workspace.json")).ok(),
            layout_before
        );
    }

    /// The three rows of the probe: from no key, from `source` and from
    /// `preview`, with the notice each showed; an unknown key rides along.
    #[test]
    fn toggle_default_view_mode_flips_source_and_preview_with_its_notices() {
        let (dir, _) = vault();
        let path = dir.path().join(".obsidian/app.json");
        std::fs::write(&path, "{}").unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let (change, notice) = toggle_default_view_mode(&state.app());
        assert_eq!(notice, "Switched default view mode to reading view.");
        state.apply_setting(&change).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"defaultViewMode\": \"preview\"\n}"
        );
        let (change, notice) = toggle_default_view_mode(&state.app());
        assert_eq!(notice, "Switched default view mode to editing view.");
        state.apply_setting(&change).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"defaultViewMode\": \"source\"\n}"
        );
        let (change, notice) = toggle_default_view_mode(&state.app());
        assert_eq!(notice, "Switched default view mode to reading view.");
        state.apply_setting(&change).unwrap();

        std::fs::write(
            &path,
            "{\n  \"probeUnknown\": [1, 2],\n  \"defaultViewMode\": \"preview\"\n}",
        )
        .unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let (change, _) = toggle_default_view_mode(&state.app());
        state.apply_setting(&change).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["defaultViewMode"], "source");
        assert_eq!(json["probeUnknown"], serde_json::json!([1, 2]));
    }

    /// `obsidian` and `moonstone` swap, the file's other keys kept; under
    /// `system`, no key, or a value Obsidian does not know, nothing is
    /// written.
    #[test]
    fn toggle_light_dark_swaps_the_two_base_themes_and_leaves_system_alone() {
        let (dir, _) = vault();
        let path = dir.path().join(".obsidian/appearance.json");
        std::fs::write(
            &path,
            "{\n  \"accentColor\": \"#ff5721\",\n  \"theme\": \"obsidian\",\n  \"probeUnknown\": true\n}",
        )
        .unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let next = toggle_light_dark(&state.appearance()).expect("a swap");
        state
            .apply_setting(&SettingChange::Appearance(next))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"accentColor\": \"#ff5721\",\n  \"theme\": \"moonstone\",\n  \"probeUnknown\": true\n}"
        );
        let back = toggle_light_dark(&state.appearance()).expect("a swap");
        assert_eq!(back.theme.as_deref(), Some("obsidian"));
        for theme in [Some("system"), None, Some("probe")] {
            let config = AppearanceConfig {
                theme: theme.map(str::to_string),
                ..AppearanceConfig::default()
            };
            assert_eq!(toggle_light_dark(&config), None, "{theme:?}");
        }
    }

    /// Obsidian words this one the other way round: the control is "Use
    /// [[Wikilinks]]", the key is `useMarkdownLinks`.
    #[test]
    fn the_wikilinks_toggle_is_use_markdown_links_negated() {
        let mut config = AppConfig::default();
        assert!(USE_WIKILINKS.is_on(&config), "wikilinks are the default");
        config = USE_WIKILINKS.toggled(&config);
        assert_eq!(config.use_markdown_links, Some(true));
        assert!(!USE_WIKILINKS.is_on(&config));
        config = USE_WIKILINKS.toggled(&config);
        assert_eq!(config.use_markdown_links, Some(false));
        assert!(USE_WIKILINKS.is_on(&config));
    }

    #[test]
    fn a_toggle_shows_the_file_first_and_the_default_when_the_key_is_absent() {
        let mut config = AppConfig::default();
        assert!(SPELLCHECK.is_on(&config), "spellcheck is on by default");
        assert!(!SHOW_LINE_NUMBER.is_on(&config), "line numbers are not");
        config.spellcheck = Some(false);
        assert!(!SPELLCHECK.is_on(&config));
    }

    /// The appearance and switcher files have their own flags; the same
    /// rules hold for them.
    #[test]
    fn the_other_files_toggles_flip_too() {
        let appearance = SHOW_RIBBON.toggled(&AppearanceConfig::default());
        assert_eq!(appearance.show_ribbon, Some(false));
        let switcher = SHOW_EXISTING_ONLY.toggled(&SwitcherConfig::default());
        assert_eq!(switcher.show_existing_only, Some(true));
    }

    /// A vault with no `core-plugins.json` gets the whole default map, not
    /// a one-key file that would read as "everything else off".
    #[test]
    fn a_missing_core_plugins_file_is_written_whole() {
        let plugins = toggled_core_plugin(None, "graph", true);
        let CorePlugins::Map(map) = &plugins else {
            panic!("expected a map");
        };
        assert_eq!(map.len(), CORE_PLUGINS.len());
        assert_eq!(map.get("graph"), Some(&false), "the toggled one is off");
        assert_eq!(
            map.get("backlink"),
            Some(&true),
            "the others keep their defaults"
        );
    }

    /// A vault to build the tabs against; `VaultState::load` wants a real
    /// folder.
    fn vault() -> (tempfile::TempDir, VaultState) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        std::fs::write(root.join("Note.md"), "# Note\n").unwrap();
        let state = VaultState::load(&root.to_path_buf()).unwrap();
        (dir, state)
    }

    /// A vault with two themes installed and three snippets, plus the two
    /// theme folders Obsidian drops: one whose manifest names something
    /// other than its folder, one with no manifest at all.
    fn vault_with_css() -> (tempfile::TempDir, VaultState) {
        let (dir, _) = vault();
        let cfg = dir.path().join(".obsidian");
        let theme = |name: &str, manifest: Option<&str>| {
            let folder = cfg.join("themes").join(name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("theme.css"), "body { --h1-color: #f00 }").unwrap();
            if let Some(manifest) = manifest {
                std::fs::write(
                    folder.join("manifest.json"),
                    format!(r#"{{"name":"{manifest}","version":"1.0.0","minAppVersion":"1.0.0","author":"t"}}"#),
                )
                .unwrap();
            }
        };
        theme("Ember", Some("Ember"));
        theme("Zephyr", Some("Zephyr"));
        theme("folder-name", Some("Display Name"));
        theme("Bare", None);
        std::fs::create_dir_all(cfg.join("snippets")).unwrap();
        for name in ["wider", "dim", "tabs"] {
            std::fs::write(
                cfg.join("snippets").join(format!("{name}.css")),
                "body { --file-line-width: 1100px }",
            )
            .unwrap();
        }
        std::fs::write(cfg.join("snippets").join("notes.txt"), "not css").unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        (dir, state)
    }

    /// The Appearance tab, row for row as Obsidian 1.14.5 builds it
    /// ([[Specification#The Appearance tab, Obsidian 1.14.5]]).
    #[test]
    fn the_appearance_tab_builds_obsidians_rows() {
        let (_dir, state) = vault_with_css();
        let css = CssFolders::read(&state);
        let rows: Vec<(String, &'static str)> = SettingsView::appearance_items(&state, &css, None)
            .iter()
            .map(|i| {
                let kind = match &i.control {
                    _ if i.heading => "heading",
                    Control::Dropdown(_) => "dropdown",
                    Control::Color(_) => "color",
                    Control::Navigable(_) => "navigable",
                    Control::AppIcon(_) => "app_icon",
                    Control::Button(_) => "button",
                    Control::Text(_) => "text",
                    Control::Slider(_) => "slider",
                    Control::Toggle(_) => "toggle",
                    Control::None => "none",
                };
                (i.name.to_string(), kind)
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("Base color scheme".into(), "dropdown"),
                ("Accent color".into(), "color"),
                ("Themes".into(), "dropdown"),
                ("Current community themes".into(), "button"),
                ("CSS snippets".into(), "navigable"),
                ("Font".into(), "heading"),
                ("Interface font".into(), "navigable"),
                ("Text font".into(), "navigable"),
                ("Monospace font".into(), "navigable"),
                ("Font size".into(), "slider"),
                ("Quick font size adjustment".into(), "toggle"),
                ("Advanced".into(), "heading"),
                ("Custom app icon".into(), "app_icon"),
            ]
        );
        let items = SettingsView::appearance_items(&state, &css, None);
        let row = |name: &str| {
            items
                .iter()
                .find(|i| i.name.as_ref() == name)
                .unwrap_or_else(|| panic!("no {name} row"))
                .clone()
        };
        // The count is of themes installed, and the plural follows it.
        assert_eq!(
            row("Current community themes").desc.as_ref(),
            "You currently have 2 themes installed."
        );
        // Nothing enabled reads "0 enabled", not "None".
        assert_eq!(
            row("CSS snippets").control,
            Control::Navigable("0 enabled".into())
        );
        // An empty font list reads nothing before the chevron.
        assert_eq!(row("Text font").control, Control::Navigable("".into()));
        assert_eq!(row("Text font").page, Some(Page::Font(FontSlot::Text)));
        // The icon's description is Obsidian's, with no full stop.
        assert_eq!(
            row("Custom app icon").desc.as_ref(),
            "Set a custom icon for the app"
        );
        // Every vault row of this tab is read live now that the loader
        // rebuilds the theme from `appearance.json` on every write; the
        // icon is the machine's and only X11 takes it, from the next start.
        for item in &items {
            let want = match item.name.as_ref() {
                "Current community themes" => Honored::No,
                "Custom app icon" if cfg!(target_os = "linux") => Honored::OnRestart,
                "Custom app icon" => Honored::No,
                _ => Honored::Live,
            };
            assert_eq!(item.honored, want, "{}", item.name);
        }
    }

    /// The font rows read the list as Obsidian's row does: nothing, the one
    /// family, or a count (step s06 of `font-pages.mjs`).
    #[test]
    fn a_font_row_reads_its_list() {
        let (dir, _) = vault();
        std::fs::write(
            dir.path().join(".obsidian/appearance.json"),
            r#"{"textFontFamily":"Consolas,Cascadia Mono","monospaceFontFamily":"Consolas","interfaceFontFamily":""}"#,
        )
        .unwrap();
        let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let css = CssFolders::read(&state);
        let items = SettingsView::appearance_items(&state, &css, Some("icon.png"));
        let value = |name: &str| {
            items
                .iter()
                .find(|i| i.name.as_ref() == name)
                .map(|i| i.control.clone())
                .unwrap()
        };
        assert_eq!(value("Interface font"), Control::Navigable("".into()));
        assert_eq!(value("Text font"), Control::Navigable("2 fonts".into()));
        assert_eq!(
            value("Monospace font"),
            Control::Navigable("Consolas".into())
        );
        // A chosen icon replaces the description with its picture.
        let icon = items
            .iter()
            .find(|i| i.name.as_ref() == "Custom app icon")
            .unwrap();
        assert_eq!(icon.control, Control::AppIcon("icon.png".into()));
        assert!(icon.desc.is_empty());
    }

    /// What each gesture on a font page does to the list, and the bytes it
    /// writes: Obsidian's own run, step for step (`font-pages.mjs` s02-s05).
    #[test]
    fn the_font_page_edits_write_what_obsidian_writes() {
        let slot = FontSlot::Interface;
        let mut a = AppearanceConfig::default();
        let step = |edit: FontEdit, a: &mut AppearanceConfig| -> String {
            let next = edit
                .apply(&slot.read(a))
                .expect("the edit changes the list");
            *a = slot.write(a, &next);
            a.interface_font_family.clone().unwrap()
        };
        assert_eq!(step(FontEdit::Add("Segoe UI".into()), &mut a), "Segoe UI");
        assert_eq!(
            step(FontEdit::Add("Arial".into()), &mut a),
            "Segoe UI,Arial"
        );
        assert_eq!(
            step(FontEdit::Add("Courier New".into()), &mut a),
            "Segoe UI,Arial,Courier New"
        );
        // A typed name the machine lacks goes on the end like any other.
        assert_eq!(
            step(FontEdit::Add("Probe Sans".into()), &mut a),
            "Segoe UI,Arial,Courier New,Probe Sans"
        );
        // The last row dragged onto the first.
        assert_eq!(
            step(FontEdit::Move { from: 3, to: 0 }, &mut a),
            "Probe Sans,Segoe UI,Arial,Courier New"
        );
        // Delete from the middle out; the last one leaves "".
        assert_eq!(
            step(FontEdit::Remove(2), &mut a),
            "Probe Sans,Segoe UI,Courier New"
        );
        assert_eq!(step(FontEdit::Remove(1), &mut a), "Probe Sans,Courier New");
        assert_eq!(step(FontEdit::Remove(1), &mut a), "Probe Sans");
        assert_eq!(step(FontEdit::Remove(0), &mut a), "");
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"interfaceFontFamily":""}"#);
        // Edits that change nothing write nothing.
        let list = vec!["Arial".to_string()];
        assert_eq!(FontEdit::Add("arial".into()).apply(&list), None);
        assert_eq!(FontEdit::Add("  ".into()).apply(&list), None);
        assert_eq!(FontEdit::Remove(1).apply(&list), None);
        assert_eq!(FontEdit::Move { from: 0, to: 0 }.apply(&list), None);
        // The other two fields are written under their own keys.
        let text = FontSlot::Text.write(&AppearanceConfig::default(), &["Consolas".into()]);
        assert_eq!(text.text_font_family.as_deref(), Some("Consolas"));
        let mono = FontSlot::Monospace.write(&AppearanceConfig::default(), &["Consolas".into()]);
        assert_eq!(mono.monospace_font_family.as_deref(), Some("Consolas"));
    }

    /// The lower card: the installed families not yet applied that contain
    /// the query, and the typed name first when it names none of them —
    /// "cour" offered itself and Courier, with Courier New already applied.
    #[test]
    fn the_font_page_offers_the_typed_name_and_the_matches() {
        let installed: Vec<String> = ["Arial", "Courier", "Courier New", "Segoe UI"]
            .map(String::from)
            .to_vec();
        let applied = vec!["Courier New".to_string()];
        let (typed, rows) = font_choices(&installed, &applied, "");
        assert_eq!(typed, None);
        assert_eq!(rows, ["Arial", "Courier", "Segoe UI"]);
        let (typed, rows) = font_choices(&installed, &applied, "cour");
        assert_eq!(typed.as_deref(), Some("cour"));
        assert_eq!(rows, ["Courier"]);
        let (typed, rows) = font_choices(&installed, &applied, " Probe Sans ");
        assert_eq!(typed.as_deref(), Some("Probe Sans"));
        assert!(rows.is_empty());
        // A name that is a family already listed is not offered twice.
        assert_eq!(font_choices(&installed, &applied, "arial").0, None);
        assert_eq!(font_choices(&installed, &applied, "courier new").0, None);
    }

    /// "Quick font size adjustment" writes `baseFontSizeAction`, `true`
    /// then `false`, and keeps the key (step s07).
    #[test]
    fn quick_font_size_writes_base_font_size_action() {
        let on = QUICK_FONT_SIZE.toggled(&AppearanceConfig::default());
        assert_eq!(
            serde_json::to_string(&on).unwrap(),
            r#"{"baseFontSizeAction":true}"#
        );
        let off = QUICK_FONT_SIZE.toggled(&on);
        assert_eq!(
            serde_json::to_string(&off).unwrap(),
            r#"{"baseFontSizeAction":false}"#
        );
    }

    /// The Themes dropdown lists what the vault has installed, which is a
    /// folder holding a `manifest.json` that names the folder — and picking
    /// one writes `cssTheme`, with the empty string for Default.
    #[test]
    fn the_themes_picker_lists_installed_themes_only() {
        let (dir, mut state) = vault_with_css();
        let css = CssFolders::read(&state);
        assert_eq!(css.themes, vec!["Ember".to_string(), "Zephyr".to_string()]);
        let items = SettingsView::appearance_items(&state, &css, None);
        let themes = items.iter().find(|i| i.name.as_ref() == "Themes").unwrap();
        let picker = themes.picker.clone().expect("the row carries a picker");
        assert_eq!(picker.key, "cssTheme");
        assert_eq!(
            picker.options,
            vec![
                (SharedString::from("Default"), String::new()),
                ("Ember".into(), "Ember".into()),
                ("Zephyr".into(), "Zephyr".into()),
            ]
        );
        assert_eq!(picker.selected, 0);
        // Picking one writes the name; Default writes the empty string
        // rather than dropping the key, as Obsidian does.
        let mut appearance = state.appearance().clone();
        appearance.css_theme = Some("Zephyr".into());
        state
            .apply_setting(&SettingChange::Appearance(appearance))
            .unwrap();
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        let items = SettingsView::appearance_items(&state, &css, None);
        let themes = items.iter().find(|i| i.name.as_ref() == "Themes").unwrap();
        assert_eq!(themes.control, Control::Dropdown("Zephyr".into()));
        assert_eq!(themes.picker.clone().unwrap().selected, 2);
        // And the theme's CSS is what the loader then reads.
        let sheets = crate::theme::sheets_of(&state.root, &state.config_dir(), &state.appearance());
        assert_eq!(sheets.len(), 1);
    }

    /// A snippet toggle appends its name on the way on and takes it out on
    /// the way off, which is the order `enabledCssSnippets` is written in.
    #[test]
    fn a_snippet_toggle_writes_the_enabled_list_in_click_order() {
        let (dir, mut state) = vault_with_css();
        let css = CssFolders::read(&state);
        assert_eq!(
            css.snippets,
            vec!["dim".to_string(), "tabs".into(), "wider".into()],
            "sorted, and the .txt is not one"
        );
        let on = |state: &VaultState, name: &str| {
            state
                .apply_setting(&Flag::CssSnippet(name.to_string()).toggled(state))
                .unwrap();
        };
        for name in ["tabs", "wider", "dim"] {
            on(&state, name);
            state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        }
        assert_eq!(
            state.appearance().enabled_css_snippets,
            Some(vec!["tabs".into(), "wider".into(), "dim".into()])
        );
        assert_eq!(
            CssFolders::read(&state).enabled_count(&state.appearance()),
            3
        );
        // Off again takes the name out and leaves the rest in place.
        on(&state, "wider");
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!(
            state.appearance().enabled_css_snippets,
            Some(vec!["tabs".into(), "dim".into()])
        );
        // The rows read it back, in the folder's order whatever the list's.
        let rows = SettingsView::snippet_items(&state, &CssFolders::read(&state), "");
        assert_eq!(
            rows.iter()
                .map(|i| (i.name.to_string(), i.control.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("dim".into(), Control::Toggle(true)),
                ("tabs".into(), Control::Toggle(true)),
                ("wider".into(), Control::Toggle(false)),
            ]
        );
        // The page's search box filters them.
        let found = SettingsView::snippet_items(&state, &CssFolders::read(&state), "TAB");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name.as_ref(), "tabs");
        // A name whose file is gone stays in the file and stops counting.
        std::fs::remove_file(dir.path().join(".obsidian/snippets/dim.css")).unwrap();
        let css = CssFolders::read(&state);
        assert_eq!(
            state.appearance().enabled_css_snippets,
            Some(vec!["tabs".into(), "dim".into()])
        );
        assert_eq!(css.enabled_count(&state.appearance()), 1);
        assert_eq!(
            SettingsView::appearance_items(&state, &css, None)
                .iter()
                .find(|i| i.name.as_ref() == "CSS snippets")
                .unwrap()
                .control,
            Control::Navigable("1 enabled".into())
        );
    }

    /// The accent row stores a `#rrggbb` in lower case for whatever shape
    /// was typed, and "Restore default" writes the empty string rather
    /// than dropping the key.
    #[test]
    fn the_accent_row_stores_a_hex_and_restores_to_empty() {
        assert_eq!(normalize_accent("#E11D48"), "#e11d48");
        assert_eq!(normalize_accent("e11d48"), "#e11d48");
        assert_eq!(normalize_accent(" #f0a "), "#ff00aa");
        for bad in ["", "   ", "#12345", "rebeccapurple", "#gggggg"] {
            assert_eq!(normalize_accent(bad), "", "{bad}");
        }
        let (dir, mut state) = vault_with_css();
        state
            .apply_setting(&(ACCENT_COLOR.write)(&state, "#E11D48"))
            .unwrap();
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!(state.appearance().accent_color.as_deref(), Some("#e11d48"));
        assert_eq!((ACCENT_COLOR.read)(&state), "#e11d48");
        // The swatch is that colour, and the accent reaches the theme.
        assert!(crate::theme::accent_swatch("#e11d48").is_some());
        let themed =
            crate::theme::Theme::load(&state.root, &state.config_dir(), &state.appearance(), true);
        assert!(
            (themed.tokens.accent_h - 347.).abs() < 1.,
            "{}",
            themed.tokens.accent_h
        );
        // Restore default: the key stays, holding the empty string.
        state
            .apply_setting(&(ACCENT_COLOR.write)(&state, ""))
            .unwrap();
        state = VaultState::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!(state.appearance().accent_color.as_deref(), Some(""));
        assert!(crate::theme::accent_swatch("").is_none());
    }

    /// The picker's colour model: a hex round-trips through it, the
    /// square and the bar map a point to the channel each one moves, and a
    /// picker opened on "no accent set" starts from the theme's own.
    #[test]
    fn the_accent_picker_maps_points_to_colours() {
        for hex in [
            "#be4f2d", "#ff5721", "#000000", "#ffffff", "#8a5cf6", "#00ff80",
        ] {
            let hsv = Hsv::of_hex(hex).unwrap();
            assert_eq!(hsv.hex(), hex);
        }
        assert_eq!(Hsv::of_hex(""), None);
        assert_eq!(Hsv::of_hex("nope"), None);
        let brick = Hsv::of_hex("#be4f2d").unwrap();
        assert!((brick.h - 14.).abs() < 1., "{}", brick.h);

        let square = Bounds::new(gpui::point(px(10.), px(20.)), size(px(200.), px(100.)));
        assert_eq!(square_pick(square, gpui::point(px(10.), px(20.))), (0., 1.));
        assert_eq!(
            square_pick(square, gpui::point(px(210.), px(120.))),
            (1., 0.)
        );
        assert_eq!(
            square_pick(square, gpui::point(px(110.), px(70.))),
            (0.5, 0.5)
        );
        // Past the edges the pick stays on them.
        assert_eq!(
            square_pick(square, gpui::point(px(-50.), px(500.))),
            (0., 0.)
        );
        let bar = Bounds::new(gpui::point(px(0.), px(0.)), size(px(360.), px(12.)));
        assert_eq!(bar_pick(bar, gpui::point(px(90.), px(6.))), 90.);
        assert_eq!(bar_pick(bar, gpui::point(px(9999.), px(6.))), 359.);

        let fallback = hsla(14. / 360., 0.62, 0.46, 1.);
        let fresh = AccentPicker::new("", fallback);
        assert_eq!(fresh.hex_for_test(), "#be4f2d");
        assert_eq!(fresh.from, "");
        let set = AccentPicker::new("#E11D48", fallback);
        assert_eq!(set.hex_for_test(), "#e11d48");
        assert_eq!(set.from, "#e11d48");
        // Every preset is a hex the file can hold, or the empty default.
        for (_, hex) in PICKER_PRESETS {
            assert!(hex.is_empty() || Hsv::of_hex(hex).is_some(), "{hex}");
        }
    }

    impl AccentPicker {
        fn hex_for_test(&self) -> String {
            self.hsv.hex()
        }
    }

    /// The frame style is the one dropdown whose value is the machine's,
    /// not the vault's: its options are 1.14.5's three, a pick is a
    /// `SettingChange::Frame` bound for `scoria.json`, and the row is tagged
    /// "on restart" because the window keeps the frame it opened with.
    #[test]
    fn the_frame_style_dropdown_writes_to_the_registry_not_the_vault() {
        let (_dir, state) = vault();
        assert_eq!(
            FRAME_STYLE.options,
            &[
                ("Hidden (default)", "hidden"),
                ("Obsidian frame", "custom"),
                ("Native frame", "native"),
            ]
        );
        for (_, value) in FRAME_STYLE.options {
            match (FRAME_STYLE.write)(&state, value) {
                SettingChange::Frame(v) => assert_eq!(v, *value),
                other => panic!("{value} wrote {}", other.file()),
            }
        }
        assert_eq!(
            SettingChange::Frame("native".into()).file(),
            scoria_vault::registry::FILE_NAME
        );
        let row = SettingsView::interface_items(&state)
            .into_iter()
            .find(|i| i.name.as_ref() == "Window frame style")
            .expect("the Interface tab has the row");
        assert_eq!(row.typed, Some(FRAME_STYLE));
        assert_eq!(row.honored, Honored::OnRestart);
    }

    /// Every toggle in the modal, with how far Scoria gets with it.
    fn toggles(state: &VaultState) -> Vec<(String, Honored)> {
        let tabs = [
            SettingsView::general_items(state),
            SettingsView::appearance_items(state, &CssFolders::read(state), None),
            SettingsView::interface_items(state),
            SettingsView::editor_items(state),
            SettingsView::files_items(state),
            SettingsView::core_plugin_items(state),
            SettingsView::community_plugin_items(state),
            SettingsView::plugin_items(state, &[], "switcher", None),
            SettingsView::plugin_items(state, &[], "backlink", None),
            SettingsView::plugin_items(state, &[], "canvas", None),
            SettingsView::plugin_items(state, &[], "page-preview", None),
            SettingsView::plugin_items(state, &[], "note-composer", None),
            SettingsView::plugin_items(state, &[], "webviewer", None),
        ];
        tabs.into_iter()
            .flatten()
            .filter(|i| i.flag.is_some())
            .map(|i| (i.name.to_string(), i.honored))
            .collect()
    }

    /// The honesty ratchet: a toggle goes untagged only when Scoria really
    /// reads the key. Implementing one of the tagged settings means
    /// dropping its `not_yet()` here as well, which is the point.
    #[test]
    fn only_the_toggles_scoria_reads_are_untagged() {
        let (_dir, state) = vault();
        let live: Vec<String> = toggles(&state)
            .into_iter()
            .filter(|(_, honored)| *honored == Honored::Live)
            .map(|(name, _)| name)
            .collect();
        assert_eq!(
            live,
            [
                "Quick font size adjustment",
                "Show ribbon",
                "Readable line length",
                "Strict line breaks",
                "Auto-pair brackets",
                "Auto-pair Markdown syntax",
                "Smart lists",
                "Indent using tabs",
                "Automatically update internal links",
                "Show all file types",
                "Confirm before deleting files",
                // The Quick switcher tab's three, read by `QuickSwitcher::set_state`.
                "Show existing only",
                "Show attachments",
                "Show all file types",
                // The Backlinks tab's one, read by the backlinks block.
                "Show backlinks at the bottom of notes",
                // The Note composer's, read by `note-composer:merge-file`.
                "Confirm file merge",
            ],
            "a toggle claims Scoria reads it"
        );
        // And the other way round: every toggle the new options tabs added
        // is tagged, because nothing in the app reads one of their keys.
        // `grep -rn snapToGrid crates/` and its friends find the key in
        // `scoria-formats` and in this file and nowhere else.
        let tagged: Vec<String> = toggles(&state)
            .into_iter()
            .filter(|(_, honored)| *honored == Honored::No)
            .map(|(name, _)| name)
            .collect();
        for name in [
            "Snap to grid",
            "Snap to objects",
            "Search, Backlinks, and Outgoing links",
            "Reading view",
            "Editing view",
            "Tab header",
            "Files",
            "Properties view",
            "Bookmarks",
            "Outline",
            "Bases",
            "Graph view",
            "Open external links",
            "Enable ad blocker",
        ] {
            assert!(
                tagged.iter().any(|n| n == name),
                "{name} is untagged, so something must read its key"
            );
        }
    }

    /// Every tab builds, and the ones that write carry a flag.
    #[test]
    fn every_toggle_in_every_tab_writes_something() {
        let (_dir, state) = vault();
        let toggles = toggles(&state);
        assert!(
            toggles.len() > 40,
            "only {} toggles were built",
            toggles.len()
        );
        for (name, _) in &toggles {
            assert!(!name.is_empty(), "a toggle has no name");
        }
    }

    #[test]
    fn toggling_a_core_plugin_keeps_the_file_as_it_was() {
        let existing = CorePlugins::Map(
            [("graph".to_string(), true), ("canvas".to_string(), false)]
                .into_iter()
                .collect(),
        );
        let next = toggled_core_plugin(Some(&existing), "graph", true);
        let CorePlugins::Map(map) = &next else {
            panic!("expected a map");
        };
        assert_eq!(map.len(), 2, "no plugin was added");
        assert_eq!(map.keys().next().map(String::as_str), Some("graph"));
        assert_eq!(map.get("graph"), Some(&false));
        // The legacy array shape keeps its shape.
        let list = CorePlugins::List(vec!["graph".into()]);
        let next = toggled_core_plugin(Some(&list), "graph", true);
        assert_eq!(next, CorePlugins::List(Vec::new()));
        let next = toggled_core_plugin(Some(&next), "graph", false);
        assert_eq!(next, CorePlugins::List(vec!["graph".into()]));
    }
}
