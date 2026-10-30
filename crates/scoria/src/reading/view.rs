//! GPUI rendering of a reading-view [`Model`].
//!
//! Top-level blocks go through a lazily measured `list`, so a long note only
//! lays out the blocks on screen. Inline content is one `InteractiveText`
//! per run of text, with the styled ranges from the model and clickable
//! ranges for links, tags and footnote references.

use super::model::{
    Embed, EmbedKind, InlineStyle, LinkTarget, Model, RBlock, RItem, RichText, Segment, TaskMarker,
};
use crate::assets::{callout_icon, icon};
use crate::fold::{self, FoldKey};
use crate::math::{self, FormulaElement, Inline, Overlay, SpaceWidths};
use crate::properties::{Field, Part, Target};
use crate::properties::{PROPERTIES_FOLD_ID, PROPERTY_ID_BASE};
use crate::theme::Theme;
use gpui::{
    actions, canvas, div, img, list, prelude::*, px, AnyElement, App, ClipboardItem, Context,
    ElementId, EventEmitter, FocusHandle, Focusable, Font, FontStyle, FontWeight, HighlightStyle,
    Hsla, InteractiveText, KeyBinding, ListAlignment, ListOffset, ListState, MouseButton,
    ObjectFit, Pixels, SharedString, StrikethroughStyle, StyledText, Task, TextRun, UnderlineStyle,
    Window, WindowTextSystem,
};
use scoria_formats::config::PropertyTypes;
use scoria_formats::properties::{PropertiesInDocument, Property, PropertyType};
use scoria_formats::FileKind;
use scoria_markdown::Alignment;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

actions!(
    reading,
    [
        ScrollPageDown,
        ScrollPageUp,
        ScrollLineDown,
        ScrollLineUp,
        ScrollToTop,
        ScrollToBottom,
        SelectAll,
        Copy,
    ]
);

/// Binds the reading view's scrolling and selection keys in its own context;
/// call once at startup. Obsidian's reading view is a DOM scroller with no
/// commands of its own, so this is the browser's set of page-scrolling keys
/// rather than a probed Obsidian binding: the page keys and Space for a page,
/// the arrows for a line, Home and End (plain and with Ctrl, which a document
/// scroller treats alike) for the ends, and the selection pair every document
/// has — Ctrl+A for the whole note, Ctrl+C for what is selected.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some("ReadingView");
    cx.bind_keys([
        KeyBinding::new("pagedown", ScrollPageDown, ctx),
        KeyBinding::new("pageup", ScrollPageUp, ctx),
        KeyBinding::new("space", ScrollPageDown, ctx),
        KeyBinding::new("shift-space", ScrollPageUp, ctx),
        KeyBinding::new("down", ScrollLineDown, ctx),
        KeyBinding::new("up", ScrollLineUp, ctx),
        KeyBinding::new("home", ScrollToTop, ctx),
        KeyBinding::new("ctrl-home", ScrollToTop, ctx),
        KeyBinding::new("end", ScrollToBottom, ctx),
        KeyBinding::new("ctrl-end", ScrollToBottom, ctx),
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
    ]);
}

/// What `render` multiplies the base font size by for the line height, and so
/// how tall the line an arrow key scrolls is.
const LINE_HEIGHT: f32 = 1.6;

/// Pixels a page key scrolls a viewport this tall: the viewport less an eighth
/// of it capped at 40 px, which is what a browser's page keys do, so a strip of
/// the page just read stays on screen. A viewport of zero is a view that has
/// not been painted yet, which has nothing to scroll.
pub(crate) fn page_distance(viewport: f32) -> f32 {
    if viewport <= 0. {
        return 0.;
    }
    viewport - (viewport / 8.).min(40.)
}

/// Pixels an arrow key scrolls: one rendered line.
pub(crate) fn line_distance(base_font_size: f32) -> f32 {
    base_font_size * LINE_HEIGHT
}

/// Whether a [`ReadingView::set_note`] is a reload of the note already on
/// screen, which keeps the reader's place and their folds. Moving to another
/// note, or to no note at all, drops both.
pub(crate) fn same_note(current: Option<&str>, next: Option<&str>) -> bool {
    next.is_some() && current == next
}

/// The space above the first block in both views: the pane body's top
/// padding, with the inline title inside it when one is shown.
pub(crate) const HEADER_TOP: f32 = 24.;

/// The inline title a note shows above its first block: the basename
/// without its extension. Only a markdown note has one; the image a
/// synthetic model embeds, or a file no view can draw, shows none.
pub(crate) fn inline_title(path: &str) -> Option<&str> {
    if !FileKind::from_path(path).is_markdown() {
        return None;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    Some(name.rsplit_once('.').map_or(name, |(stem, _)| stem))
}

/// The title the header row shows: none unless `showInlineTitle` is on and
/// a note is open.
pub(crate) fn shown_title(show: bool, path: Option<&str>) -> Option<&str> {
    if !show {
        return None;
    }
    path.and_then(inline_title)
}

/// Height of the header row above the first block, in pixels, for a body
/// font of `base` pixels: the top padding, plus the title when there is
/// one. Obsidian's `.inline-title` is an h1 — 1.618em at weight 700 on a
/// 1.2 line, 25.888 px / 31.07 px at the 16 px default — with a 0.5em
/// margin below it, the 12.944 px measured. The editor reserves the same
/// height above its first line, so a mode switch does not move the text.
pub(crate) fn header_height(base: f32, title: bool) -> f32 {
    let (size, _, line_height) = crate::theme::heading_style(1);
    let h1 = base * size;
    HEADER_TOP
        + if title {
            h1 * line_height + h1 * 0.5
        } else {
            0.
        }
}

// ----- selection ---------------------------------------------------------------

/// A place in the reading view's text: which top-level block it belongs to,
/// which [`RichText`] inside that block, and a byte offset into that run's
/// own string. Obsidian's reading view is a DOM document, so a position
/// there is a text node and an offset; this is the same triple over the
/// model the view paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadingPos {
    /// Index into [`Model::blocks`].
    pub block: usize,
    /// [`RichText::id`], unique within one model.
    pub rich: usize,
    /// Byte offset into that run's text.
    pub offset: usize,
}

/// One [`RichText`] of a note, in document order: what a selection can
/// cover, and what copying it joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run<'a> {
    pub block: usize,
    pub rich: usize,
    pub text: &'a str,
    /// What separates this run from the one before it when a selection is
    /// joined into plain text: a newline for a new line of the document,
    /// or a tab between the cells of one table row, which is what copying
    /// a table out of a browser gives.
    pub before: &'static str,
}

/// Every [`RichText`] of `model`, in document order, with the top-level
/// block each belongs to. This is the order a selection is read in and the
/// order copying joins, so it walks the model the way the renderer draws
/// it: an embedded note's blocks where the embed sits, a callout's title
/// before its children, a table's header row before its body.
///
/// Blocks with no inline model carry nothing here and so cannot be
/// selected: a code block (which has a copy button of its own), raw HTML,
/// a display formula, and the properties block, whose rows are built by
/// the properties module rather than held in the model.
pub(crate) fn runs(model: &Model) -> Vec<Run<'_>> {
    let mut out = Vec::new();
    for (ix, block) in model.blocks.iter().enumerate() {
        collect_runs(std::slice::from_ref(block), ix, &mut out);
    }
    out
}

fn collect_runs<'a>(blocks: &'a [RBlock], block: usize, out: &mut Vec<Run<'a>>) {
    let push = |text: &'a RichText, before: &'static str, out: &mut Vec<Run<'a>>| {
        out.push(Run {
            block,
            rich: text.id,
            text: &text.text,
            before,
        });
    };
    for b in blocks {
        match b {
            // The renderer lays a paragraph's segments out in a column, so
            // every text segment of one starts a line of its own.
            RBlock::Paragraph(segments) => {
                for segment in segments {
                    match segment {
                        Segment::Text(text) => push(text, "\n", out),
                        Segment::Embed(embed) => {
                            if let EmbedKind::Note { blocks, .. } = &embed.kind {
                                collect_runs(blocks, block, out);
                            }
                        }
                        Segment::Math(_) => {}
                    }
                }
            }
            RBlock::Heading { text, .. } => push(text, "\n", out),
            RBlock::Quote(children) => collect_runs(children, block, out),
            RBlock::Callout {
                title, children, ..
            } => {
                push(title, "\n", out);
                collect_runs(children, block, out);
            }
            RBlock::List { items, .. } => {
                for item in items {
                    collect_runs(&item.children, block, out);
                }
            }
            RBlock::Table { header, rows, .. } => {
                for row in std::iter::once(header).chain(rows.iter()) {
                    for (at, cell) in row.iter().enumerate() {
                        push(cell, if at == 0 { "\n" } else { "\t" }, out);
                    }
                }
            }
            RBlock::Footnotes(notes) => {
                for (_, body) in notes {
                    collect_runs(body, block, out);
                }
            }
            RBlock::Properties(_)
            | RBlock::ThematicBreak
            | RBlock::Code { .. }
            | RBlock::Html(_)
            | RBlock::Math(_) => {}
        }
    }
}

/// Where `pos` sits in `runs`, by block and run id.
fn run_index(runs: &[Run<'_>], pos: ReadingPos) -> Option<usize> {
    runs.iter()
        .position(|r| r.block == pos.block && r.rich == pos.rich)
}

/// The two ends of a selection in document order, lowest first. `None`
/// when either end names a run this model does not have, which is what an
/// anchor left over from another note looks like.
pub(crate) fn ordered(
    runs: &[Run<'_>],
    anchor: ReadingPos,
    head: ReadingPos,
) -> Option<(ReadingPos, ReadingPos)> {
    let a = run_index(runs, anchor)?;
    let b = run_index(runs, head)?;
    Some(if (a, anchor.offset) <= (b, head.offset) {
        (anchor, head)
    } else {
        (head, anchor)
    })
}

/// The byte range selected inside each run the selection covers, as indices
/// into `runs`, in document order. Runs with nothing of them selected are
/// left out, so an empty selection yields nothing at all.
pub(crate) fn selected_ranges(
    runs: &[Run<'_>],
    anchor: ReadingPos,
    head: ReadingPos,
) -> Vec<(usize, Range<usize>)> {
    let Some((start, end)) = ordered(runs, anchor, head) else {
        return Vec::new();
    };
    let (first, last) = match (run_index(runs, start), run_index(runs, end)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for (at, run) in runs.iter().enumerate().take(last + 1).skip(first) {
        let text = run.text;
        let from = if at == first {
            floor_boundary(text, start.offset)
        } else {
            0
        };
        let to = if at == last {
            floor_boundary(text, end.offset)
        } else {
            text.len()
        };
        if from < to {
            out.push((at, from..to));
        }
    }
    out
}

/// The plain text a selection yields: the selected part of each run in
/// document order, joined by the separation the reader sees — a newline
/// between the lines of the document, a tab between the cells of one table
/// row. Obsidian's reading view is DOM, so this is what a copy out of the
/// browser gives, without the markers the source carries.
pub(crate) fn selected_text(runs: &[Run<'_>], anchor: ReadingPos, head: ReadingPos) -> String {
    let mut out = String::new();
    for (n, (at, range)) in selected_ranges(runs, anchor, head).into_iter().enumerate() {
        if n > 0 {
            out.push_str(runs[at].before);
        }
        out.push_str(&runs[at].text[range]);
    }
    out
}

/// Anchor and head covering every run of the note, for Ctrl+A. `None` when
/// the note has no selectable text at all.
pub(crate) fn whole_note(runs: &[Run<'_>]) -> Option<(ReadingPos, ReadingPos)> {
    let first = runs.first()?;
    let last = runs.last()?;
    Some((
        ReadingPos {
            block: first.block,
            rich: first.rich,
            offset: 0,
        },
        ReadingPos {
            block: last.block,
            rich: last.rich,
            offset: last.text.len(),
        },
    ))
}

/// The word around `offset`, the range a double-click takes: a run of
/// alphanumerics, underscores and apostrophes, or the run of whitespace or
/// punctuation that is there instead, so a press always takes something.
pub(crate) fn word_at(text: &str, offset: usize) -> Range<usize> {
    let at = floor_boundary(text, offset);
    if text.is_empty() {
        return 0..0;
    }
    let wordish = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    // A press at the very end of the text takes the word before it.
    let probe = if at == text.len() {
        text[..at].chars().next_back()
    } else {
        text[at..].chars().next()
    };
    let word = probe.is_some_and(wordish);
    let like = |c: char| wordish(c) == word;
    let start = text[..at]
        .char_indices()
        .rev()
        .take_while(|(_, c)| like(*c))
        .map(|(i, _)| i)
        .last()
        .unwrap_or(at);
    let end = text[at..]
        .char_indices()
        .take_while(|(_, c)| like(*c))
        .map(|(i, c)| at + i + c.len_utf8())
        .last()
        .unwrap_or(at);
    start..end
}

/// The largest char boundary at or below `at`, so an offset that came from
/// a hit test against a shaped string can never slice a run apart.
fn floor_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// One end of a selection, for the control socket's state dump.
fn describe_pos(pos: ReadingPos) -> Value {
    json!({"block": pos.block, "rich": pos.rich, "offset": pos.offset})
}

/// What the reader did over a run of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectGesture {
    /// Left button down: the anchor and the head go here.
    Press,
    /// Moved with the left button down: the head follows.
    Drag,
    /// Double-click: the word under the pointer.
    Word,
    /// Triple-click: the whole run, which is Obsidian's paragraph.
    Line,
}

/// Told which run a gesture landed on and the byte offset in it.
pub type SelectHandler = Rc<dyn Fn(SelectGesture, usize, usize, &mut Window, &mut App)>;

/// Lets the text the renderer draws be selected. Only the reading view sets
/// one; Live Preview's widgets sit behind the editor's own selection, so
/// they pass none and the runs take no mouse of their own.
pub(crate) struct SelectHost {
    /// The selected byte range of each [`RichText`], by id, painted as a
    /// wash behind the text.
    pub ranges: Rc<HashMap<usize, Range<usize>>>,
    pub color: Hsla,
    pub on_select: SelectHandler,
}

/// The byte offset a point fell on in a laid-out run: the exact one when
/// the point is inside the text, and the nearest otherwise — past the end
/// of a wrapped line, or in the gap a short last line leaves.
fn index_at(layout: &gpui::TextLayout, position: gpui::Point<Pixels>) -> usize {
    match layout.index_for_position(position) {
        Ok(at) | Err(at) => at,
    }
}

/// The wash behind selected text: the accent at the alpha the editor's own
/// selection uses, so the two views look alike.
pub(crate) fn selection_color(theme: &Theme) -> Hsla {
    let mut color = theme.accent;
    color.a = if theme.dark { 0.35 } else { 0.25 };
    color
}

/// Merges a selection's wash into a text's style runs: the pieces the
/// selection covers gain its background, and any run it starts or ends
/// inside is split, so the list stays sorted and non-overlapping the way
/// `StyledText::with_highlights` needs it.
pub(crate) fn with_selection(
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    text: &str,
    selection: Range<usize>,
    color: Hsla,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let len = text.len();
    // The offsets came from a hit test, and over a paragraph with inline
    // formulas the string laid out is not the run's own, so both ends are
    // walked back to a boundary the string can be cut on.
    let selection = floor_boundary(text, selection.start)..floor_boundary(text, selection.end);
    if selection.is_empty() {
        return highlights;
    }
    let mut cuts: Vec<usize> = vec![0, len, selection.start, selection.end];
    for (range, _) in &highlights {
        cuts.push(range.start.min(len));
        cuts.push(range.end.min(len));
    }
    cuts.retain(|at| *at <= len);
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let base = highlights
            .iter()
            .find(|(range, _)| range.start <= from && to <= range.end)
            .map(|(_, style)| *style);
        let inside = selection.start <= from && to <= selection.end;
        let Some(mut style) = base.or(if inside {
            Some(HighlightStyle::default())
        } else {
            None
        }) else {
            continue;
        };
        if inside {
            style.background_color = Some(color);
        }
        out.push((from..to, style));
    }
    out
}

/// Something the reader did to a property row. Live Preview turns each into
/// a document edit; the reading view raises only [`PropertyAction::RemoveItem`],
/// which the workspace turns into the same edit.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyAction {
    /// Start typing in a row's key or value.
    Edit {
        target: Target,
        part: Part,
    },
    /// Start typing in one item of a list-shaped value. `index` past the
    /// last item is the empty input Obsidian keeps after the pills, where
    /// a new item is written.
    EditItem {
        key: String,
        index: usize,
    },
    /// A pill's own remove control: drop that item and leave the rest.
    RemoveItem {
        key: String,
        index: usize,
    },
    /// Flip a checkbox row.
    Toggle {
        key: String,
    },
    Remove {
        key: String,
    },
    Retype {
        key: String,
        ty: PropertyType,
    },
    /// Open an unnamed row at the end of the block.
    AddRow,
    /// Show the type menu for a row.
    OpenTypeMenu {
        key: String,
    },
}

/// Lets the properties block be edited. The reading view passes `None` and
/// draws the rows read-only, but for the pills' remove control, which
/// [`ReadingHost::on_remove_item`] takes; Live Preview passes the open field
/// and a sink for what the reader does.
pub(crate) struct PropertiesHost {
    /// The field being typed into, when it belongs to this block.
    pub field: Option<Field>,
    /// The pill a Backspace selected, by item index in `field`'s row: the
    /// first of the two presses that remove it moves focus there, and the
    /// pill is drawn focused until the second press takes it.
    pub selected_pill: Option<usize>,
    /// `propertiesInDocument`: `Hidden` draws nothing at all.
    pub mode: PropertiesInDocument,
    /// The key whose type menu is open, if any.
    pub menu: Option<String>,
    pub on_action: PropertyHandler,
}

/// What a property row's affordances do when clicked.
pub type PropertyHandler = Rc<dyn Fn(PropertyAction, &mut Window, &mut App)>;

/// Something the reader did to a table cell. The reading view raises none
/// of these; Live Preview turns each into a document edit.
#[derive(Debug, Clone, PartialEq)]
pub enum TableAction {
    /// Start typing in the cell standing for these bytes.
    Edit { span: Range<usize> },
}

/// What a table cell does when clicked.
pub type TableHandler = Rc<dyn Fn(TableAction, &mut Window, &mut App)>;

/// Lets a table's cells be edited. The reading view passes `None` and draws
/// the cells read-only; Live Preview passes the open cell and a sink for
/// what the reader does.
pub(crate) struct TableHost {
    /// The cell being typed into: the bytes it stands for and the text in
    /// hand, which has not reached the document yet.
    pub cell: Option<(Range<usize>, Field)>,
    pub on_action: TableHandler,
}

/// What a rendered link does when clicked.
pub type LinkHandler = Rc<dyn Fn(LinkTarget, &mut Window, &mut App)>;
/// What a foldable callout's title does when clicked, by title id.
pub type ToggleHandler = Rc<dyn Fn(usize, &mut App)>;
/// What a task checkbox does when clicked.
pub type TaskHandler = Rc<dyn Fn(TaskMarker, &mut App)>;
/// Told which code block's copy button was pressed, so its check can show.
pub type CopyHandler = Rc<dyn Fn(usize, &mut App)>;

/// The settings and affordances only the reading view has. Live Preview
/// passes `None` and keeps the click handling it already does on the
/// source line behind a widget.
pub(crate) struct ReadingHost {
    /// `propertiesInDocument`: `Hidden` draws no properties block.
    pub properties: PropertiesInDocument,
    /// The code block whose copy button is showing its check.
    pub copied: Option<usize>,
    pub on_copy: CopyHandler,
    pub on_task: TaskHandler,
    /// The one property affordance the reading view has: a pill's remove
    /// control, handed [`PropertyAction::RemoveItem`] and nothing else. The
    /// rest of the block stays read-only, with no [`PropertiesHost`] to
    /// open a field or a menu. `None` draws no control at all, for a host
    /// with no way to write the note (a canvas card).
    pub on_remove_item: Option<PropertyHandler>,
}

pub enum ReadingEvent {
    OpenLink(LinkTarget),
    /// A Ctrl+click on a link: it opens in a new tab.
    OpenLinkInNewTab(LinkTarget),
}

/// A reader clicked a task checkbox. The reading view does not write the
/// note itself: the workspace turns this into the same edit Live Preview
/// makes when the same box is clicked there.
pub struct TaskToggled {
    /// The note the reading view was showing, so an event that outlived
    /// its note writes nothing.
    pub path: String,
    /// Where the three bytes `[`, status, `]` are in it.
    pub marker: TaskMarker,
}

/// A reader clicked the remove control of a list property's pill. As with
/// [`TaskToggled`], the reading view writes nothing itself: the workspace
/// makes the edit the same pill makes in Live Preview.
pub struct PropertyItemRemoved {
    /// The note the reading view was showing, so an event that outlived
    /// its note writes nothing.
    pub path: String,
    /// The property the pill belongs to.
    pub key: String,
    /// The pill's index among the list's items.
    pub index: usize,
}

pub struct ReadingView {
    focus_handle: FocusHandle,
    pub path: Option<String>,
    model: Arc<Model>,
    list: ListState,
    /// Callouts whose fold state the reader flipped, by title id.
    toggled: HashSet<usize>,
    /// The note's heading and list folds, by key — the same keys the
    /// editor keeps, handed across on a mode switch. Only the heading ones
    /// collapse anything here; the list ones ride along so a switch to the
    /// editor and back loses nothing.
    folds: Vec<FoldKey>,
    /// `foldHeading`: with it off the indicator is not drawn.
    fold_headings: bool,
    /// Per top-level block: inside a collapsed heading's section.
    hidden: Vec<bool>,
    readable_line_length: bool,
    /// `showInlineTitle`: the note's name at h1 size above its first block.
    show_inline_title: bool,
    properties_in_document: PropertiesInDocument,
    /// The vault's `types.json`, for the type each property row shows.
    property_types: PropertyTypes,
    /// The code block whose copy button is showing its check, and the
    /// timer that takes it away again.
    copied: Option<usize>,
    flash: Option<Task<()>>,
    /// Width of the view as last painted; tables lay their columns out for it.
    width: Pixels,
    /// The reader's text selection: where the drag started and where it is
    /// now, each a block, a run and a byte offset. Dropped when the note
    /// changes.
    selection: Option<(ReadingPos, ReadingPos)>,
    /// Whether a press landed on text and the button has not come up yet,
    /// so a move extends the selection.
    dragging: bool,
    /// The selected byte range of each run, by [`RichText::id`]: derived
    /// from `selection` whenever it changes, because the painted blocks ask
    /// for it on every frame.
    selected: Rc<HashMap<usize, Range<usize>>>,
}

impl EventEmitter<ReadingEvent> for ReadingView {}
impl EventEmitter<TaskToggled> for ReadingView {}
impl EventEmitter<PropertyItemRemoved> for ReadingView {}

impl Focusable for ReadingView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ReadingView {
    pub fn new(cx: &mut Context<Self>) -> ReadingView {
        ReadingView {
            focus_handle: cx.focus_handle(),
            path: None,
            model: Arc::new(Model::default()),
            list: ListState::new(0, ListAlignment::Top, px(1200.)),
            toggled: HashSet::new(),
            folds: Vec::new(),
            fold_headings: true,
            hidden: Vec::new(),
            readable_line_length: true,
            show_inline_title: true,
            properties_in_document: PropertiesInDocument::default(),
            property_types: PropertyTypes::default(),
            copied: None,
            flash: None,
            width: px(900.),
            selection: None,
            dragging: false,
            selected: Rc::new(HashMap::new()),
        }
    }

    pub fn set_readable_line_length(&mut self, on: bool) {
        self.readable_line_length = on;
    }

    pub fn set_show_inline_title(&mut self, on: bool) {
        self.show_inline_title = on;
    }

    pub fn set_properties_in_document(&mut self, mode: PropertiesInDocument) {
        self.properties_in_document = mode;
    }

    /// The inline title over the blocks, if the setting and the file give
    /// one.
    pub fn inline_title(&self) -> Option<&str> {
        shown_title(self.show_inline_title, self.path.as_deref())
    }

    pub fn set_property_types(&mut self, types: PropertyTypes) {
        self.property_types = types;
    }

    pub fn set_note(&mut self, path: Option<String>, model: Model, cx: &mut Context<Self>) {
        let same = same_note(self.path.as_deref(), path.as_deref());
        let top = self.list.logical_scroll_top();
        self.path = path;
        self.list.reset(model.blocks.len());
        if same {
            // A reload after a change on disk keeps the reader's place.
            self.list.scroll_to(top);
        }
        self.model = Arc::new(model);
        if !same {
            // ... and the folds they opened: the watcher re-enters this on
            // every autosave, so clearing them unconditionally collapsed an
            // expanded callout 1.5 s after any save of the note being read.
            self.toggled.clear();
            self.folds.clear();
        }
        self.recompute_hidden();
        self.copied = None;
        // A selection names runs of the model that has just been replaced,
        // so it goes even on a reload: the ids a rebuild hands out are not
        // the ones the reader pressed on.
        self.selection = None;
        self.dragging = false;
        self.selected = Rc::new(HashMap::new());
        cx.notify();
    }

    // ----- selection ------------------------------------------------------------

    /// Stores a selection and derives the per-run ranges the blocks paint.
    fn set_selection(
        &mut self,
        selection: Option<(ReadingPos, ReadingPos)>,
        cx: &mut Context<Self>,
    ) {
        let model = self.model.clone();
        let runs = runs(&model);
        let mut ranges = HashMap::new();
        let selection = selection.filter(|(a, h)| ordered(&runs, *a, *h).is_some());
        if let Some((anchor, head)) = selection {
            for (at, range) in selected_ranges(&runs, anchor, head) {
                ranges.insert(runs[at].rich, range);
            }
        }
        self.selection = selection;
        self.selected = Rc::new(ranges);
        cx.notify();
    }

    /// What the reader did over a run of text.
    fn gesture(&mut self, gesture: SelectGesture, pos: ReadingPos, cx: &mut Context<Self>) {
        match gesture {
            SelectGesture::Press => {
                self.dragging = true;
                self.set_selection(Some((pos, pos)), cx);
            }
            SelectGesture::Drag => {
                // A drag fires for every move the pointer makes, and most
                // of them are still on the byte the last one landed on.
                if self.dragging {
                    if let Some((anchor, head)) = self.selection {
                        if head != pos {
                            self.set_selection(Some((anchor, pos)), cx);
                        }
                    }
                }
            }
            SelectGesture::Word | SelectGesture::Line => {
                let model = self.model.clone();
                let runs = runs(&model);
                let Some(at) = runs
                    .iter()
                    .position(|r| r.block == pos.block && r.rich == pos.rich)
                else {
                    return;
                };
                let range = if gesture == SelectGesture::Word {
                    word_at(runs[at].text, pos.offset)
                } else {
                    0..runs[at].text.len()
                };
                self.dragging = false;
                self.set_selection(
                    Some((
                        ReadingPos {
                            offset: range.start,
                            ..pos
                        },
                        ReadingPos {
                            offset: range.end,
                            ..pos
                        },
                    )),
                    cx,
                );
            }
        }
    }

    /// A press that landed on no text at all: the selection goes, the way a
    /// click on a browser's margin drops one.
    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_some() {
            self.set_selection(None, cx);
        }
        self.dragging = false;
    }

    /// The plain text of the selection, empty when there is none.
    pub fn selected_text(&self) -> String {
        let Some((anchor, head)) = self.selection else {
            return String::new();
        };
        selected_text(&runs(&self.model), anchor, head)
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let model = self.model.clone();
        let whole = whole_note(&runs(&model));
        self.dragging = false;
        self.set_selection(whole, cx);
    }

    /// Ctrl+C: the selection as plain text. With nothing selected the
    /// clipboard is left alone, as a copy out of a browser with no
    /// selection writes nothing.
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    // ----- heading folds --------------------------------------------------------

    /// Every top-level heading: its block index, level and key.
    fn headings(&self) -> Vec<(usize, u8, FoldKey)> {
        let heads: Vec<(usize, u8, &str)> = self
            .model
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(ix, b)| match b {
                RBlock::Heading { level, plain, .. } => Some((ix, *level, plain.as_str())),
                _ => None,
            })
            .collect();
        let keys = fold::heading_keys(heads.iter().map(|(_, l, t)| (*l, *t)));
        heads
            .into_iter()
            .zip(keys)
            .map(|((ix, level, _), key)| (ix, level, key))
            .collect()
    }

    /// Marks the blocks a collapsed heading hides: from the block after it
    /// to the one before the next heading of the same or a higher level —
    /// absent from the view, as Obsidian's are absent from the DOM.
    fn recompute_hidden(&mut self) {
        let heads = self.headings();
        let mut hidden = vec![false; self.model.blocks.len()];
        for (i, (ix, level, key)) in heads.iter().enumerate() {
            if !self.folds.contains(key) {
                continue;
            }
            let end = heads[i + 1..]
                .iter()
                .find(|(_, l, _)| l <= level)
                .map(|(next, _, _)| *next)
                .unwrap_or(hidden.len());
            for h in hidden.iter_mut().take(end).skip(ix + 1) {
                *h = true;
            }
        }
        self.hidden = hidden;
    }

    fn toggle_heading(&mut self, key: FoldKey, cx: &mut Context<Self>) {
        match self.folds.iter().position(|k| *k == key) {
            Some(at) => {
                self.folds.remove(at);
            }
            None => self.folds.push(key),
        }
        self.recompute_hidden();
        cx.notify();
    }

    /// The folds by key, for the editor to take over on a mode switch.
    pub fn fold_keys(&self) -> Vec<FoldKey> {
        self.folds.clone()
    }

    pub fn set_fold_keys(&mut self, keys: Vec<FoldKey>, cx: &mut Context<Self>) {
        self.folds = keys;
        self.recompute_hidden();
        cx.notify();
    }

    pub fn set_fold_headings(&mut self, on: bool) {
        self.fold_headings = on;
    }

    pub fn properties_folded(&self) -> bool {
        self.toggled.contains(&PROPERTIES_FOLD_ID)
    }

    pub fn set_properties_folded(&mut self, on: bool, cx: &mut Context<Self>) {
        if on != self.properties_folded() {
            self.toggle_callout(PROPERTIES_FOLD_ID, cx);
        }
    }

    /// `editor:toggle-fold-properties` run from a preview leaf, which
    /// Obsidian honours.
    pub fn toggle_properties_fold(&mut self, cx: &mut Context<Self>) {
        self.toggle_callout(PROPERTIES_FOLD_ID, cx);
    }

    /// Scrolls so the top-level block `ix` is at the top of the view.
    /// The note on screen, as built.
    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn scroll_to_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        // A jump into a collapsed section expands it, as a link into a
        // folded section was observed doing in the editor.
        if self.hidden.get(ix).copied().unwrap_or(false) {
            let heads = self.headings();
            let hiding: Vec<FoldKey> = heads
                .iter()
                .enumerate()
                .filter(|(i, (at, level, _))| {
                    let end = heads[i + 1..]
                        .iter()
                        .find(|(_, l, _)| l <= level)
                        .map(|(next, _, _)| *next)
                        .unwrap_or(self.model.blocks.len());
                    *at < ix && ix < end
                })
                .map(|(_, (_, _, key))| key.clone())
                .collect();
            self.folds.retain(|k| !hiding.contains(k));
            self.recompute_hidden();
        }
        if ix < self.model.blocks.len() {
            self.list.scroll_to(ListOffset {
                item_ix: ix,
                offset_in_item: px(0.),
            });
            cx.notify();
        }
    }

    pub fn scroll_to_offset(&mut self, offset: usize, cx: &mut Context<Self>) {
        if let Some(ix) = self.model.block_at(offset) {
            self.scroll_to_block(ix, cx);
        }
    }

    /// Byte offset of the top-level block at the top of the view, which
    /// is what the Outline marks in reading mode.
    pub fn top_offset(&self) -> Option<usize> {
        let top = self.list.logical_scroll_top();
        self.model.starts.get(top.item_ix).copied()
    }

    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.scroll_to_end();
        cx.notify();
    }

    /// Scrolls by `distance` pixels, the way the wheel does. A distance past
    /// either end is left to the list: its layout pass walks the scroll top
    /// back until the content fills the view again, which is what
    /// [`ReadingView::scroll_to_end`] relies on too.
    fn scroll_by(&mut self, distance: f32, cx: &mut Context<Self>) {
        self.list.scroll_by(px(distance));
        cx.notify();
    }

    /// One page of the painted viewport, down or up.
    fn page(&mut self, down: bool, cx: &mut Context<Self>) {
        let height = f32::from(self.list.viewport_bounds().size.height);
        let dy = page_distance(height);
        self.scroll_by(if down { dy } else { -dy }, cx);
    }

    /// One rendered line, down or up.
    fn line(&mut self, down: bool, cx: &mut Context<Self>) {
        let dy = line_distance(cx.global::<Theme>().base_font_size);
        self.scroll_by(if down { dy } else { -dy }, cx);
    }

    fn scroll_page_down(&mut self, _: &ScrollPageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page(true, cx);
    }

    fn scroll_page_up(&mut self, _: &ScrollPageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page(false, cx);
    }

    fn scroll_line_down(&mut self, _: &ScrollLineDown, _: &mut Window, cx: &mut Context<Self>) {
        self.line(true, cx);
    }

    fn scroll_line_up(&mut self, _: &ScrollLineUp, _: &mut Window, cx: &mut Context<Self>) {
        self.line(false, cx);
    }

    fn scroll_to_top(&mut self, _: &ScrollToTop, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_to_block(0, cx);
    }

    fn scroll_to_bottom(&mut self, _: &ScrollToBottom, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_to_end(cx);
    }

    /// What the reader is looking at, for the control socket's state dump.
    /// The scroll position is the list's own: the top-level block at the top
    /// of the view and how far into it the view starts, which is what
    /// [`ReadingView::scroll_to_block`] takes back.
    pub fn describe(&self) -> Value {
        let top = self.list.logical_scroll_top();
        json!({
            "path": self.path,
            "scroll": {
                "item": top.item_ix,
                "offset": f32::from(top.offset_in_item),
            },
            // The reader's text selection, the way `editor.selection`
            // reports the editor's: the two ends as the view holds them,
            // in document order, and the plain text they cover. `null`
            // with nothing selected.
            "selection": self.selection.and_then(|(anchor, head)| {
                let runs = runs(&self.model);
                let (start, end) = ordered(&runs, anchor, head)?;
                Some(json!({
                    "anchor": describe_pos(start),
                    "head": describe_pos(end),
                    "runs": selected_ranges(&runs, start, end).len(),
                    "text": selected_text(&runs, start, end),
                }))
            }),
            // An embedded base draws no chrome of its own, so this is the
            // only way one is visible from outside the app.
            "bases": crate::bases::embed::describe_all(&self.model.blocks),
            // And an embedded PDF likewise: its pages are images and its
            // box has no title, so the dump is what names it.
            "pdfs": crate::pdf::embed::describe_all(&self.model.blocks),
        })
    }

    /// Shows the check on a code block's copy button for as long as
    /// Obsidian shows its own: sampled every 100 ms from the click, its
    /// icon is `lucide-check` through 900 ms and back to `lucide-copy` at
    /// 1000.
    fn flash_copied(&mut self, id: usize, cx: &mut Context<Self>) {
        self.copied = Some(id);
        self.flash = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1000))
                .await;
            this.update(cx, |view, cx| {
                view.copied = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn toggle_callout(&mut self, id: usize, cx: &mut Context<Self>) {
        if !self.toggled.remove(&id) {
            self.toggled.insert(id);
        }
        cx.notify();
    }

    fn render_block(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.global::<Theme>().clone();
        let model = self.model.clone();
        let Some(block) = model.blocks.get(ix) else {
            return div().into_any_element();
        };
        if self.hidden.get(ix).copied().unwrap_or(false) {
            // Collapsed content takes no room at all.
            return div().into_any_element();
        }
        // The block's inner width: the view minus its padding, capped at the
        // readable line length.
        let mut available = f32::from(self.width) - 64.;
        if self.readable_line_length {
            available = available.min(700.);
        }
        let view = cx.entity();
        let on_link: LinkHandler = Rc::new({
            let view = view.clone();
            move |target, window, cx| {
                let modifiers = window.modifiers();
                let new_tab = modifiers.control || modifiers.platform;
                view.update(cx, |_, cx| {
                    cx.emit(if new_tab {
                        ReadingEvent::OpenLinkInNewTab(target)
                    } else {
                        ReadingEvent::OpenLink(target)
                    })
                });
            }
        });
        let on_toggle: ToggleHandler = Rc::new({
            let view = view.clone();
            move |id, cx| {
                view.update(cx, |v, cx| v.toggle_callout(id, cx));
            }
        });
        let on_copy: CopyHandler = Rc::new({
            let view = view.clone();
            move |id, cx| {
                view.update(cx, |v, cx| v.flash_copied(id, cx));
            }
        });
        let on_task: TaskHandler = Rc::new({
            let view = view.clone();
            move |marker, cx| {
                view.update(cx, |v, cx| {
                    if let Some(path) = v.path.clone() {
                        cx.emit(TaskToggled { path, marker });
                    }
                });
            }
        });
        let on_remove_item: PropertyHandler = Rc::new({
            let view = view.clone();
            move |action, _window, cx| {
                // The block is read-only but for this one gesture; a host
                // that handed anything else over would be a bug, and it is
                // ignored rather than written.
                if let PropertyAction::RemoveItem { key, index } = action {
                    view.update(cx, |v, cx| {
                        if let Some(path) = v.path.clone() {
                            cx.emit(PropertyItemRemoved { path, key, index });
                        }
                    });
                }
            }
        });
        let host = ReadingHost {
            properties: self.properties_in_document,
            copied: self.copied,
            on_copy,
            on_task,
            on_remove_item: Some(on_remove_item),
        };
        // Selection: the runs of this block take the mouse themselves, so
        // the press lands on the text it hit rather than being fitted to a
        // bounding box afterwards, and the ranges already selected ride in
        // to be painted.
        let select = SelectHost {
            ranges: self.selected.clone(),
            color: selection_color(&theme),
            on_select: Rc::new({
                let view = view.clone();
                move |gesture, rich, offset, _window, cx| {
                    view.update(cx, |v, cx| {
                        v.gesture(
                            gesture,
                            ReadingPos {
                                block: ix,
                                rich,
                                offset,
                            },
                            cx,
                        )
                    });
                }
            }),
        };
        let r = Renderer::new(
            &theme,
            _window.text_system().clone(),
            available,
            &self.toggled,
            &self.property_types,
            // Read-only: the reading view edits neither properties nor
            // table cells.
            None,
            None,
            on_link,
            on_toggle,
        )
        .with_reading_host(&host)
        .with_selection(&select);
        let mut inner = r.block(block, false);
        // A heading carries Obsidian's `heading-collapse-indicator`: a
        // chevron in the margin to its left, shown on hover or while
        // collapsed, that only its own click toggles (the heading's text
        // and the space beside it were observed doing nothing). `foldHeading`
        // off hides the affordance. The heading is drawn here rather than
        // by the renderer so that its margins sit outside the box the
        // chevron is placed in.
        if let RBlock::Heading { level, text, .. } = block {
            if self.fold_headings {
                if let Some((_, _, key)) = self.headings().into_iter().find(|(at, _, _)| *at == ix)
                {
                    let collapsed = self.folds.contains(&key);
                    let base = theme.base_font_size;
                    let (size, weight, line_height) = crate::theme::heading_style(*level);
                    let top = match level {
                        1 => 1.0,
                        2 => 2.5,
                        _ => 1.5,
                    };
                    let line_px = base * size * line_height;
                    let view = view.clone();
                    // The box reaches 28 px into the left margin, where the
                    // chevron sits, so that hovering the chevron itself
                    // counts as hovering the heading; the padding puts the
                    // text back where it was.
                    inner = div()
                        .mt(px(base * top))
                        .mb(px(base))
                        .ml(px(-28.))
                        .pl(px(28.))
                        .relative()
                        .group("heading")
                        .child(
                            div()
                                .text_size(px(base * size))
                                .line_height(px(line_px))
                                .font_weight(FontWeight(weight))
                                .child(r.rich_at(text, base * size)),
                        )
                        .child(
                            div()
                                .id(("heading-fold", ix))
                                .absolute()
                                .left(px(0.))
                                .top(px((line_px - 16.) / 2.))
                                .size(px(16.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .when(!collapsed, |d| d.invisible())
                                .group_hover("heading", |s| s.visible())
                                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                    let key = key.clone();
                                    view.update(cx, |v, cx| v.toggle_heading(key, cx));
                                })
                                .child(icon(
                                    if collapsed {
                                        "chevron-right"
                                    } else {
                                        "chevron-down"
                                    },
                                    16.,
                                    theme.text_muted,
                                )),
                        )
                        .into_any_element();
                }
            }
        }
        let last = ix + 1 == model.blocks.len();
        div()
            .w_full()
            .flex()
            .flex_row()
            .justify_center()
            .px(px(32.))
            .when(last, |d| d.pb(px(120.)))
            .child(
                div()
                    .w_full()
                    .when(self.readable_line_length, |d| d.max_w(px(700.)))
                    .child(inner),
            )
            .into_any_element()
    }
}

impl Render for ReadingView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let base = theme.base_font_size;
        let view = cx.entity().downgrade();
        // Obsidian's `div.mod-header`: the inline title, the note's basename
        // at h1 size, above the properties block. A sibling of the list
        // rather than its first item, so block indices, `scroll_to_block`
        // and the folds keep their meaning; it holds the top padding the
        // first block used to carry, so the blocks sit where they did.
        let title = self
            .inline_title()
            .map(|t| SharedString::from(t.to_string()));
        let (size, weight, line_height) = crate::theme::heading_style(1);
        let h1 = base * size;
        let header = div()
            .w_full()
            .flex()
            .flex_row()
            .justify_center()
            .px(px(32.))
            .pt(px(HEADER_TOP))
            .child(
                div()
                    .w_full()
                    .when(self.readable_line_length, |d| d.max_w(px(700.)))
                    .children(title.map(|title| {
                        div()
                            .text_size(px(h1))
                            .line_height(px(h1 * line_height))
                            .font_weight(FontWeight(weight))
                            .mb(px(h1 * 0.5))
                            .child(title)
                    })),
            );
        div()
            .key_context("ReadingView")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::scroll_page_down))
            .on_action(cx.listener(Self::scroll_page_up))
            .on_action(cx.listener(Self::scroll_line_down))
            .on_action(cx.listener(Self::scroll_line_up))
            .on_action(cx.listener(Self::scroll_to_top))
            .on_action(cx.listener(Self::scroll_to_bottom))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            // A left press anywhere in the view drops the selection; a
            // press that landed on a run puts one back, because the run's
            // own handler is a bubble listener and so runs after this
            // capture one. A right press leaves the selection alone, so a
            // context menu can still act on it.
            .capture_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                if event.button == MouseButton::Left {
                    this.clear_selection(cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .size_full()
            .font_family(theme.text_font.clone())
            .text_size(px(base))
            .line_height(px(base * LINE_HEIGHT))
            .text_color(theme.text)
            .relative()
            .flex()
            .flex_col()
            .child(header)
            .child(
                list(self.list.clone(), move |ix, window, cx| {
                    view.update(cx, |this, cx| this.render_block(ix, window, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .w_full()
                .flex_1()
                .min_h(px(0.)),
            )
            .child(
                // Records the painted width; a change lays the blocks out again.
                canvas(
                    {
                        let view = cx.entity();
                        move |bounds, _, cx| {
                            view.update(cx, |this, cx| {
                                if this.width != bounds.size.width {
                                    this.width = bounds.size.width;
                                    cx.notify();
                                }
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }
}

/// Builds GPUI elements for model blocks; the reading view and Live
/// Preview's widgets share it, each with its own click handlers.
pub(crate) struct Renderer<'a> {
    theme: &'a Theme,
    toggled: &'a HashSet<usize>,
    /// The vault's `types.json`, so a property row can draw the type its key
    /// is assigned rather than only the one its value infers.
    property_types: &'a PropertyTypes,
    /// Set by Live Preview to make the properties block editable.
    properties_host: Option<&'a PropertiesHost>,
    /// Set by Live Preview to make a table's cells editable.
    table_host: Option<&'a TableHost>,
    /// Set by the reading view for the affordances only it draws.
    reading_host: Option<&'a ReadingHost>,
    /// Set by the reading view so its text can be selected and the
    /// selection painted.
    select: Option<&'a SelectHost>,
    on_link: LinkHandler,
    on_toggle: ToggleHandler,
    text_system: Arc<WindowTextSystem>,
    /// Inner width of a top-level block, in pixels.
    available: f32,
}

impl<'a> Renderer<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        theme: &'a Theme,
        text_system: Arc<WindowTextSystem>,
        available: f32,
        toggled: &'a HashSet<usize>,
        property_types: &'a PropertyTypes,
        properties_host: Option<&'a PropertiesHost>,
        table_host: Option<&'a TableHost>,
        on_link: LinkHandler,
        on_toggle: ToggleHandler,
    ) -> Renderer<'a> {
        Renderer {
            theme,
            toggled,
            property_types,
            properties_host,
            table_host,
            reading_host: None,
            select: None,
            on_link,
            on_toggle,
            text_system,
            available: available.max(100.),
        }
    }

    /// Makes the text selectable: each run takes the mouse and paints the
    /// part of it the selection covers. Live Preview leaves it unset — the
    /// editor's own selection is drawn over its source lines.
    pub(crate) fn with_selection(mut self, host: &'a SelectHost) -> Renderer<'a> {
        self.select = Some(host);
        self
    }

    /// The reading view's own settings and handlers. Live Preview leaves
    /// them unset, so its widgets keep the one click path they have.
    pub(crate) fn with_reading_host(mut self, host: &'a ReadingHost) -> Renderer<'a> {
        self.reading_host = Some(host);
        self
    }

    /// Width of `text` set in the note's font, in pixels.
    fn measure(&self, text: &str, font_size: f32, bold: bool, mono: bool) -> f32 {
        let t = self.theme;
        let font = Font {
            family: if mono {
                t.mono_font.clone()
            } else {
                t.text_font.clone()
            },
            features: Default::default(),
            fallbacks: None,
            weight: if bold {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            },
            style: FontStyle::Normal,
        };
        let run = TextRun {
            len: text.len(),
            font,
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = self.text_system.shape_line(
            SharedString::from(text.to_string()),
            px(font_size),
            &[run],
            None,
        );
        f32::from(line.width)
    }

    /// Column widths the way an HTML table's automatic layout sizes them: a
    /// column wants its longest cell (max-content) and needs at least its
    /// longest word (min-content); when the note is too narrow for every
    /// column's wish, the slack is shared in proportion to each column's
    /// range, and below the minimum the table overflows.
    /// `editing` is the column a cell is being typed into and the text in
    /// hand, so the column grows with what is typed rather than clipping it.
    fn column_widths(
        &self,
        header: &[RichText],
        rows: &[Vec<RichText>],
        font_size: f32,
        editing: Option<(usize, &str)>,
    ) -> Vec<f32> {
        const PADDING: f32 = 17.;
        let count = header
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        let mut max = vec![0f32; count];
        let mut min = vec![0f32; count];
        let mut fold = |ix: usize, cell: &RichText, bold: bool| {
            let mono = cell.runs.iter().any(|(_, s)| s.code || s.kbd);
            let longest_line = cell
                .text
                .lines()
                .max_by_key(|l| l.chars().count())
                .unwrap_or("");
            let longest_word = cell
                .text
                .split_whitespace()
                .max_by_key(|w| w.chars().count())
                .unwrap_or("");
            max[ix] = max[ix].max(self.measure(longest_line, font_size, bold, mono));
            min[ix] = min[ix].max(self.measure(longest_word, font_size, bold, mono));
        };
        for (ix, cell) in header.iter().enumerate() {
            fold(ix, cell, true);
        }
        for row in rows {
            for (ix, cell) in row.iter().enumerate() {
                fold(ix, cell, false);
            }
        }
        if let Some((ix, text)) = editing.filter(|(ix, _)| *ix < count) {
            let width = self.measure(text, font_size, false, false);
            max[ix] = max[ix].max(width);
            min[ix] = min[ix].max(width);
        }
        // Two pixels of slack: the wrapper measures advances a little
        // differently from the shaper and must not break a line that fits.
        let max: Vec<f32> = max.iter().map(|m| m.ceil() + 2. + PADDING).collect();
        let min: Vec<f32> = min
            .iter()
            .map(|m| m.max(8.).ceil() + 2. + PADDING)
            .collect();
        let sum_max: f32 = max.iter().sum();
        let sum_min: f32 = min.iter().sum();
        if sum_max <= self.available {
            max
        } else if sum_min >= self.available {
            min
        } else {
            let share = (self.available - sum_min) / (sum_max - sum_min);
            max.iter()
                .zip(&min)
                .map(|(mx, mn)| (mn + (mx - mn) * share).floor())
                .collect()
        }
    }

    fn blocks(&self, blocks: &[RBlock], tight: bool) -> Vec<AnyElement> {
        blocks.iter().map(|b| self.block(b, tight)).collect()
    }

    pub(crate) fn block(&self, block: &RBlock, tight: bool) -> AnyElement {
        let t = self.theme;
        let base = t.base_font_size;
        let gap = if tight { px(0.) } else { px(base * 0.9) };
        match block {
            RBlock::Properties(entries) => self.properties(entries),
            RBlock::Paragraph(segments) => div()
                .mb(gap)
                .flex()
                .flex_col()
                .children(segments.iter().map(|s| self.segment(s)))
                .into_any_element(),
            RBlock::Heading { level, text, .. } => {
                // Obsidian's scale: h1 1.618em at 700, down to h6 1em at
                // 600, with 1.2 line heights at the top; a 40 px gap above
                // an h2 and 16 px below every heading, the body margin.
                let (size, weight, line_height) = crate::theme::heading_style(*level);
                let top = match level {
                    1 => 1.0,
                    2 => 2.5,
                    _ => 1.5,
                };
                div()
                    .mt(px(base * top))
                    .mb(px(base))
                    .text_size(px(base * size))
                    .line_height(px(base * size * line_height))
                    .font_weight(FontWeight(weight))
                    .child(self.rich_at(text, base * size))
                    .into_any_element()
            }
            RBlock::ThematicBreak => div()
                .my(px(base * 1.5))
                .h(px(1.))
                .w_full()
                .bg(t.border)
                .into_any_element(),
            RBlock::Code { id, text, copy, .. } => {
                let id = *id;
                let payload = copy.clone();
                let on_copy = self.reading_host.map(|h| h.on_copy.clone());
                let copied = self.reading_host.is_some_and(|h| h.copied == Some(id));
                div()
                    .mb(gap)
                    .relative()
                    .group("code-block")
                    .bg(t.code_bg)
                    .rounded_md()
                    .py_2()
                    .font_family(t.mono_font.clone())
                    .text_size(px(base * 0.875))
                    .line_height(px(base * 0.875 * 1.5))
                    .child(
                        // Code keeps its columns: long lines scroll sideways.
                        div().id(("code", id)).overflow_x_scroll().px_3().child(
                            div()
                                .whitespace_nowrap()
                                .child(SharedString::from(text.clone())),
                        ),
                    )
                    .child(
                        // Obsidian draws no language name on a rendered
                        // block; the only thing on it is a copy button,
                        // and only while the block is hovered.
                        div()
                            .id(("copy-code", id))
                            .absolute()
                            .top_0()
                            .right_0()
                            .m(px(6.))
                            .px_2()
                            .py(px(6.))
                            .rounded_sm()
                            .invisible()
                            .group_hover("code-block", |s| s.visible())
                            .cursor_pointer()
                            .hover(|s| s.bg(t.bg_secondary))
                            // `lucide-copy`, and `lucide-check` for as long
                            // as the check shows. A 16 px icon in this box
                            // is the 31.98 x 27.97 px button Obsidian paints.
                            .child(icon(
                                if copied { "check" } else { "copy" },
                                16.,
                                t.text_muted,
                            ))
                            .on_click(move |_, _, cx| {
                                // An empty fence's button writes nothing at
                                // all -- a click on one in Obsidian left the
                                // clipboard holding what it already had, so
                                // an empty body is not copied as an empty
                                // string. Whether its check still flashes is
                                // unobserved; flashing one for a copy that
                                // did not happen would be a lie, so it does
                                // not.
                                if payload.is_empty() {
                                    return;
                                }
                                cx.write_to_clipboard(ClipboardItem::new_string(payload.clone()));
                                if let Some(on_copy) = &on_copy {
                                    on_copy(id, cx);
                                }
                            }),
                    )
                    .into_any_element()
            }
            RBlock::Html(text) => div()
                .mb(gap)
                .font_family(t.mono_font.clone())
                .text_size(px(base * 0.875))
                .text_color(t.text_muted)
                .child(SharedString::from(text.clone()))
                .into_any_element(),
            RBlock::Quote(children) => div()
                .mb(gap)
                .border_l_2()
                .border_color(t.accent)
                .pl_4()
                .flex()
                .flex_col()
                .children(self.blocks(children, false))
                .into_any_element(),
            RBlock::Callout {
                kind,
                fold,
                title,
                children,
            } => {
                let color = t.callout_color(kind);
                let foldable = fold.is_some();
                let flipped = self.toggled.contains(&title.id);
                let collapsed = fold.unwrap_or(false) != flipped;
                let id = title.id;
                let on_toggle = self.on_toggle.clone();
                let mut bg = color;
                bg.a = if t.dark { 0.12 } else { 0.08 };
                div()
                    .mb(gap)
                    .bg(bg)
                    .border_l_4()
                    .border_color(color)
                    .rounded_md()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id(("callout", id))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color)
                            // Mouse-down rather than click, and blocking the
                            // mouse rather than letting it through: in Live
                            // Preview the editor's own window listener runs
                            // ahead of this element and would put the caret
                            // on the title line, which reveals the source
                            // and takes the widget — chevron and all — away
                            // before the button-up that a click needs. Scroll
                            // still falls through to the editor behind.
                            .when(foldable, |d| {
                                d.cursor_pointer()
                                    .block_mouse_except_scroll()
                                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                        on_toggle(id, cx)
                                    })
                            })
                            // The type's Lucide icon, in the callout's own
                            // colour, which is where Obsidian puts it and
                            // what Obsidian paints it (see [`callout_icon`]).
                            .child(icon(callout_icon(kind), 16., color))
                            .child(
                                div().flex_1().child(
                                    // Where the title holds a link the link
                                    // wins: Obsidian navigates and leaves the
                                    // fold alone, so the text keeps the
                                    // mouse-down to itself. The chevron and the
                                    // rest of the row still fold.
                                    match foldable && !title.links.is_empty() {
                                        true => div()
                                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .child(self.rich(title))
                                            .into_any_element(),
                                        false => self.rich(title).into_any_element(),
                                    },
                                ),
                            )
                            .when(foldable, |d| {
                                // Obsidian's `.callout-fold` holds
                                // `lucide-chevron-down` collapsed and
                                // expanded alike, turned a quarter turn when
                                // collapsed by something that is not a
                                // transform on that element;
                                // `chevron-right` is the same drawing
                                // already turned.
                                d.child(icon(
                                    if collapsed {
                                        "chevron-right"
                                    } else {
                                        "chevron-down"
                                    },
                                    16.,
                                    t.text_muted,
                                ))
                            }),
                    )
                    .when(!collapsed && !children.is_empty(), |d| {
                        d.child(
                            div()
                                .mt_2()
                                .flex()
                                .flex_col()
                                .children(self.blocks(children, false)),
                        )
                    })
                    .into_any_element()
            }
            RBlock::List { tight, items } => div()
                .mb(gap)
                .flex()
                .flex_col()
                .children(items.iter().map(|item| self.item(item, *tight)))
                .into_any_element(),
            RBlock::Table {
                alignments,
                header,
                rows,
                spans,
            } => {
                let font_size = base * 0.9375;
                let host = self.table_host;
                // The column holding the open cell measures the text in
                // hand, so it grows as the cell is typed into.
                let editing = host.and_then(|h| h.cell.as_ref()).and_then(|(at, f)| {
                    spans
                        .iter()
                        .find_map(|row| row.iter().position(|s| s == at))
                        .map(|ix| (ix, f.text.as_str()))
                });
                let columns = self.column_widths(header, rows, font_size, editing);
                let table_width: f32 = columns.iter().sum();
                // `row` is 0 for the header, so the span of every cell is
                // where the model put it.
                let cell = |text: &RichText, row: usize, ix: usize, head: bool| -> AnyElement {
                    let align = alignments.get(ix).copied().unwrap_or(Alignment::None);
                    let width = columns.get(ix).copied().unwrap_or(80.);
                    let span = spans.get(row).and_then(|r| r.get(ix)).cloned();
                    // The cell being typed into shows the text in hand with
                    // a caret, the way a property row's field does.
                    let editing = host
                        .and_then(|h| h.cell.as_ref())
                        .filter(|(at, _)| Some(at) == span.as_ref())
                        .map(|(_, f)| f);
                    let drawn = div()
                        .w(px(width))
                        .flex_shrink_0()
                        .px_2()
                        .py_1()
                        .border_r_1()
                        .border_color(t.border)
                        .when(head, |d| d.font_weight(FontWeight::SEMIBOLD))
                        .map(|d| match align {
                            Alignment::Center => d.text_center(),
                            Alignment::Right => d.text_right(),
                            _ => d,
                        })
                        .map(|d| match editing {
                            Some(field) => d.child(self.property_field(field, false)),
                            None => d.child(self.rich(text)),
                        });
                    // A cell is clicked into, the way a property row is.
                    match (host, span) {
                        (Some(host), Some(span)) => {
                            let on_action = host.on_action.clone();
                            drawn
                                .id(("table-cell", span.start))
                                .cursor_text()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    on_action(TableAction::Edit { span: span.clone() }, window, cx);
                                })
                                .into_any_element()
                        }
                        _ => drawn.into_any_element(),
                    }
                };
                // The table is as wide as its columns, like an HTML table.
                div()
                    .mb(gap)
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .w(px(table_width))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .border_1()
                            .border_color(t.border)
                            .rounded_sm()
                            .text_size(px(font_size))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .bg(t.bg_secondary)
                                    .border_b_1()
                                    .border_color(t.border)
                                    .children(
                                        header.iter().enumerate().map(|(i, c)| cell(c, 0, i, true)),
                                    ),
                            )
                            .children(rows.iter().enumerate().map(|(ri, row)| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .when(ri + 1 < rows.len(), |d| {
                                        d.border_b_1().border_color(t.border)
                                    })
                                    .children(
                                        row.iter()
                                            .enumerate()
                                            .map(|(i, c)| cell(c, ri + 1, i, false)),
                                    )
                            })),
                    )
                    .into_any_element()
            }
            RBlock::Math(text) => self.math_block(text, gap),
            RBlock::Footnotes(notes) => div()
                .mt(px(base * 2.))
                .pt(px(base))
                .border_t_1()
                .border_color(t.border)
                .text_size(px(base * 0.875))
                .flex()
                .flex_col()
                .children(notes.iter().map(|(n, blocks)| {
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .w(px(28.))
                                .flex_shrink_0()
                                .text_color(t.text_muted)
                                .child(SharedString::from(format!("{n}."))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .children(self.blocks(blocks, true)),
                        )
                }))
                .into_any_element(),
        }
    }

    fn item(&self, item: &RItem, tight: bool) -> AnyElement {
        let t = self.theme;
        let base = t.base_font_size;
        let marker: AnyElement = match (item.task, item.number) {
            (Some(c), _) => {
                let checked = c != ' ';
                let check = div()
                    .w(px(16.))
                    .h(px(16.))
                    .mt(px(base * 0.3))
                    .rounded_sm()
                    .border_1()
                    .border_color(if checked { t.accent } else { t.text_muted })
                    .when(checked, |d| d.bg(t.accent))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.))
                    .text_color(t.bg)
                    .when(checked, |d| d.child("✓"));
                // Clicking the box rewrites its line, the way it does in
                // Live Preview. An item that came out of an embed has no
                // marker and stays inert: those bytes are another file's.
                match (self.reading_host, item.marker) {
                    (Some(host), Some(marker)) => {
                        let on_task = host.on_task.clone();
                        check
                            .id(("task", marker.line))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| on_task(marker, cx))
                            .into_any_element()
                    }
                    _ => check.into_any_element(),
                }
            }
            (None, Some(n)) => div()
                .text_color(t.text_muted)
                .child(SharedString::from(format!("{n}.")))
                .into_any_element(),
            (None, None) => div().text_color(t.text_muted).child("•").into_any_element(),
        };
        let done = item.task.is_some_and(|c| c != ' ');
        div()
            .flex()
            .flex_row()
            .gap_2()
            .when(!tight, |d| d.mb(px(base * 0.4)))
            .child(
                div()
                    .min_w(px(22.))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .child(marker),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    // Obsidian's default theme dims a done task and
                    // strikes it through (`--checklist-done-decoration`).
                    .when(done, |d| d.text_color(t.text_muted).line_through())
                    .children(self.blocks(&item.children, tight)),
            )
            .into_any_element()
    }

    fn segment(&self, segment: &Segment) -> AnyElement {
        match segment {
            Segment::Text(text) => self.rich(text),
            Segment::Embed(embed) => self.embed(embed),
            // Display math inside a paragraph is a block of its own; the
            // paragraph's own gap follows the paragraph, not this.
            Segment::Math(tex) => self.math_block(tex, px(0.)),
        }
    }

    /// `$$...$$`: the formula typeset in display style, centred, at the
    /// body size, and wide formulas scroll sideways rather than wrap, as
    /// MathJax's display container does. A formula that does not parse
    /// shows its source in the error red, the way MathJax's error output
    /// does.
    fn math_block(&self, tex: &str, gap: Pixels) -> AnyElement {
        let t = self.theme;
        let base = t.base_font_size;
        let tex = tex.trim();
        match math::typeset(tex, true) {
            Ok(formula) => div()
                .id(ElementId::Name(format!("math-block:{tex}").into()))
                .mb(gap)
                .py(px(base * 0.5))
                .w_full()
                .overflow_x_scroll()
                .flex()
                .flex_row()
                .child(div().mx_auto().flex_shrink_0().child(FormulaElement::new(
                    formula,
                    px(base),
                    t.text,
                )))
                .into_any_element(),
            Err(_) => div()
                .mb(gap)
                .py(px(base * 0.5))
                .text_color(math::error_color())
                .text_center()
                .child(SharedString::from(tex.to_string()))
                .into_any_element(),
        }
    }

    fn embed(&self, embed: &Embed) -> AnyElement {
        let t = self.theme;
        let base = t.base_font_size;
        match &embed.kind {
            EmbedKind::Image(path) => {
                let mut image = img(path.clone())
                    .max_w_full()
                    .object_fit(ObjectFit::Contain);
                if let Some(w) = embed.width {
                    image = image.w(px(w));
                }
                if let Some(h) = embed.height {
                    image = image.h(px(h));
                }
                div().my_1().child(image).into_any_element()
            }
            EmbedKind::Note {
                path,
                title,
                blocks,
            } => {
                let on_link = self.on_link.clone();
                let link = embed.link.clone();
                div()
                    .my_2()
                    .border_1()
                    .border_color(t.border)
                    .rounded_md()
                    .px_4()
                    .py_3()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id(("embed-title", path.len() + title.len()))
                            .mb_2()
                            .text_size(px(base * 0.75))
                            .text_color(t.text_muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(t.link))
                            .on_click(move |_, window, cx| {
                                on_link(LinkTarget::Internal(link.clone()), window, cx)
                            })
                            .child(SharedString::from(format!("↗ {title}"))),
                    )
                    .children(self.blocks(blocks, false))
                    .into_any_element()
            }
            // `![[x.base]]`: the base's own drawing is `bases::embed`, which
            // takes the view resolved by the same engine the open view uses.
            // Every click inside it is an internal link by vault path — the
            // title opens the `.base`, a row opens the file that row came
            // from — which is how every other embed opens its target.
            EmbedKind::Base(base) => {
                let on_link = self.on_link.clone();
                let open: crate::bases::embed::OpenPath =
                    Rc::new(move |path: &str, window: &mut Window, cx: &mut App| {
                        on_link(LinkTarget::Internal(path.to_string()), window, cx)
                    });
                crate::bases::embed::render(base, t, open)
            }
            // `![[x.pdf]]`: the pages are drawn by `pdf::embed`, which asks
            // the document cache for them when it is painted; the model
            // carried only the path and the subpath. A click opens the file
            // by vault path, as every other embed's target is opened.
            EmbedKind::Pdf(pdf) => {
                let on_link = self.on_link.clone();
                let open: crate::pdf::embed::OpenPath =
                    Rc::new(move |path: &str, window: &mut Window, cx: &mut App| {
                        on_link(LinkTarget::Internal(path.to_string()), window, cx)
                    });
                crate::pdf::embed::render(pdf, t, self.available, open)
            }
            // A missing attachment is Obsidian's `file-embed
            // mod-empty-attachment` box (observed on `![[missing.pdf]]`,
            // `tools/obsidian-probe/results/2026-10-26-pdf.json` s08): a
            // block padded 8 px with an 8 px radius on the alternate
            // ground, 14 px muted text saying `“missing.pdf” could not be
            // found.` A missing note keeps the shell's own line.
            EmbedKind::Missing
                if FileKind::from_path(scoria_markdown::parse_linktext(&embed.link).path)
                    != FileKind::Markdown =>
            {
                div()
                    .my_1()
                    .p(px(8.))
                    .rounded(px(8.))
                    .bg(t.bg_alt)
                    .text_size(px(14.))
                    .text_color(t.text_muted)
                    .child(SharedString::from(format!(
                        "\u{201c}{}\u{201d} could not be found.",
                        embed.link
                    )))
                    .into_any_element()
            }
            EmbedKind::Missing => div()
                .my_1()
                .text_color(t.text_muted)
                .italic()
                .child(SharedString::from(format!("{}: file not found", embed.alt)))
                .into_any_element(),
            EmbedKind::Unsupported(kind) => {
                let on_link = self.on_link.clone();
                let link = embed.link.clone();
                div()
                    .id(("embed-file", embed.link.len()))
                    .my_1()
                    .text_color(t.link)
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        on_link(LinkTarget::Internal(link.clone()), window, cx)
                    })
                    .child(SharedString::from(format!("{} ({kind:?})", embed.alt)))
                    .into_any_element()
            }
            EmbedKind::TooDeep => div()
                .my_1()
                .text_color(t.text_muted)
                .italic()
                .child(SharedString::from(format!(
                    "{}: embed nested too deep",
                    embed.alt
                )))
                .into_any_element(),
        }
    }

    /// The properties block: a foldable "Properties" heading and one row per
    /// property, each with its type glyph, its key and its value. Links and
    /// tags inside a value are clickable. Without a [`PropertiesHost`] the
    /// rows are not editable, bar a list pill's remove control, which the
    /// reading view takes through [`ReadingHost::on_remove_item`]; the fields,
    /// type menu and add-row control are the way the reading view still
    /// differs from Obsidian's (it edits properties in reading view too).
    fn properties(&self, entries: &[Property]) -> AnyElement {
        let t = self.theme;
        let base = t.base_font_size;
        let host = self.properties_host;
        let hidden = host.is_some_and(|h| h.mode == PropertiesInDocument::Hidden)
            || self
                .reading_host
                .is_some_and(|h| h.properties == PropertiesInDocument::Hidden);
        if hidden {
            // "Hide" in `propertiesInDocument`: the YAML stays in the file,
            // and neither the editor nor the rendered text shows it.
            return div().into_any_element();
        }
        let folded = self.toggled.contains(&PROPERTIES_FOLD_ID);
        let mut next_id = PROPERTY_ID_BASE;
        let rows = crate::properties::rows(
            entries,
            |key| scoria_formats::properties::type_of(key, entries, self.property_types),
            &mut next_id,
        );
        let on_toggle = self.on_toggle.clone();
        let add = host.map(|h| h.on_action.clone());
        div()
            .mb(px(base * 1.2))
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border)
            .pb(px(base * 0.6))
            .text_size(px(base * 0.875))
            .child(
                div()
                    .id("properties-heading")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .py_1()
                    .cursor_pointer()
                    .text_color(t.text_muted)
                    .child(SharedString::from(if folded {
                        "\u{25b8}"
                    } else {
                        "\u{25be}"
                    }))
                    .child(SharedString::from("Properties"))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        on_toggle(PROPERTIES_FOLD_ID, cx);
                    }),
            )
            .when(!folded, |d| {
                d.children(rows.into_iter().map(|row| self.property_row(row)))
            })
            .when_some(
                host.and_then(|h| h.field.clone())
                    .filter(|f| f.target == Target::New && !folded),
                |d, field| {
                    // The row "Add file property" opened: a key field with
                    // nothing written to the note behind it yet.
                    d.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .py_1()
                            .child(
                                div()
                                    .w(px(base))
                                    .flex_shrink_0()
                                    .text_color(t.text_faint)
                                    .child(SharedString::from("T")),
                            )
                            .child(self.property_field(&field, false)),
                    )
                },
            )
            .when_some(
                add.filter(|_| {
                    !folded
                        && !host.is_some_and(|h| {
                            h.field.as_ref().is_some_and(|f| f.target == Target::New)
                        })
                }),
                |d, on_action| {
                    // The row Obsidian's "Add file property" opens. Nothing is
                    // written until it is named.
                    d.child(
                        div()
                            .id("properties-add")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .py_1()
                            .cursor_pointer()
                            .text_color(t.text_faint)
                            .child(SharedString::from("+"))
                            .child(SharedString::from("Add property"))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                on_action(PropertyAction::AddRow, window, cx);
                            }),
                    )
                },
            )
            .into_any_element()
    }

    /// The caret-bearing field a row shows while it is being typed into.
    fn property_field(&self, field: &Field, wide: bool) -> AnyElement {
        let t = self.theme;
        let before = &field.text[..field.cursor];
        let after = &field.text[field.cursor..];
        div()
            .flex()
            .flex_row()
            .items_center()
            .when(wide, |d| d.flex_1())
            .min_w_0()
            .px_1()
            .rounded_sm()
            .bg(t.bg_active)
            .child(SharedString::from(before.to_string()))
            .child(
                div()
                    .w(px(1.))
                    .h(px(t.base_font_size))
                    .bg(t.accent)
                    .flex_shrink_0(),
            )
            .child(SharedString::from(after.to_string()))
            .into_any_element()
    }

    fn property_row(&self, row: crate::properties::Row) -> AnyElement {
        let t = self.theme;
        let base = t.base_font_size;
        let host = self.properties_host;
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap_2()
            .py_1()
            .child(
                // The type glyph and the key share the left column, as the
                // properties UI lays them out.
                div()
                    .w(px(160.))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_color(t.text_muted)
                    .child({
                        let glyph = div()
                            .w(px(base))
                            .flex_shrink_0()
                            .text_color(t.text_faint)
                            .child(SharedString::from(crate::properties::type_glyph(row.ty)));
                        match host.map(|h| h.on_action.clone()) {
                            Some(on_action) => {
                                let key = row.key.clone();
                                glyph
                                    .id(("property-type", row.id))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                        on_action(
                                            PropertyAction::OpenTypeMenu { key: key.clone() },
                                            window,
                                            cx,
                                        );
                                    })
                                    .into_any_element()
                            }
                            None => glyph.into_any_element(),
                        }
                    })
                    .child(match self.open_field(&row, Part::Key) {
                        Some(field) => self.property_field(field, true),
                        None => {
                            let cell = div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(SharedString::from(row.key.clone()));
                            match host.map(|h| h.on_action.clone()) {
                                Some(on_action) => {
                                    let key = row.key.clone();
                                    cell.id(("property-key", row.id))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            move |_, window, cx| {
                                                on_action(
                                                    PropertyAction::Edit {
                                                        target: Target::Existing(key.clone()),
                                                        part: Part::Key,
                                                    },
                                                    window,
                                                    cx,
                                                );
                                            },
                                        )
                                        .into_any_element()
                                }
                                None => cell.into_any_element(),
                            }
                        }
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(match self.open_field(&row, Part::Value) {
                        Some(field) => self.property_field(field, true),
                        None => self.property_value_cell(&row, host),
                    })
                    .when(row.mismatched, |d| {
                        // Obsidian marks a value that does not fit its type
                        // and keeps the text as written.
                        d.child(
                            div()
                                .flex_shrink_0()
                                .text_color(t.text_faint)
                                .child(SharedString::from("\u{26a0}")),
                        )
                    })
                    .when_some(host.map(|h| h.on_action.clone()), |d, on_action| {
                        let key = row.key.clone();
                        d.child(
                            div()
                                .id(("property-remove", row.id))
                                .flex_shrink_0()
                                .px_1()
                                .cursor_pointer()
                                .text_color(t.text_faint)
                                .child(SharedString::from("\u{00d7}"))
                                .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                    on_action(
                                        PropertyAction::Remove { key: key.clone() },
                                        window,
                                        cx,
                                    );
                                }),
                        )
                    }),
            )
            .when_some(
                host.filter(|h| h.menu.as_deref() == Some(row.key.as_str())),
                |d, h| d.child(self.property_type_menu(&row, h)),
            )
            .into_any_element()
    }

    /// The type menu Obsidian opens from a row's icon. `tags` and `aliases`
    /// are reserved for their keys and are not offered.
    fn property_type_menu(
        &self,
        row: &crate::properties::Row,
        host: &PropertiesHost,
    ) -> AnyElement {
        let t = self.theme;
        div()
            .ml(px(24.))
            .mb_1()
            .flex()
            .flex_col()
            .border_1()
            .border_color(t.border)
            .rounded_md()
            .bg(t.bg_secondary)
            .py_1()
            .children(PropertyType::selectable().iter().copied().map(|ty| {
                let on_action = host.on_action.clone();
                let key = row.key.clone();
                div()
                    .id(("property-type-option", row.id * 16 + ty as usize))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_0p5()
                    .cursor_pointer()
                    .when(ty == row.ty, |d| d.text_color(t.accent))
                    .hover(|d| d.bg(t.bg_active))
                    .child(SharedString::from(crate::properties::type_glyph(ty)))
                    .child(SharedString::from(ty.label()))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                        on_action(
                            PropertyAction::Retype {
                                key: key.clone(),
                                ty,
                            },
                            window,
                            cx,
                        );
                    })
            }))
            .into_any_element()
    }

    /// The open field when it belongs to this row and part. A field holding
    /// one item of a list is not one of these: the value cell draws it among
    /// the pills, in the place of the item it stands for.
    fn open_field(&self, row: &crate::properties::Row, part: Part) -> Option<&Field> {
        let field = self.properties_host?.field.as_ref()?;
        (field.part == part
            && field.key() == Some(row.key.as_str())
            && field.item_index().is_none())
        .then_some(field)
    }

    /// Which item of this row's list the open field holds, if it holds one.
    /// An index past the last item is the empty input Obsidian keeps after
    /// the pills, where a new item is typed.
    fn open_item(&self, row: &crate::properties::Row) -> Option<(usize, &Field)> {
        let field = self.properties_host?.field.as_ref()?;
        Some((field.item_of(row)?, field))
    }

    /// The value cell when no field is open: read-only in the reading view,
    /// clickable in Live Preview (a checkbox flips, anything else opens a
    /// field).
    fn property_value_cell(
        &self,
        row: &crate::properties::Row,
        host: Option<&PropertiesHost>,
    ) -> AnyElement {
        let Some(on_action) = host.map(|h| h.on_action.clone()) else {
            return self.property_value(row);
        };
        if let crate::properties::Display::Checkbox(checked) = row.value {
            let key = row.key.clone();
            let on_toggle = on_action.clone();
            return div()
                .id(("property-checkbox", row.id))
                .flex_1()
                .min_w_0()
                .cursor_pointer()
                .child(self.property_value(row))
                .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    let _ = checked;
                    on_toggle(PropertyAction::Toggle { key: key.clone() }, window, cx);
                })
                .into_any_element();
        }
        let key = row.key.clone();
        // A list value is never opened whole: a click in the cell lands in
        // the empty input after the pills, so what is typed becomes one
        // more item however many commas it holds. Only a scalar opens on
        // its whole text.
        let append = row.is_list_valued().then_some(row.items.len());
        div()
            .id(("property-value", row.id))
            .flex_1()
            .min_w_0()
            .cursor_text()
            .child(self.property_value(row))
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                let action = match append {
                    Some(index) => PropertyAction::EditItem {
                        key: key.clone(),
                        index,
                    },
                    None => PropertyAction::Edit {
                        target: Target::Existing(key.clone()),
                        part: Part::Value,
                    },
                };
                on_action(action, window, cx);
            })
            .into_any_element()
    }

    fn property_value(&self, row: &crate::properties::Row) -> AnyElement {
        let t = self.theme;
        let open = self.open_item(row);
        match &row.value {
            crate::properties::Display::Scalar(text) => div()
                .flex_1()
                .min_w_0()
                .child(self.rich(text))
                .into_any_element(),
            crate::properties::Display::Pills(items) if !row.is_blank() => {
                self.property_pills(row, items, open)
            }
            // A value with no items still takes a new one, and then the
            // field for it is the only thing the cell has to draw.
            crate::properties::Display::Empty | crate::properties::Display::Pills(_)
                if open.is_some() =>
            {
                self.property_pills(row, &[], open)
            }
            // The box keeps its own size; the cell around it takes the width,
            // so the row's affordances still sit at the right edge.
            crate::properties::Display::Checkbox(checked) => div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .w(px(t.base_font_size))
                        .h(px(t.base_font_size))
                        .border_1()
                        .border_color(t.border)
                        .rounded_sm()
                        .when(*checked, |d| {
                            d.bg(t.accent)
                                .text_color(t.bg)
                                .child(SharedString::from("\u{2713}"))
                        }),
                )
                .into_any_element(),
            // `aliases: []` lands here along with a key that has no value:
            // both are blank, and a blank cell that drew nothing would have
            // no height and so take no click at all.
            crate::properties::Display::Empty | crate::properties::Display::Pills(_) => div()
                .text_color(t.text_faint)
                .child(SharedString::from("Empty"))
                .into_any_element(),
        }
    }

    /// A list value: one pill per item, with the open field standing in for
    /// the item it holds, so editing one item never touches the others. A
    /// field past the last item draws last, as the empty input Obsidian
    /// keeps after the pills.
    ///
    /// In Live Preview each pill is its own affordance — clicking it opens
    /// that item alone, and its remove button drops that item — which is
    /// the markup a real Obsidian was observed to draw
    /// (`div.multi-select-pill` with its own
    /// `div.multi-select-pill-remove-button` holding a `lucide-x`). The
    /// reading view passes no properties host, so a click on the pill
    /// itself opens nothing there, but it draws the same remove button —
    /// Obsidian draws one in its reading view too — and routes it through
    /// [`ReadingHost::on_remove_item`] to the workspace, which writes the
    /// note.
    fn property_pills(
        &self,
        row: &crate::properties::Row,
        items: &[RichText],
        open: Option<(usize, &Field)>,
    ) -> AnyElement {
        let t = self.theme;
        let on_action = self.properties_host.map(|h| h.on_action.clone());
        // The remove control is drawn for whichever host can take the item
        // away: Live Preview's sink, or the reading view's narrower one.
        let remove = on_action
            .clone()
            .or_else(|| self.reading_host.and_then(|h| h.on_remove_item.clone()));
        // The pill a Backspace selected, drawn focused until the second
        // press removes it. It belongs to the open field's row, so it only
        // means anything while a field of this row is open.
        let selected = open.and_then(|_| self.properties_host.and_then(|h| h.selected_pill));
        // A click inside the open field is not a click on the cell behind
        // it, which would commit this item and open a fresh one.
        let item_field = |field: &Field| {
            div()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(self.property_field(field, false))
                .into_any_element()
        };
        let mut cells: Vec<AnyElement> = Vec::new();
        for (ix, item) in items.iter().enumerate() {
            if let Some((at, field)) = open {
                if at == ix {
                    cells.push(item_field(field));
                    continue;
                }
            }
            cells.push(
                div()
                    .id(("property-pill", item.id))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_1p5()
                    .rounded_sm()
                    .bg(t.code_bg)
                    // The border is the pill's own colour until the pill is
                    // the selected one, so taking focus moves nothing.
                    .border_1()
                    .border_color(if selected == Some(ix) {
                        t.border_focus
                    } else {
                        t.code_bg
                    })
                    .child(self.rich(item))
                    .when_some(on_action.clone(), |d, on_action| {
                        let key = row.key.clone();
                        d.cursor_text().on_mouse_down(
                            gpui::MouseButton::Left,
                            move |_, window, cx| {
                                // The cell behind opens the append input;
                                // this pill opens itself instead.
                                cx.stop_propagation();
                                on_action(
                                    PropertyAction::EditItem {
                                        key: key.clone(),
                                        index: ix,
                                    },
                                    window,
                                    cx,
                                );
                            },
                        )
                    })
                    .when_some(remove.clone(), |d, remove| {
                        let key = row.key.clone();
                        d.child(
                            div()
                                .id(("property-pill-remove", item.id))
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                // Obsidian's `.multi-select-pill-remove-button`
                                // holds a `lucide-x`, 14 px wide, and it is
                                // always drawn rather than revealed on hover.
                                .child(crate::assets::icon(
                                    "x",
                                    t.base_font_size * 0.875,
                                    t.text_faint,
                                ))
                                .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                    cx.stop_propagation();
                                    remove(
                                        PropertyAction::RemoveItem {
                                            key: key.clone(),
                                            index: ix,
                                        },
                                        window,
                                        cx,
                                    );
                                }),
                        )
                    })
                    .into_any_element(),
            );
        }
        if let Some((_, field)) = open.filter(|(at, _)| *at >= items.len()) {
            // A selected pill has the focus, so the input does not: it is
            // empty by then (the gesture only starts from an empty one), so
            // dropping its caret leaves nothing of it to draw.
            if selected.is_none() {
                cells.push(item_field(field));
            }
        }
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_1()
            .children(cells)
            .into_any_element()
    }

    /// Inline content at the body size.
    fn rich(&self, text: &RichText) -> AnyElement {
        self.rich_at(text, self.theme.base_font_size)
    }

    /// Inline content set at `size` pixels: one `InteractiveText`, with
    /// inline formulas typeset over placeholders when the text has any.
    pub(crate) fn rich_at(&self, text: &RichText, size: f32) -> AnyElement {
        // Each `$x$` run, typeset in text style. A formula that does not
        // parse stays as its source, in the error red (`highlight`).
        let formulas: Vec<(Range<usize>, Arc<scoria_math::Formula>)> = text
            .runs
            .iter()
            .filter(|(_, s)| s.math)
            .filter_map(|(r, _)| {
                let tex = text.text.get(r.clone())?;
                math::typeset(tex, false).ok().map(|f| (r.clone(), f))
            })
            .collect();
        if formulas.is_empty() {
            let (interactive, layout) =
                self.interactive_with_layout(text, &text.text, &text.runs, &text.links);
            return self.selectable(interactive.into_any_element(), text, &layout);
        }
        let t = self.theme;
        let em = px(size);
        let font = Font {
            family: t.text_font.clone(),
            features: Default::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        };
        let spaces = SpaceWidths::measure(&self.text_system, &font, em);
        // Rebuild the string with a placeholder where each formula was,
        // and carry the other runs and the links across the edits.
        let mut edits: Vec<(Range<usize>, usize)> = Vec::new();
        let mut shaped = String::with_capacity(text.text.len());
        let mut inlines = Vec::new();
        let mut at = 0;
        for (range, formula) in &formulas {
            shaped.push_str(&text.text[at..range.start]);
            let (placeholder, slot) = spaces.placeholder(math::width_px(formula, em));
            let start = shaped.len();
            shaped.push_str(&placeholder);
            inlines.push(Inline {
                range: start..shaped.len(),
                formula: formula.clone(),
                slot,
            });
            edits.push((range.clone(), placeholder.len()));
            at = range.end;
        }
        shaped.push_str(&text.text[at..]);
        let remap = |offset: usize| -> usize {
            let mut shift = 0isize;
            for (range, len) in &edits {
                if offset <= range.start {
                    break;
                }
                if offset < range.end {
                    // Inside a formula: its start.
                    return (range.start as isize + shift) as usize;
                }
                shift += *len as isize - range.len() as isize;
            }
            (offset as isize + shift) as usize
        };
        let runs: Vec<(Range<usize>, InlineStyle)> = text
            .runs
            .iter()
            .filter(|(r, s)| !(s.math && formulas.iter().any(|(f, _)| f == r)))
            .map(|(r, s)| (remap(r.start)..remap(r.end), *s))
            .collect();
        let links: Vec<(Range<usize>, LinkTarget)> = text
            .links
            .iter()
            .map(|(r, l)| (remap(r.start)..remap(r.end), l.clone()))
            .collect();
        let (interactive, layout) = self.interactive_with_layout(text, &shaped, &runs, &links);
        let element = Overlay::new(
            interactive.into_any_element(),
            layout.clone(),
            inlines,
            em,
            t.text,
        )
        .into_any_element();
        self.selectable(element, text, &layout)
    }

    /// Wraps a run of text so a press, a drag or a multiple click over it
    /// reaches the view with the byte offset it landed on. A layout can
    /// only be asked where a point fell once it has been laid out *and*
    /// prepainted, which is why the handlers hang here rather than on the
    /// view's root: gpui registers an element's mouse listeners while
    /// painting it, by when the text inside has both.
    fn selectable(
        &self,
        element: AnyElement,
        text: &RichText,
        layout: &gpui::TextLayout,
    ) -> AnyElement {
        let Some(host) = self.select else {
            return element;
        };
        let id = text.id;
        div()
            .child(element)
            .on_mouse_down(MouseButton::Left, {
                let (on_select, layout) = (host.on_select.clone(), layout.clone());
                move |event: &gpui::MouseDownEvent, window, cx| {
                    let gesture = match event.click_count {
                        0 | 1 => SelectGesture::Press,
                        2 => SelectGesture::Word,
                        _ => SelectGesture::Line,
                    };
                    on_select(gesture, id, index_at(&layout, event.position), window, cx);
                }
            })
            .on_mouse_move({
                let (on_select, layout) = (host.on_select.clone(), layout.clone());
                move |event: &gpui::MouseMoveEvent, window, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        on_select(
                            SelectGesture::Drag,
                            id,
                            index_at(&layout, event.position),
                            window,
                            cx,
                        );
                    }
                }
            })
            .into_any_element()
    }

    /// The `InteractiveText` of `string` styled by `runs`, and its layout
    /// handle, which reports where each byte landed once the text has been
    /// laid out.
    fn interactive_with_layout(
        &self,
        text: &RichText,
        string: &str,
        runs: &[(Range<usize>, InlineStyle)],
        links: &[(Range<usize>, LinkTarget)],
    ) -> (InteractiveText, gpui::TextLayout) {
        let t = self.theme;
        let highlights: Vec<_> = runs
            .iter()
            .map(|(range, style)| (range.clone(), self.highlight(*style)))
            .collect();
        let mono: Vec<_> = runs
            .iter()
            .filter(|(_, s)| s.code || s.kbd || s.html)
            .map(|(range, _)| (range.clone(), t.mono_font.clone()))
            .collect();
        // The selection is a background behind whatever styles the runs
        // already carry, so it is merged into the same highlight list the
        // model's own styles build rather than drawn as a quad of its own.
        let highlights = match self
            .select
            .and_then(|h| h.ranges.get(&text.id).map(|range| (range.clone(), h.color)))
        {
            Some((range, color)) => with_selection(highlights, string, range, color),
            None => highlights,
        };
        let mut styled =
            StyledText::new(SharedString::from(string.to_string())).with_highlights(highlights);
        if !mono.is_empty() {
            styled = styled.with_font_family_overrides(mono);
        }
        let layout = styled.layout().clone();
        let ranges: Vec<_> = links.iter().map(|(r, _)| r.clone()).collect();
        let targets: Vec<LinkTarget> = links.iter().map(|(_, l)| l.clone()).collect();
        let on_link = self.on_link.clone();
        let interactive = InteractiveText::new(("rich", text.id), styled).on_click(
            ranges,
            move |ix, window, cx| {
                if let Some(target) = targets.get(ix).cloned() {
                    on_link(target, window, cx);
                }
            },
        );
        (interactive, layout)
    }

    fn highlight(&self, s: InlineStyle) -> HighlightStyle {
        let t = self.theme;
        let mut h = HighlightStyle::default();
        if s.bold {
            h.font_weight = Some(FontWeight::BOLD);
        }
        if s.italic {
            h.font_style = Some(FontStyle::Italic);
        }
        if s.strike {
            h.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                color: Some(t.text_muted),
            });
            h.color = Some(t.text_muted);
        }
        if s.highlight {
            h.background_color = Some(match s.highlight_color {
                Some(c) => t.highlight_color(c),
                None => t.highlight_bg,
            });
        }
        // `<kbd>` wears inline code's look, the box Obsidian themes it with.
        if s.code || s.kbd {
            h.background_color = Some(t.code_bg);
        }
        // `<u>`: underlined in the text colour; a link's own underline below
        // takes over when both apply.
        if s.underline {
            h.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: None,
                wavy: false,
            });
        }
        // `s.sup` and `s.sub` have no look yet: a gpui text run cannot be
        // raised, lowered or shrunk on its own, so the text sits at body
        // size with the tags dropped.
        if s.math {
            // Only a formula that failed to typeset is still text here.
            h.color = Some(math::error_color());
        }
        if s.html {
            h.color = Some(t.text_muted);
        }
        if s.tag {
            h.color = Some(t.tag);
        }
        if s.footnote {
            h.color = Some(t.link);
        }
        if s.link {
            h.color = Some(t.link);
            h.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(t.link),
                wavy: false,
            });
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::{
        header_height, inline_title, line_distance, ordered, page_distance, runs, same_note,
        selected_ranges, selected_text, shown_title, whole_note, with_selection, word_at,
        ReadingPos, HEADER_TOP, LINE_HEIGHT,
    };
    use crate::reading::model::{build, EmbedSource, Model};
    use gpui::{hsla, HighlightStyle};
    use std::ops::Range;
    use std::path::PathBuf;

    #[test]
    fn a_page_is_the_viewport_less_a_strip_of_overlap() {
        // Tall viewport: the overlap is the 40 px cap.
        assert_eq!(page_distance(800.), 760.);
        // Short viewport: the overlap is an eighth of it, so the page keys
        // still move most of a view rather than a fixed 40 px.
        assert_eq!(page_distance(160.), 140.);
        // The cap and the eighth meet at 320 px.
        assert_eq!(page_distance(320.), 280.);
    }

    #[test]
    fn a_page_never_scrolls_backwards_and_an_unpainted_view_not_at_all() {
        assert_eq!(page_distance(0.), 0.);
        assert_eq!(page_distance(-1.), 0.);
        for viewport in [1., 10., 41., 320., 1000., 4000.] {
            let d = page_distance(viewport);
            assert!(d > 0., "{viewport} scrolled {d}");
            assert!(d <= viewport, "{viewport} scrolled {d}, past a page");
            assert!(
                d >= viewport * 0.875,
                "{viewport} scrolled only {d}, less than seven eighths"
            );
        }
    }

    #[test]
    fn a_line_is_the_rendered_line_height() {
        assert_eq!(line_distance(16.), 16. * LINE_HEIGHT);
        assert_eq!(line_distance(20.), 32.);
        // An arrow key moves less than a page key for every viewport a
        // window can have.
        assert!(line_distance(16.) < page_distance(200.));
    }

    #[test]
    fn only_a_reload_of_the_same_note_is_the_same_note() {
        // The reload the watcher drives on every autosave: the reader's
        // place and their folds are kept.
        assert!(same_note(Some("Notes/A.md"), Some("Notes/A.md")));
        // Another note, or no note: both are dropped.
        assert!(!same_note(Some("Notes/A.md"), Some("Notes/B.md")));
        assert!(!same_note(Some("Notes/A.md"), None));
        assert!(!same_note(None, Some("Notes/A.md")));
        // Clearing the view twice is not a reload of a note.
        assert!(!same_note(None, None));
    }

    #[test]
    fn the_inline_title_is_a_notes_basename_without_its_extension() {
        assert_eq!(inline_title("Folder/Note.md"), Some("Note"));
        assert_eq!(inline_title("Note.md"), Some("Note"));
        // Only the extension comes off: a dotted name keeps its dots.
        assert_eq!(inline_title("Notes/v1.2 release.md"), Some("v1.2 release"));
        assert_eq!(inline_title("Notes/README.MD"), Some("README"));
        // An image opens through the reading view with a synthetic model,
        // and a file no view draws comes through with no path at all:
        // neither is a note, so neither gets a title.
        assert_eq!(inline_title("Attachments/photo.png"), None);
        assert_eq!(inline_title("Board.canvas"), None);
        assert_eq!(inline_title("Paper.pdf"), None);
    }

    #[test]
    fn the_title_shows_only_with_the_setting_on_and_a_note_open() {
        // What `set_note` + `showInlineTitle` come to.
        assert_eq!(shown_title(true, Some("Folder/Note.md")), Some("Note"));
        assert_eq!(shown_title(true, None), None);
        assert_eq!(shown_title(false, Some("Folder/Note.md")), None);
        assert_eq!(shown_title(true, Some("Folder/photo.jpg")), None);
    }

    #[test]
    fn the_header_is_the_top_padding_plus_an_h1_line_and_its_margin() {
        // Without a title the row is only the padding the first block used
        // to carry.
        assert_eq!(header_height(16., false), HEADER_TOP);
        // With one: Obsidian's 25.888 px h1 on a 31.07 px line with a
        // 12.944 px margin below, at the 16 px default.
        let with = header_height(16., true);
        assert!(
            (with - (HEADER_TOP + 31.0656 + 12.944)).abs() < 0.01,
            "{with}"
        );
        // It scales with the body font, as an em-sized heading does.
        assert!(header_height(20., true) > with);
    }

    // ----- selection ------------------------------------------------------------

    /// A vault with nothing in it: the model builder wants a source, and
    /// these notes have no embeds to resolve.
    struct NoVault;

    impl EmbedSource for NoVault {
        fn resolve(&self, _: &str, _: &str) -> Option<String> {
            None
        }
        fn read(&self, _: &str) -> Option<String> {
            None
        }
        fn absolute(&self, path: &str) -> PathBuf {
            PathBuf::from(path)
        }
    }

    fn model(text: &str) -> Model {
        build("Note.md", text, &NoVault)
    }

    /// The position `offset` bytes into the `n`th run of the note.
    fn at(model: &Model, n: usize, offset: usize) -> ReadingPos {
        let runs = runs(model);
        let run = &runs[n];
        ReadingPos {
            block: run.block,
            rich: run.rich,
            offset,
        }
    }

    fn texts(model: &Model) -> Vec<String> {
        runs(model).iter().map(|r| r.text.to_string()).collect()
    }

    #[test]
    fn the_runs_of_a_note_are_its_inline_text_in_document_order() {
        let m = model("# Title\n\nfirst para\n\n- one\n- two\n\n```\ncode\n```\n\n> quoted\n");
        assert_eq!(
            texts(&m),
            ["Title", "first para", "one", "two", "quoted"],
            "a code block carries no inline model, so it cannot be selected"
        );
        // Every run knows the top-level block it was drawn in, and the
        // blocks only ever move forwards.
        let blocks: Vec<usize> = runs(&m).iter().map(|r| r.block).collect();
        assert!(blocks.windows(2).all(|p| p[0] <= p[1]), "{blocks:?}");
        assert_eq!(blocks[0], 0);
        // The two list items share their block and have ids of their own.
        assert_eq!(blocks[2], blocks[3]);
        assert_ne!(runs(&m)[2].rich, runs(&m)[3].rich);
    }

    #[test]
    fn a_callouts_title_comes_before_its_body() {
        let m = model("> [!note] Heads up\n> the body\n");
        assert_eq!(texts(&m), ["Heads up", "the body"]);
    }

    #[test]
    fn an_anchor_below_the_head_is_ordered_for_painting() {
        let m = model("one\n\ntwo\n");
        let r = runs(&m);
        let (first, second) = (at(&m, 0, 2), at(&m, 1, 1));
        assert_eq!(ordered(&r, second, first), Some((first, second)));
        assert_eq!(ordered(&r, first, second), Some((first, second)));
        // Inside one run it is the offsets that order the pair.
        let (low, high) = (at(&m, 0, 1), at(&m, 0, 3));
        assert_eq!(ordered(&r, high, low), Some((low, high)));
        // An end naming a run this note has not got is no selection at
        // all, which is what an anchor left over from another note is.
        let stray = ReadingPos {
            block: 0,
            rich: usize::MAX,
            offset: 0,
        };
        assert_eq!(ordered(&r, stray, first), None);
        assert_eq!(ordered(&r, first, stray), None);
    }

    #[test]
    fn a_selection_across_blocks_copies_a_line_for_each() {
        let m = model("# Heading\n\nfirst para\n\nsecond para\n");
        let r = runs(&m);
        let (from, to) = (at(&m, 0, 3), at(&m, 2, 6));
        assert_eq!(selected_text(&r, from, to), "ding\nfirst para\nsecond");
        // The same selection dragged the other way copies the same text.
        assert_eq!(selected_text(&r, to, from), "ding\nfirst para\nsecond");
        // Three runs are touched: the first and last partly, the middle
        // one whole.
        assert_eq!(
            selected_ranges(&r, from, to),
            vec![(0, 3..7), (1, 0..10), (2, 0..6)]
        );
    }

    #[test]
    fn a_tables_cells_copy_tab_separated_and_its_rows_by_line() {
        let m = model("| a | b |\n| - | - |\n| c | d |\n");
        let r = runs(&m);
        let (from, to) = whole_note(&r).unwrap();
        assert_eq!(selected_text(&r, from, to), "a\tb\nc\td");
    }

    #[test]
    fn select_all_takes_the_whole_note_and_a_note_with_no_text_nothing() {
        let m = model("alpha\n\nbeta\n");
        let r = runs(&m);
        let (from, to) = whole_note(&r).unwrap();
        assert_eq!(from, at(&m, 0, 0));
        assert_eq!(to, at(&m, 1, 4));
        assert_eq!(selected_text(&r, from, to), "alpha\nbeta");
        // A note that is nothing but a code block has no selectable run,
        // so Ctrl+A selects nothing rather than panicking on an end.
        let bare = model("```\ncode\n```\n");
        assert!(runs(&bare).is_empty());
        assert_eq!(whole_note(&runs(&bare)), None);
    }

    #[test]
    fn an_empty_selection_covers_no_runs_and_copies_nothing() {
        let m = model("alpha\n\nbeta\n");
        let r = runs(&m);
        let caret = at(&m, 0, 2);
        assert_eq!(selected_ranges(&r, caret, caret), Vec::new());
        assert_eq!(selected_text(&r, caret, caret), "");
        // Two ends in different runs that still cover no bytes: the end of
        // one run and the start of the next. Nothing is copied, so no
        // stray newline is either.
        assert_eq!(selected_text(&r, at(&m, 0, 5), at(&m, 1, 0)), "");
    }

    #[test]
    fn an_offset_past_a_runs_text_is_clamped_rather_than_slicing_it() {
        let m = model("héllo\n");
        let r = runs(&m);
        // Past the end.
        assert_eq!(selected_text(&r, at(&m, 0, 0), at(&m, 0, 99)), "héllo");
        // Inside the two bytes of the é: the cut walks back to the
        // boundary below it.
        assert_eq!(selected_text(&r, at(&m, 0, 0), at(&m, 0, 2)), "h");
    }

    #[test]
    fn a_double_click_takes_the_word_under_it() {
        assert_eq!(word_at("the quick brown", 4), 4..9);
        assert_eq!(word_at("the quick brown", 7), 4..9);
        // A press at the very end takes the word before it.
        assert_eq!(word_at("the quick brown", 15), 10..15);
        // Spaces and punctuation are a run of their own, so a press always
        // takes something.
        assert_eq!(word_at("a, b", 1), 1..3);
        assert_eq!(word_at("", 0), 0..0);
        // An apostrophe is part of the word, as a browser's word select
        // treats one.
        assert_eq!(word_at("don't stop", 2), 0..5);
    }

    #[test]
    fn the_selection_wash_splits_the_style_runs_it_overlaps() {
        let bold = HighlightStyle {
            font_weight: Some(gpui::FontWeight::BOLD),
            ..Default::default()
        };
        let wash = hsla(0.5, 0.5, 0.5, 0.3);
        // "plain bold plain" with the bold run at 6..10, selected from
        // inside it to the middle of the plain tail.
        let out = with_selection(vec![(6..10, bold)], "plain bold plain", 8..14, wash);
        let ranges: Vec<Range<usize>> = out.iter().map(|(r, _)| r.clone()).collect();
        assert_eq!(ranges, vec![6..8, 8..10, 10..14]);
        assert_eq!(out[0].1.background_color, None, "outside the selection");
        assert_eq!(out[1].1.background_color, Some(wash));
        assert_eq!(
            out[1].1.font_weight, bold.font_weight,
            "the wash is behind the style the model gave the run"
        );
        assert_eq!(out[2].1.background_color, Some(wash));
        // Sorted and non-overlapping, which is what
        // `StyledText::with_highlights` needs of its list.
        assert!(out.windows(2).all(|p| p[0].0.end <= p[1].0.start));
        // An empty selection leaves the runs exactly as they were.
        assert_eq!(
            with_selection(vec![(6..10, bold)], "plain bold plain", 3..3, wash),
            vec![(6..10, bold)]
        );
    }
}
