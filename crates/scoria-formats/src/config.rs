//! `.obsidian/*.json` configuration files.
//!
//! Obsidian writes these sparsely (only keys that differ from defaults), so every
//! typed field is optional and unknown keys are kept in `extra`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Default name of the configuration directory; overridable per vault.
pub const DEFAULT_CONFIG_DIR: &str = ".obsidian";

/// `app.json`: Editor plus Files & links settings (key names exactly as Obsidian writes them).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    // Editor
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spellcheck: Option<bool>,
    #[serde(
        default,
        rename = "spellcheckLanguages",
        skip_serializing_if = "Option::is_none"
    )]
    pub spellcheck_languages: Option<Vec<String>>,
    #[serde(
        default,
        rename = "autoPairBrackets",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_pair_brackets: Option<bool>,
    #[serde(
        default,
        rename = "autoPairMarkdown",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_pair_markdown: Option<bool>,
    #[serde(
        default,
        rename = "smartIndentList",
        skip_serializing_if = "Option::is_none"
    )]
    pub smart_indent_list: Option<bool>,
    #[serde(default, rename = "useTab", skip_serializing_if = "Option::is_none")]
    pub use_tab: Option<bool>,
    #[serde(default, rename = "tabSize", skip_serializing_if = "Option::is_none")]
    pub tab_size: Option<u8>,
    #[serde(
        default,
        rename = "autoConvertHtml",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_convert_html: Option<bool>,
    #[serde(default, rename = "vimMode", skip_serializing_if = "Option::is_none")]
    pub vim_mode: Option<bool>,
    #[serde(
        default,
        rename = "showInlineTitle",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_inline_title: Option<bool>,
    #[serde(
        default,
        rename = "readableLineLength",
        skip_serializing_if = "Option::is_none"
    )]
    pub readable_line_length: Option<bool>,
    #[serde(
        default,
        rename = "strictLineBreaks",
        skip_serializing_if = "Option::is_none"
    )]
    pub strict_line_breaks: Option<bool>,
    /// `visible` | `hidden` | `source`
    #[serde(
        default,
        rename = "propertiesInDocument",
        skip_serializing_if = "Option::is_none"
    )]
    pub properties_in_document: Option<String>,
    #[serde(
        default,
        rename = "foldHeading",
        skip_serializing_if = "Option::is_none"
    )]
    pub fold_heading: Option<bool>,
    #[serde(
        default,
        rename = "foldIndent",
        skip_serializing_if = "Option::is_none"
    )]
    pub fold_indent: Option<bool>,
    #[serde(
        default,
        rename = "showLineNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_line_number: Option<bool>,
    #[serde(
        default,
        rename = "showIndentGuide",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_indent_guide: Option<bool>,
    #[serde(
        default,
        rename = "rightToLeft",
        skip_serializing_if = "Option::is_none"
    )]
    pub right_to_left: Option<bool>,
    /// `source` | `preview`
    #[serde(
        default,
        rename = "defaultViewMode",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_view_mode: Option<String>,
    #[serde(
        default,
        rename = "livePreview",
        skip_serializing_if = "Option::is_none"
    )]
    pub live_preview: Option<bool>,
    #[serde(
        default,
        rename = "focusNewTab",
        skip_serializing_if = "Option::is_none"
    )]
    pub focus_new_tab: Option<bool>,
    // Files and links
    #[serde(
        default,
        rename = "promptDelete",
        skip_serializing_if = "Option::is_none"
    )]
    pub prompt_delete: Option<bool>,
    /// `system` | `local` | `none`
    #[serde(
        default,
        rename = "trashOption",
        skip_serializing_if = "Option::is_none"
    )]
    pub trash_option: Option<String>,
    #[serde(
        default,
        rename = "alwaysUpdateLinks",
        skip_serializing_if = "Option::is_none"
    )]
    pub always_update_links: Option<bool>,
    /// `root` | `current` | `folder`
    #[serde(
        default,
        rename = "newFileLocation",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_file_location: Option<String>,
    #[serde(
        default,
        rename = "newFileFolderPath",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_file_folder_path: Option<String>,
    #[serde(
        default,
        rename = "useMarkdownLinks",
        skip_serializing_if = "Option::is_none"
    )]
    pub use_markdown_links: Option<bool>,
    /// `shortest` | `relative` | `absolute`
    #[serde(
        default,
        rename = "newLinkFormat",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_link_format: Option<String>,
    #[serde(
        default,
        rename = "showUnsupportedFiles",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_unsupported_files: Option<bool>,
    /// Vault root, a folder path, or `./subfolder` for a subfolder under the current folder.
    #[serde(
        default,
        rename = "attachmentFolderPath",
        skip_serializing_if = "Option::is_none"
    )]
    pub attachment_folder_path: Option<String>,
    #[serde(
        default,
        rename = "userIgnoreFilters",
        skip_serializing_if = "Option::is_none"
    )]
    pub user_ignore_filters: Option<Vec<String>>,
    /// Export to PDF's last values, written by every export (observed
    /// 2026-11-10, Obsidian 1.15.0).
    #[serde(
        default,
        rename = "pdfExportSettings",
        skip_serializing_if = "Option::is_none"
    )]
    pub pdf_export_settings: Option<PdfExportSettings>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `app.json`'s `pdfExportSettings`, in the key order Obsidian writes:
/// `includeName` (only once the toggle has been touched), `pageSize`,
/// `landscape`, `margin` (a **string**: `"0"` Default, `"1"` None, `"2"`
/// Minimal) and `downscalePercent`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PdfExportSettings {
    #[serde(
        default,
        rename = "includeName",
        skip_serializing_if = "Option::is_none"
    )]
    pub include_name: Option<bool>,
    #[serde(default, rename = "pageSize", skip_serializing_if = "Option::is_none")]
    pub page_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landscape: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<String>,
    #[serde(
        default,
        rename = "downscalePercent",
        skip_serializing_if = "Option::is_none"
    )]
    pub downscale_percent: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `appearance.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// `obsidian` (dark) | `moonstone` (light) | `system`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Installed community theme name, or empty for default.
    #[serde(default, rename = "cssTheme", skip_serializing_if = "Option::is_none")]
    pub css_theme: Option<String>,
    #[serde(
        default,
        rename = "accentColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub accent_color: Option<String>,
    #[serde(
        default,
        rename = "baseFontSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub base_font_size: Option<u32>,
    /// "Quick font size adjustment": Ctrl+wheel over a note changes the
    /// font size for the session. The toggle writes `true` and then
    /// `false`, never dropping the key; the size the wheel reaches is not
    /// written anywhere (probed 2026-11-04).
    #[serde(
        default,
        rename = "baseFontSizeAction",
        skip_serializing_if = "Option::is_none"
    )]
    pub base_font_size_action: Option<bool>,
    /// The font fields hold the families picked on the font pages, joined
    /// by a bare `,` in the order of the page's list, unquoted:
    /// `Segoe UI,Arial,Courier New`. Removing the last one writes `""`.
    #[serde(
        default,
        rename = "interfaceFontFamily",
        skip_serializing_if = "Option::is_none"
    )]
    pub interface_font_family: Option<String>,
    #[serde(
        default,
        rename = "textFontFamily",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_font_family: Option<String>,
    #[serde(
        default,
        rename = "monospaceFontFamily",
        skip_serializing_if = "Option::is_none"
    )]
    pub monospace_font_family: Option<String>,
    #[serde(
        default,
        rename = "enabledCssSnippets",
        skip_serializing_if = "Option::is_none"
    )]
    pub enabled_css_snippets: Option<Vec<String>>,
    #[serde(
        default,
        rename = "showViewHeader",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_view_header: Option<bool>,
    #[serde(
        default,
        rename = "showRibbon",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_ribbon: Option<bool>,
    #[serde(
        default,
        rename = "nativeMenus",
        skip_serializing_if = "Option::is_none"
    )]
    pub native_menus: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translucency: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One binding in `hotkeys.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    /// Any of `Mod`, `Ctrl`, `Meta`, `Shift`, `Alt`.
    pub modifiers: Vec<String>,
    /// A `KeyboardEvent.key` name such as `P`, `Enter`, `ArrowLeft`.
    pub key: String,
}

/// `hotkeys.json`: command id to its user overrides; an empty list unbinds
/// the command. Insertion order is kept, like [`PropertyTypes`]: Obsidian
/// writes the file in the order the bindings were made, so a sorted map
/// would reorder the whole file the first time Scoria saved it.
pub type Hotkeys = IndexMap<String, Vec<Hotkey>>;

/// `core-plugins.json`. Current builds write a map of id to enabled; older
/// builds wrote an array of enabled ids. Both are accepted.
///
/// The map keeps the file's order: Obsidian writes the plugins in its own
/// registration order, not alphabetically, so a sorted map would rewrite
/// every line the first time Scoria saved the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CorePlugins {
    Map(IndexMap<String, bool>),
    List(Vec<String>),
}

impl CorePlugins {
    pub fn is_enabled(&self, id: &str) -> bool {
        match self {
            CorePlugins::Map(m) => m.get(id).copied().unwrap_or(false),
            CorePlugins::List(l) => l.iter().any(|x| x == id),
        }
    }
}

/// `community-plugins.json`: enabled community plugin ids.
pub type CommunityPlugins = Vec<String>;

/// A color in `graph.json`: alpha plus a packed `0xRRGGBB`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphColor {
    #[serde(default = "one")]
    pub a: f32,
    #[serde(default)]
    pub rgb: u32,
}

fn one() -> f32 {
    1.0
}

/// One entry of `colorGroups`: nodes matching a search query get a color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphColorGroup {
    #[serde(default)]
    pub query: String,
    pub color: GraphColor,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `graph.json`: the global graph's filters, groups, display and force
/// settings. `collapse-*` keys and anything newer stay in `extra`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default, rename = "showTags", skip_serializing_if = "Option::is_none")]
    pub show_tags: Option<bool>,
    #[serde(
        default,
        rename = "showAttachments",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_attachments: Option<bool>,
    #[serde(
        default,
        rename = "hideUnresolved",
        skip_serializing_if = "Option::is_none"
    )]
    pub hide_unresolved: Option<bool>,
    #[serde(
        default,
        rename = "showOrphans",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_orphans: Option<bool>,
    #[serde(default, rename = "colorGroups", skip_serializing_if = "Vec::is_empty")]
    pub color_groups: Vec<GraphColorGroup>,
    #[serde(default, rename = "showArrow", skip_serializing_if = "Option::is_none")]
    pub show_arrow: Option<bool>,
    #[serde(
        default,
        rename = "textFadeMultiplier",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_fade_multiplier: Option<f32>,
    #[serde(
        default,
        rename = "nodeSizeMultiplier",
        skip_serializing_if = "Option::is_none"
    )]
    pub node_size_multiplier: Option<f32>,
    #[serde(
        default,
        rename = "lineSizeMultiplier",
        skip_serializing_if = "Option::is_none"
    )]
    pub line_size_multiplier: Option<f32>,
    #[serde(
        default,
        rename = "centerStrength",
        skip_serializing_if = "Option::is_none"
    )]
    pub center_strength: Option<f32>,
    #[serde(
        default,
        rename = "repelStrength",
        skip_serializing_if = "Option::is_none"
    )]
    pub repel_strength: Option<f32>,
    #[serde(
        default,
        rename = "linkStrength",
        skip_serializing_if = "Option::is_none"
    )]
    pub link_strength: Option<f32>,
    #[serde(
        default,
        rename = "linkDistance",
        skip_serializing_if = "Option::is_none"
    )]
    pub link_distance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `command-palette.json`: the ids pinned to the top of the command palette.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CommandPaletteConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `switcher.json`: the quick switcher's options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SwitcherConfig {
    #[serde(
        default,
        rename = "showExistingOnly",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_existing_only: Option<bool>,
    #[serde(
        default,
        rename = "showAttachments",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_attachments: Option<bool>,
    #[serde(
        default,
        rename = "showAllFileTypes",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_all_file_types: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `templates.json`: the Templates core plugin's settings.
///
/// The file does not exist until a setting is written, and a cleared field
/// is written as the empty string rather than dropped. An empty format
/// means Obsidian's own default, not an empty rendering — see
/// [[Specification#Observed templates behavior, Obsidian 1.14.4]].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplatesConfig {
    /// Vault-relative folder holding the templates, verbatim as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// A moment format; `{{date}}` with no format of its own.
    #[serde(
        default,
        rename = "dateFormat",
        skip_serializing_if = "Option::is_none"
    )]
    pub date_format: Option<String>,
    /// A moment format; `{{time}}` with no format of its own.
    #[serde(
        default,
        rename = "timeFormat",
        skip_serializing_if = "Option::is_none"
    )]
    pub time_format: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `daily-notes.json`: the Daily notes core plugin's settings.
///
/// Like `templates.json`, the file appears only once a setting is written.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DailyNotesConfig {
    /// A moment format naming the note; empty or absent means
    /// [`DAILY_NOTE_DEFAULT_FORMAT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Vault-relative folder the notes are created in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// A template applied to a note the plugin creates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What a daily note is named when the format is empty or absent.
pub const DAILY_NOTE_DEFAULT_FORMAT: &str = "YYYY-MM-DD";

// The other core-plugin option files Obsidian 1.14.5 writes from its own
// options tabs, observed on 2026-10-30 (`tools/obsidian-probe/results/
// 2026-10-30-editor-menus.json`, step `s04-plugin-tabs`: every toggle
// clicked twice, every dropdown walked through its options and back, the
// `.obsidian` folder diffed after each click).
//
// Each is sparse and appears only once a control writes, each field is
// named exactly as Obsidian names it, and the fields are declared in the
// order Obsidian's own first write put them in. A row whose key the probe
// could not reach -- a file suggester, a slider, a textarea, a button --
// has no field here, and Scoria writes nothing for it rather than guessing
// a name; `scoria::settings::plugin_items` draws those rows tagged.

/// `backlink.json`: the Backlinks plugin's one option, off when absent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BacklinkConfig {
    /// "Show backlinks at the bottom of notes".
    #[serde(
        default,
        rename = "backlinkInDocument",
        skip_serializing_if = "Option::is_none"
    )]
    pub backlink_in_document: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `canvas.json`: the Canvas plugin's options.
///
/// Obsidian's first write held both snap keys at once
/// (`{"snapToObjects": true, "snapToGrid": false}`), with each dropdown's
/// key appended as it was picked. Two rows of the tab are not here: the
/// folder name the `folder` option reveals, and the zoom threshold slider,
/// neither of whose key the probe observed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasConfig {
    /// "Snap to objects", on when absent.
    #[serde(
        default,
        rename = "snapToObjects",
        skip_serializing_if = "Option::is_none"
    )]
    pub snap_to_objects: Option<bool>,
    /// "Snap to grid", on when absent.
    #[serde(
        default,
        rename = "snapToGrid",
        skip_serializing_if = "Option::is_none"
    )]
    pub snap_to_grid: Option<bool>,
    /// "Default location for new canvas files": `root`, `current`, `folder`.
    #[serde(
        default,
        rename = "newFileLocation",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_file_location: Option<String>,
    /// "Default mouse wheel behavior": `pan` or `zoom`.
    #[serde(
        default,
        rename = "defaultWheelBehavior",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_wheel_behavior: Option<String>,
    /// "Default Ctrl + Drag behavior": `menu`, `card`, `note`, `media`,
    /// `webpage` or `group`.
    #[serde(
        default,
        rename = "defaultModDragBehavior",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_mod_drag_behavior: Option<String>,
    /// "Show card names": `always`, `hover` or `never`.
    #[serde(
        default,
        rename = "cardLabelVisibility",
        skip_serializing_if = "Option::is_none"
    )]
    pub card_label_visibility: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `page-preview.json`: one key per surface the hover preview can be made
/// to require Ctrl on, each named after the surface rather than after the
/// row. Every surface is on when absent but Reading view, which is off.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PagePreviewConfig {
    /// "Search, Backlinks, and Outgoing links".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<bool>,
    /// "Reading view".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<bool>,
    /// "Editing view".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<bool>,
    /// "Tab header".
    #[serde(
        default,
        rename = "tab-header",
        skip_serializing_if = "Option::is_none"
    )]
    pub tab_header: Option<bool>,
    /// "Files".
    #[serde(
        default,
        rename = "file-explorer",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_explorer: Option<bool>,
    /// "Properties view".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<bool>,
    /// "Bookmarks".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmarks: Option<bool>,
    /// "Outline".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    /// "Bases".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bases: Option<bool>,
    /// "Graph view".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `note-composer.json`: the Note composer plugin's options. Obsidian 1.15.0
/// writes the file only once something is set, its keys in first-write
/// order, and the plugin's in-memory default is `{"askBeforeMerging": true}`
/// -- which is why its first write carries that key too (2026-11-09 probe).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteComposerConfig {
    /// "Confirm file merge", on when absent.
    #[serde(
        default,
        rename = "askBeforeMerging",
        skip_serializing_if = "Option::is_none"
    )]
    pub ask_before_merging: Option<bool>,
    /// "Text after extraction": `link`, `embed` or `none`.
    #[serde(
        default,
        rename = "replacementText",
        skip_serializing_if = "Option::is_none"
    )]
    pub replacement_text: Option<String>,
    /// "Template file location": the template's vault path, `.md` and all
    /// (`"Templates/Composer.md"`); clearing the row writes `""` rather than
    /// dropping the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl NoteComposerConfig {
    /// The config as the plugin saves it: its in-memory default,
    /// `askBeforeMerging: true`, is written with the first change when the
    /// file did not say.
    pub fn with_defaults(mut self) -> NoteComposerConfig {
        self.ask_before_merging.get_or_insert(true);
        self
    }
}

/// `zk-prefixer.json`: the Unique note creator plugin's options.
///
/// Observed on Obsidian 1.15.0 on 2026-11-09 (`tools/obsidian-probe/results/
/// 2026-11-09-note-composer-unique.json`, run `settings:s01-tab-zk`): absent
/// until a row writes, the plugin's default `{}`, the three keys in the order
/// they were first written, and a cleared field writes `""` rather than
/// dropping its key. An empty `format` names notes by
/// [`ZK_PREFIXER_DEFAULT_FORMAT`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ZkPrefixerConfig {
    /// "New file location": the folder new unique notes go in, which has to
    /// exist; empty or absent is the vault root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// "Template file location": the template's vault path, with `.md`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// "Unique prefix format": a moment format naming the note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What a unique note is named by when `format` is empty or absent: the
/// format field's placeholder, and what an emptied field renders its sample
/// from.
pub const ZK_PREFIXER_DEFAULT_FORMAT: &str = "YYYYMMDDHHmm";

/// `file-recovery.json`: the File recovery plugin's options.
///
/// Observed on Obsidian 1.15.0 on 2026-11-09 (`tools/obsidian-probe/results/
/// 2026-11-09-file-recovery.json`, step `s05-settings-typed`): absent until
/// a field writes; then the plugin's whole options object, both keys in this
/// order whichever field changed, as JavaScript numbers (`10`, `2.5`), two
/// spaces, no trailing newline. The snapshots themselves are not here: they
/// live in Obsidian's IndexedDB, outside the vault.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileRecoveryConfig {
    /// "Snapshot interval": the least minutes between two snapshots of a
    /// file; [`FILE_RECOVERY_DEFAULT_INTERVAL`] when absent.
    #[serde(
        default,
        rename = "intervalMinutes",
        skip_serializing_if = "Option::is_none"
    )]
    pub interval_minutes: Option<serde_json::Number>,
    /// "History length": the days a snapshot is kept;
    /// [`FILE_RECOVERY_DEFAULT_KEEP_DAYS`] when absent.
    #[serde(default, rename = "keepDays", skip_serializing_if = "Option::is_none")]
    pub keep_days: Option<serde_json::Number>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The plugin's "Snapshot interval" before anything is written.
pub const FILE_RECOVERY_DEFAULT_INTERVAL: i64 = 5;
/// The plugin's "History length" before anything is written.
pub const FILE_RECOVERY_DEFAULT_KEEP_DAYS: i64 = 7;

/// A number as JavaScript writes it into JSON: an integral value without a
/// fraction (`10`, not `10.0`), anything else as the float it is. `None`
/// for a value JSON cannot hold.
pub fn js_number(value: f64) -> Option<serde_json::Number> {
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        Some(serde_json::Number::from(value as i64))
    } else {
        serde_json::Number::from_f64(value)
    }
}

/// `webviewer.json`: the Web viewer plugin's options.
///
/// Obsidian's first write held both toggle keys at once; picking a search
/// engine appended `searchEngine`, and picking `custom` also wrote
/// `customSearchEngine` with the DuckDuckGo URL already in the field. The
/// homepage, the saved-page folder, the ad-blocking rules textarea and the
/// update-frequency slider had no observed key.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WebviewerConfig {
    /// "Open external links", on when absent.
    #[serde(
        default,
        rename = "openExternalURLs",
        skip_serializing_if = "Option::is_none"
    )]
    pub open_external_urls: Option<bool>,
    /// "Enable ad blocker", on when absent.
    #[serde(
        default,
        rename = "enableAdblocking",
        skip_serializing_if = "Option::is_none"
    )]
    pub enable_adblocking: Option<bool>,
    /// "Search engine": `duckduckgo`, `brave`, `bing`, `google` or `custom`.
    #[serde(
        default,
        rename = "searchEngine",
        skip_serializing_if = "Option::is_none"
    )]
    pub search_engine: Option<String>,
    /// "Custom search engine": a URL with `%s` where the query goes.
    #[serde(
        default,
        rename = "customSearchEngine",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_search_engine: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What the Web viewer tab's "Custom search engine" row shows, and what
/// picking `custom` wrote with the field untouched.
pub const WEBVIEWER_DEFAULT_SEARCH_URL: &str = "https://duckduckgo.com/?q=%s";

/// `plugins/<id>/manifest.json` of an installed community plugin.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default, rename = "authorUrl", skip_serializing_if = "Option::is_none")]
    pub author_url: Option<String>,
    #[serde(
        default,
        rename = "isDesktopOnly",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_desktop_only: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `themes/<name>/manifest.json` of an installed CSS theme.
///
/// Obsidian lists a theme folder only when this file's `name` is the
/// folder's own name, so the one string identifies the theme, names the
/// folder and is what `cssTheme` holds (probed 2026-11-02).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThemeManifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default, rename = "authorUrl", skip_serializing_if = "Option::is_none")]
    pub author_url: Option<String>,
    #[serde(
        default,
        rename = "fundingUrl",
        skip_serializing_if = "Option::is_none"
    )]
    pub funding_url: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A core plugin as the Core plugins tab lists it (names and descriptions
/// observed in Obsidian 1.14.4) and whether a fresh vault has it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorePlugin {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default_on: bool,
}

/// The core plugins in the order the Core plugins tab shows them.
pub const CORE_PLUGINS: &[CorePlugin] = &[
    CorePlugin { id: "audio-recorder", name: "Audio recorder", description: "Record audio notes and save them as attachments.", default_on: false },
    CorePlugin { id: "backlink", name: "Backlinks", description: "Show links from other files to the current file. Backlinks can be shown in a separate view or at the bottom of the current file.", default_on: true },
    CorePlugin { id: "bases", name: "Bases", description: "Create custom views that let you edit, sort, and filter files using their properties.", default_on: true },
    CorePlugin { id: "bookmarks", name: "Bookmarks", description: "Save shortcuts to files, searches, headings, and graphs.", default_on: true },
    CorePlugin { id: "canvas", name: "Canvas", description: "Arrange and connect notes on an infinite canvas.", default_on: true },
    CorePlugin { id: "command-palette", name: "Command palette", description: "Use Cmd/Ctrl+P and begin typing to invoke a command.", default_on: true },
    CorePlugin { id: "daily-notes", name: "Daily notes", description: "Create or open today's daily note.", default_on: true },
    CorePlugin { id: "file-recovery", name: "File recovery", description: "Restore recent snapshots to recover from accidental data loss. Snapshots are only saved for Markdown files.", default_on: true },
    CorePlugin { id: "file-explorer", name: "Files", description: "Browse the files and folders in your vault.", default_on: true },
    CorePlugin { id: "footnotes", name: "Footnotes view", description: "Show a list of footnotes from the current note.", default_on: false },
    CorePlugin { id: "graph", name: "Graph view", description: "Visualize the relationships between your notes.", default_on: true },
    CorePlugin { id: "note-composer", name: "Note composer", description: "Merge two notes or split one into two.", default_on: true },
    CorePlugin { id: "outgoing-link", name: "Outgoing links", description: "Show outgoing links and detect unlinked mentions of other notes in the current note.", default_on: true },
    CorePlugin { id: "outline", name: "Outline", description: "Show the table of contents for the current note.", default_on: true },
    CorePlugin { id: "page-preview", name: "Page preview", description: "Hover an internal link to preview its content. In editor mode, press Ctrl/Cmd while hovering.", default_on: true },
    CorePlugin { id: "properties", name: "Properties view", description: "Show the metadata for your files in the sidebar.", default_on: true },
    CorePlugin { id: "publish", name: "Publish", description: "Host your notes online as a website, wiki or documentation.", default_on: false },
    CorePlugin { id: "switcher", name: "Quick switcher", description: "Jump to other files with your keyboard.", default_on: true },
    CorePlugin { id: "random-note", name: "Random note", description: "Open a random note to rediscover or review.", default_on: false },
    CorePlugin { id: "global-search", name: "Search", description: "Search for a keyword in all the notes.", default_on: true },
    CorePlugin { id: "slash-command", name: "Slash commands", description: "Trigger commands in the editor by using the forward slash key.", default_on: false },
    CorePlugin { id: "slides", name: "Slides", description: "Create a presentation by using \u{201c}---\u{201d} to separate slides.", default_on: false },
    CorePlugin { id: "sync", name: "Sync", description: "Synchronize your files through Obsidian Sync.", default_on: true },
    CorePlugin { id: "tag-pane", name: "Tags view", description: "Show a list of all tags and their number of occurrences.", default_on: true },
    CorePlugin { id: "templates", name: "Templates", description: "Insert template content from a folder of template files.", default_on: true },
    CorePlugin { id: "zk-prefixer", name: "Unique note creator", description: "Create notes with unique timestamp prefixes, for workflows like zettelkasten or slip box.", default_on: false },
    CorePlugin { id: "webviewer", name: "Web viewer", description: "Open external links to web pages inside Obsidian.", default_on: false },
    CorePlugin { id: "word-count", name: "Word count", description: "Show word count in the status bar.", default_on: true },
    CorePlugin { id: "workspaces", name: "Workspaces", description: "Save and load workspace layouts.", default_on: false },
];

impl CorePlugins {
    /// Enabled state from the file, or the fresh-vault default when the
    /// file does not mention the plugin (list form: only listed ids are on).
    pub fn is_enabled_or_default(this: Option<&CorePlugins>, plugin: &CorePlugin) -> bool {
        match this {
            Some(CorePlugins::Map(m)) => m.get(plugin.id).copied().unwrap_or(plugin.default_on),
            Some(CorePlugins::List(l)) => l.iter().any(|x| x == plugin.id),
            None => plugin.default_on,
        }
    }
}

/// `types.json`: property name to type.
///
/// The map keeps insertion order because Obsidian writes the file that way,
/// seeded with `aliases`, `cssclasses` and `tags` before any key the user
/// assigns; a sorted map would rewrite the whole file on the first save.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PropertyTypes {
    #[serde(default)]
    pub types: IndexMap<String, String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl PropertyTypes {
    /// The entries Obsidian adds to `types.json` whenever it writes the file
    /// and finds them missing: the built-in keys, each pinned to its reserved
    /// type. Observed on the gate vault, Obsidian 1.15.0, 2026-11-09: a file
    /// Scoria had written with one key came back as that key, then these
    /// three, then the keys the session assigned -- so they are appended
    /// after whatever the file holds, in this order, and before the new key.
    pub const DEFAULTS: [(&'static str, &'static str); 3] = [
        ("aliases", "aliases"),
        ("cssclasses", "multitext"),
        ("tags", "tags"),
    ];

    /// Appends the missing defaults. Every writer of the file calls this
    /// first, so a new `types.json` opens with the three and an old one
    /// gains them where Obsidian would put them.
    pub fn seed_defaults(&mut self) {
        for (key, ty) in Self::DEFAULTS {
            if !self.types.contains_key(key) {
                self.types.insert(key.to_string(), ty.to_string());
            }
        }
    }
}

/// The type names Obsidian 1.14.4 registers (`registeredTypeWidgets`), which
/// are the values `types.json` carries. `text`, `multitext`, `number`,
/// `checkbox`, `date`, `datetime`, `tags` and `aliases` are the property
/// types; `file`, `folder` and `property` serve Bases views and the
/// properties type menu does not offer them. There is no `cssclasses` type:
/// that key infers as `multitext`.
pub const PROPERTY_TYPES: &[&str] = &[
    "text",
    "multitext",
    "number",
    "checkbox",
    "date",
    "datetime",
    "tags",
    "aliases",
    "file",
    "folder",
    "property",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_export_settings_keep_obsidians_order_a_string_margin_and_unknown_keys() {
        let text = r#"{"vimMode":true,"pdfExportSettings":{"includeName":true,"pageSize":"A4","landscape":false,"margin":"2","downscalePercent":80,"future":1}}"#;
        let app: AppConfig = serde_json::from_str(text).unwrap();
        let pdf = app.pdf_export_settings.clone().unwrap();
        assert_eq!(pdf.page_size.as_deref(), Some("A4"));
        assert_eq!(pdf.margin.as_deref(), Some("2"));
        assert_eq!(pdf.downscale_percent, Some(80));
        assert_eq!(serde_json::to_string(&app).unwrap(), text);
        // The first export from `{}` writes four keys, `includeName` not
        // among them.
        let first = PdfExportSettings {
            page_size: Some("Letter".into()),
            landscape: Some(false),
            margin: Some("0".into()),
            downscale_percent: Some(100),
            ..PdfExportSettings::default()
        };
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            r#"{"pageSize":"Letter","landscape":false,"margin":"0","downscalePercent":100}"#
        );
    }

    #[test]
    fn graph_json_round_trips_groups_and_collapse_keys() {
        let text = r#"{"collapse-filter":true,"showOrphans":true,"colorGroups":[{"query":"tag:#x","color":{"a":1,"rgb":16711680}}],"repelStrength":10,"scale":1,"close":true}"#;
        let cfg: GraphConfig = serde_json::from_str(text).unwrap();
        assert_eq!(cfg.show_orphans, Some(true));
        assert_eq!(cfg.color_groups[0].query, "tag:#x");
        assert_eq!(cfg.color_groups[0].color.rgb, 0xff0000);
        assert_eq!(cfg.extra["collapse-filter"], serde_json::json!(true));
        let back: serde_json::Value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(back["collapse-filter"], serde_json::json!(true));
        assert_eq!(
            back["colorGroups"][0]["color"]["rgb"],
            serde_json::json!(16711680)
        );
    }

    #[test]
    fn app_json_keeps_unknown_keys() {
        let text = r#"{"readableLineLength":false,"vimMode":true,"someFutureKey":[1,2]}"#;
        let cfg: AppConfig = serde_json::from_str(text).unwrap();
        assert_eq!(cfg.readable_line_length, Some(false));
        assert_eq!(cfg.vim_mode, Some(true));
        assert_eq!(cfg.extra["someFutureKey"], serde_json::json!([1, 2]));
        let back: Value = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(text).unwrap());
    }

    #[test]
    fn core_plugins_accepts_both_shapes() {
        let map: CorePlugins =
            serde_json::from_str(r#"{"file-explorer":true,"graph":false}"#).unwrap();
        assert!(map.is_enabled("file-explorer"));
        assert!(!map.is_enabled("graph"));
        let list: CorePlugins = serde_json::from_str(r#"["file-explorer","graph"]"#).unwrap();
        assert!(list.is_enabled("graph"));
        assert!(!list.is_enabled("canvas"));
    }

    #[test]
    fn command_palette_and_switcher_json() {
        let p: CommandPaletteConfig =
            serde_json::from_str(r#"{"pinned":["app:go-back","switcher:open"],"future":1}"#)
                .unwrap();
        assert_eq!(p.pinned, vec!["app:go-back", "switcher:open"]);
        assert_eq!(p.extra["future"], serde_json::json!(1));
        let s: SwitcherConfig = serde_json::from_str(r#"{"showAttachments":false}"#).unwrap();
        assert_eq!(s.show_attachments, Some(false));
        assert_eq!(s.show_existing_only, None);
        let plugin = CORE_PLUGINS.iter().find(|p| p.id == "graph").unwrap();
        assert!(CorePlugins::is_enabled_or_default(None, plugin));
        let off: CorePlugins = serde_json::from_str(r#"{"graph":false}"#).unwrap();
        assert!(!CorePlugins::is_enabled_or_default(Some(&off), plugin));
    }

    /// `zk-prefixer.json` as Obsidian 1.15.0 wrote it after its tab's three
    /// rows were filled in, byte for byte, and after all three were
    /// cleared: the keys stay, empty.
    #[test]
    fn zk_prefixer_json_round_trips_the_bytes_obsidian_wrote() {
        for bytes in [
            "{\n  \"folder\": \"Zettel\",\n  \"template\": \"Templates/Unique.md\",\n  \"format\": \"YYYY-MM-DD-HHmmss\"\n}",
            "{\n  \"folder\": \"\",\n  \"template\": \"\",\n  \"format\": \"\"\n}",
        ] {
            let cfg: ZkPrefixerConfig = serde_json::from_str(bytes).unwrap();
            assert!(cfg.extra.is_empty(), "every key is modelled");
            assert_eq!(serde_json::to_string_pretty(&cfg).unwrap(), bytes);
        }
        let cfg: ZkPrefixerConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg, ZkPrefixerConfig::default());
        assert_eq!(serde_json::to_string(&cfg).unwrap(), "{}");
    }

    /// `file-recovery.json` as Obsidian 1.15.0 wrote it while its two fields
    /// were typed into, byte for byte: both keys every time, numbers as
    /// JavaScript writes them.
    #[test]
    fn file_recovery_json_round_trips_the_bytes_obsidian_wrote() {
        for bytes in [
            "{\n  \"intervalMinutes\": 10,\n  \"keepDays\": 7\n}",
            "{\n  \"intervalMinutes\": 0,\n  \"keepDays\": 30\n}",
            "{\n  \"intervalMinutes\": 2.5,\n  \"keepDays\": 30\n}",
            "{\n  \"intervalMinutes\": 5,\n  \"keepDays\": 7\n}",
        ] {
            let cfg: FileRecoveryConfig = serde_json::from_str(bytes).unwrap();
            assert!(cfg.extra.is_empty(), "every key is modelled");
            assert_eq!(serde_json::to_string_pretty(&cfg).unwrap(), bytes);
        }
        let cfg = FileRecoveryConfig {
            interval_minutes: js_number(10.0),
            keep_days: js_number(FILE_RECOVERY_DEFAULT_KEEP_DAYS as f64),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string_pretty(&cfg).unwrap(),
            "{\n  \"intervalMinutes\": 10,\n  \"keepDays\": 7\n}"
        );
        assert_eq!(js_number(2.5).unwrap().to_string(), "2.5");
        assert_eq!(js_number(0.0).unwrap().to_string(), "0");
        assert_eq!(js_number(f64::NAN), None);
    }

    /// Every core-plugin options file: the keys Obsidian 1.14.5 writes read
    /// into their own fields, an unknown key comes back where it was, and
    /// the bytes round trip exactly. The files the probe saw are the input,
    /// each with one key Obsidian has never written appended.
    #[test]
    fn core_plugin_option_files_round_trip_with_an_unknown_key() {
        macro_rules! round_trip {
            ($ty:ty, $text:literal) => {{
                let cfg: $ty = serde_json::from_str($text).unwrap();
                assert_eq!(
                    cfg.extra["someFutureKey"],
                    serde_json::json!({ "nested": [1, 2] }),
                    "{} dropped the unknown key",
                    stringify!($ty)
                );
                let back: Value =
                    serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
                assert_eq!(
                    back,
                    serde_json::from_str::<Value>($text).unwrap(),
                    "{} did not round trip",
                    stringify!($ty)
                );
                cfg
            }};
        }

        let backlink = round_trip!(
            BacklinkConfig,
            r#"{"backlinkInDocument":true,"someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(backlink.backlink_in_document, Some(true));

        let canvas = round_trip!(
            CanvasConfig,
            r#"{"snapToObjects":true,"snapToGrid":false,"newFileLocation":"folder","defaultWheelBehavior":"zoom","defaultModDragBehavior":"group","cardLabelVisibility":"never","someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(canvas.snap_to_objects, Some(true));
        assert_eq!(canvas.snap_to_grid, Some(false));
        assert_eq!(canvas.new_file_location.as_deref(), Some("folder"));
        assert_eq!(canvas.default_wheel_behavior.as_deref(), Some("zoom"));
        assert_eq!(canvas.default_mod_drag_behavior.as_deref(), Some("group"));
        assert_eq!(canvas.card_label_visibility.as_deref(), Some("never"));

        let preview = round_trip!(
            PagePreviewConfig,
            r#"{"search":true,"preview":false,"editor":true,"tab-header":false,"file-explorer":true,"properties":false,"bookmarks":true,"outline":false,"bases":true,"graph":false,"someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(preview.search, Some(true));
        assert_eq!(preview.tab_header, Some(false));
        assert_eq!(preview.file_explorer, Some(true));
        assert_eq!(preview.graph, Some(false));

        let composer = round_trip!(
            NoteComposerConfig,
            r#"{"askBeforeMerging":false,"replacementText":"embed","template":"Templates/Composer.md","someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(composer.ask_before_merging, Some(false));
        assert_eq!(composer.replacement_text.as_deref(), Some("embed"));
        assert_eq!(composer.template.as_deref(), Some("Templates/Composer.md"));

        let zk = round_trip!(
            ZkPrefixerConfig,
            r#"{"folder":"Zettels","template":"T.md","format":"YYYYMMDDHHmm","someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(zk.folder.as_deref(), Some("Zettels"));
        assert_eq!(zk.template.as_deref(), Some("T.md"));
        assert_eq!(zk.format.as_deref(), Some("YYYYMMDDHHmm"));
        let recovery = round_trip!(
            FileRecoveryConfig,
            r#"{"intervalMinutes":2.5,"keepDays":30,"someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(recovery.interval_minutes, js_number(2.5));
        assert_eq!(recovery.keep_days, js_number(30.0));

        let web = round_trip!(
            WebviewerConfig,
            r#"{"openExternalURLs":false,"enableAdblocking":false,"searchEngine":"custom","customSearchEngine":"https://duckduckgo.com/?q=%s","someFutureKey":{"nested":[1,2]}}"#
        );
        assert_eq!(web.open_external_urls, Some(false));
        assert_eq!(web.enable_adblocking, Some(false));
        assert_eq!(web.search_engine.as_deref(), Some("custom"));
        assert_eq!(
            web.custom_search_engine.as_deref(),
            Some(WEBVIEWER_DEFAULT_SEARCH_URL)
        );

        // A file none of them has ever been written to is all defaults and
        // serializes to nothing at all, the way the sparse files do.
        assert_eq!(
            serde_json::to_string(&CanvasConfig::default()).unwrap(),
            "{}"
        );
        assert_eq!(
            serde_json::to_string(&PagePreviewConfig::default()).unwrap(),
            "{}"
        );
    }

    #[test]
    fn hotkeys_shape() {
        let text = r#"{"editor:toggle-bold":[{"modifiers":["Mod"],"key":"B"}],"app:reload":[]}"#;
        let hk: Hotkeys = serde_json::from_str(text).unwrap();
        assert_eq!(hk["editor:toggle-bold"][0].key, "B");
        assert!(hk["app:reload"].is_empty());
    }

    #[test]
    fn types_json_is_seeded_the_way_obsidian_seeds_it() {
        // The bytes Obsidian 1.15.0 left on the gate vault (2026-11-09) after
        // a session that found `status` alone in the file and assigned two
        // more keys: the one it found, the three defaults, the new ones.
        let observed = "{\n  \"types\": {\n    \"status\": \"date\",\n    \"aliases\": \"aliases\",\n    \"cssclasses\": \"multitext\",\n    \"tags\": \"tags\",\n    \"draft\": \"checkbox\",\n    \"scoria\": \"multitext\"\n  }\n}";
        let mut types = PropertyTypes::default();
        types.types.insert("status".into(), "date".into());
        types.seed_defaults();
        types.types.insert("draft".into(), "checkbox".into());
        types.seed_defaults();
        types.types.insert("scoria".into(), "multitext".into());
        assert_eq!(serde_json::to_string_pretty(&types).unwrap(), observed);
        // A fresh file opens with the three.
        let mut fresh = PropertyTypes::default();
        fresh.seed_defaults();
        fresh.types.insert("status".into(), "multitext".into());
        assert_eq!(
            fresh.types.keys().collect::<Vec<_>>(),
            ["aliases", "cssclasses", "tags", "status"]
        );
        // Seeding twice adds nothing.
        let again = fresh.clone();
        fresh.seed_defaults();
        assert_eq!(fresh, again);
    }
}
