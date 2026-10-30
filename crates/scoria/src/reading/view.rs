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
use std::collections::HashSet;
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
    ]
);

/// Binds the reading view's scrolling keys in its own context; call once at
/// startup. Obsidian's reading view is a DOM scroller with no commands of its
/// own, so this is the browser's set of page-scrolling keys rather than a
/// probed Obsidian binding: the page keys and Space for a page, the arrows
/// for a line, Home and End (plain and with Ctrl, which a document scroller
/// treats alike) for the ends.
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
        cx.notify();
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
        .with_reading_host(&host);
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
            on_link,
            on_toggle,
            text_system,
            available: available.max(100.),
        }
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
            return self
                .interactive(text, &text.text, &text.runs, &text.links)
                .into_any_element();
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
        Overlay::new(interactive.into_any_element(), layout, inlines, em, t.text).into_any_element()
    }

    fn interactive(
        &self,
        text: &RichText,
        string: &str,
        runs: &[(Range<usize>, InlineStyle)],
        links: &[(Range<usize>, LinkTarget)],
    ) -> InteractiveText {
        self.interactive_with_layout(text, string, runs, links).0
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
        header_height, inline_title, line_distance, page_distance, same_note, shown_title,
        HEADER_TOP, LINE_HEIGHT,
    };

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
}
