//! The element that draws a `.pdf` open in a leaf, and the state behind it.
//!
//! The view is a toolbar over a column of pages, with a sidebar of
//! thumbnails or the table of contents on the left when it is open and a
//! find bar between the toolbar and the pages when that is. The pages are
//! laid out from their sizes alone — every page's box is exact before any
//! page is rendered — so the scroll extent is right from the first frame,
//! and only the pages in or near the viewport ask the [`documents`] cache
//! for an image; the rest are empty white boxes. Nothing here reads the
//! file: the cache does, on the background executor, and the view asks it
//! again every frame until the answer is there.
//!
//! What it draws is **observed**
//! (`tools/obsidian-probe/results/2026-10-26-pdf.json`, Obsidian 1.14.5,
//! written up in
//! [[Specification#Observed PDF view behavior, Obsidian 1.14.x]]): the
//! toolbar's six controls and their order, icons, sizes and spacing; the
//! two menus and their rows; pages stacked with no gap but their own 4 px
//! transparent border, centred, ringed by `--pdf-shadow`; the 140 px
//! sidebar showing thumbnails by default; the 42 px find bar and its
//! "1 of 3 matches"; `page-width` as the zoom on open, with the zoom
//! buttons stepping as pdf.js does; the leaf recording nothing but the
//! file; Left and Right as the page keys and the browser's own scrolling
//! keys; and a file that will not open showing the toolbar over nothing,
//! "of 0".
//!
//! The zoom vocabulary is pdf.js's, which is what the viewer is built on:
//! `page-width`, `page-fit` and a number, where `1` (100 %) is
//! [`CSS_UNITS`] logical pixels per point — pdf.js lays a page out at 96
//! dpi — and a step in or out multiplies by [`SCALE_DELTA`] and rounds to
//! a tenth, between [`MIN_SCALE`] and [`MAX_SCALE`]. The current page is
//! pdf.js's too: the page showing the most of itself, by area, unless the
//! page the view was on is still wholly in view.
//!
//! The view scrolls itself: the column sits at `-scroll` inside an
//! `overflow_hidden` box and the wheel, the keys and the page input move
//! it. That keeps the layout exact under a zoom change (the page under the
//! pointer, or at the top, stays put) and lets the socket read the scroll
//! back as plain numbers. Where the view was in a file is kept for the
//! session, per file, so a tab that comes back to it comes back to the
//! same page — Obsidian's leaf stays alive behind an inactive tab, and its
//! `workspace.json` never records a page, zoom or scroll. What was not
//! observed is marked `OBSERVE:` where the guess is made.

use crate::assets::icon;
use crate::canvas::menu::tooltip;
use crate::pdf::selection::{self, TextPos};
use crate::pdf::{
    adapt_to_theme, documents, page_source, theming, toggle_adapt_to_theme, DocState, Loaded,
};
use crate::search::{menu, menu_item};
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER, ICON_SIZE};
use gpui::{
    actions, anchored, canvas, deferred, div, img, point, prelude::*, px, Anchor, AnyElement, App,
    Bounds, BoxShadow, ClickEvent, ClipboardItem, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Hsla, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ObjectFit, Pixels, Point, Render, ScrollHandle, ScrollWheelEvent, SharedString,
    Size, Subscription, Task, Window,
};
use scoria_pdf::{OutlineItem, PageSize, Selection, TextItem, Theming};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// ---- pdf.js's constants ---------------------------------------------------------

/// Logical pixels per point at 100 %: pdf.js lays pages out at 96 dpi.
/// Observed: `page-actual` draws A4 at 801.65 = 595.28 × 4/3 + 8.
pub const CSS_UNITS: f32 = 96.0 / 72.0;
/// One zoom step, `DEFAULT_SCALE_DELTA`. Observed: Zoom in from
/// `page-width` at 0.791 goes 0.9, 1, 1.1, 1.3, 1.5, 1.7 and Zoom out 0.7,
/// 0.6 … 0.1, which is this step rounded to tenths each time.
pub const SCALE_DELTA: f32 = 1.1;
pub const MIN_SCALE: f32 = 0.1;
pub const MAX_SCALE: f32 = 10.0;
/// What fit-width leaves beside the page, `SCROLLBAR_PADDING`. Observed:
/// a 668 px viewer fits an A4 page at 0.7912 = (668 - 40) / (595.28 pt ×
/// 96/72), to the digit.
const SCROLLBAR_PADDING: f32 = 40.0;
/// What fit-height leaves above and below the page. pdf.js's own
/// `VERTICAL_PADDING` is 5, but the one observation does not bear it out:
/// `page-fit` in a viewer reporting 683 px tall came to 0.5844, which is
/// (683 - 27) / (841.89 × 96/72). OBSERVE: whether the 22 px difference is
/// a bar the probe's `clientHeight` did not see or Obsidian's own padding;
/// the observed value is kept until a second measurement says.
const VERTICAL_PADDING: f32 = 27.0;

// ---- the observed chrome ---------------------------------------------------------

/// Each page's own transparent border: the only space between pages (the
/// gap between two `.page`s is 0) and above the first one.
const PAGE_BORDER: f32 = 4.0;
/// The toolbar: 4 px above and below a 30 px page box, 12 px at the ends.
const TOOLBAR_HEIGHT: f32 = 38.0;
const TOOLBAR_PAD_X: f32 = 12.0;
/// A `clickable-icon` in it: a 16 px icon padded 4 by 6.
const CONTROL_WIDTH: f32 = 28.0;
const CONTROL_HEIGHT: f32 = 24.0;
/// Between two controls.
const CONTROL_GAP: f32 = 4.0;
/// A `pdf-toolbar-spacer`.
const SPACER_WIDTH: f32 = 12.0;
/// A `pdf-toolbar-divider`: 1 by 16.
const DIVIDER_HEIGHT: f32 = 16.0;
/// The page box, `input.pdf-page-input`: 52 by 30, 13 px text.
const PAGE_INPUT_WIDTH: f32 = 52.0;
/// The sidebar, `pdf-sidebar-container`.
const SIDEBAR_WIDTH: f32 = 140.0;
/// The find bar, `--findbar-height`: 6 px around a 30 px field.
const FIND_BAR_HEIGHT: f32 = 42.0;
/// The find field, `input[type=search]`, placeholder "Search...".
const FIND_INPUT_WIDTH: f32 = 300.0;
/// A thumbnail's image is 98 px wide in an 8 px border with a 4 px radius;
/// the column is padded 12 with 4 between.
const THUMBNAIL_WIDTH: f32 = 98.0;
const THUMBNAIL_BORDER: f32 = 8.0;
const THUMBNAIL_GAP: f32 = 4.0;
const THUMBNAIL_PAD: f32 = 12.0;
/// The table of contents' padding.
const OUTLINE_PAD: f32 = 8.0;
/// How far Up and Down scroll: the browser's own arrow step on the focused
/// viewer (observed: 40).
const SCROLL_LINE: f32 = 40.0;
/// How far Page Down and Space scroll, as a share of the viewport: the
/// browser's own (observed: 566 of a 647 px viewer).
const SCROLL_PAGE: f32 = 0.875;
/// Wheel pixels per zoom step under Ctrl. Observed: 100 px of wheel is
/// three steps either way.
const WHEEL_STEP: f32 = 33.0;
/// How long the page, scroll and zoom have to stay put before they are
/// remembered for the file.
const VIEW_STATE_DEBOUNCE: Duration = Duration::from_millis(300);
/// The zoom on open: observed `page-width`.
const DEFAULT_ZOOM: Zoom = Zoom::FitWidth;
/// How far below the viewport's top a followed selection's first line
/// lands. Observed: 36 px at scale 2.90 and 2.52 on the typst file, 41 on
/// the hand-written one; pdf.js's `FitBH` destination at the item's top,
/// less what the run did not isolate.
const FOLLOW_MARGIN: f32 = 37.0;
/// A `#selection=` link's mark: `.textLayer .mod-focused`, `#ffd000` with
/// a 4 px pad and radius, under the text layer's 0.2 opacity.
const FOLLOW_MARK: u32 = 0xffd000;
const FOLLOW_PAD: f32 = 4.0;
/// The text layer's opacity, which every mark and the selection's
/// `--color-accent` are drawn through.
const TEXT_LAYER_OPACITY: f32 = 0.2;

// ---- actions --------------------------------------------------------------------

actions!(
    pdf_view,
    [
        ScrollUp,
        ScrollDown,
        PageUp,
        PageDown,
        ScrollToTop,
        ScrollToBottom,
        NextPage,
        PreviousPage,
        Find,
        FindNext,
        FindPrevious,
        Dismiss,
        CopySelection
    ]
);

/// Key bindings for the view; called once at startup. The context is the
/// view's own, beneath the workspace's, so the app's hotkeys still reach
/// the workspace for anything not bound here — Ctrl+=, Ctrl+- and Ctrl+0
/// in particular, which Obsidian leaves to the window's own zoom
/// (observed: they change `zoomFactor`, never the PDF's scale).
///
/// Observed with the viewer focused: Left and Right go a page (the view's
/// scope registers Left, Right and Escape, nothing else); Up and Down
/// scroll 40 px, Page Down and Space most of a viewport, Home and End to
/// the ends; j, k, n, p and r do nothing; Ctrl+F opens the find bar and
/// Escape closes it. The plain keys are bound only while no field in the
/// view has the keyboard.
pub fn bind_keys(cx: &mut App) {
    let view = Some("PdfView");
    let keys = Some("PdfView && !TextInput && !PdfPassword");
    cx.bind_keys([
        KeyBinding::new("left", PreviousPage, keys),
        KeyBinding::new("right", NextPage, keys),
        KeyBinding::new("up", ScrollUp, keys),
        KeyBinding::new("down", ScrollDown, keys),
        KeyBinding::new("pageup", PageUp, keys),
        KeyBinding::new("pagedown", PageDown, keys),
        KeyBinding::new("shift-space", PageUp, keys),
        KeyBinding::new("space", PageDown, keys),
        KeyBinding::new("home", ScrollToTop, keys),
        KeyBinding::new("end", ScrollToBottom, keys),
        KeyBinding::new("ctrl-f", Find, view),
        KeyBinding::new("f3", FindNext, view),
        KeyBinding::new("shift-f3", FindPrevious, view),
        KeyBinding::new("shift-enter", FindPrevious, view),
        KeyBinding::new("escape", Dismiss, view),
        // Observed: Ctrl+C copies the selection as "Copy" does; Ctrl+A
        // selects nothing, so it is not bound.
        KeyBinding::new("ctrl-c", CopySelection, keys),
    ]);
}

/// What the view asks the workspace to do; it owns no vault of its own.
pub enum PdfEvent {
    /// Something the view has to say, as a Notice.
    Report(String),
    /// "Copy link to selection" (`quote: None`) or "Copy as quote" (the
    /// selection's text): the workspace writes the link to the file it
    /// shows, `[[<link path>#<subpath>|<alias>]]`, since the link path
    /// depends on the vault.
    CopyLink {
        subpath: String,
        alias: String,
        quote: Option<String>,
    },
}

/// The Display options menu's spreads, pdf.js's `spreadMode`: one page to
/// a row, or two with the odd pages on the left, or two with page 1 alone
/// and the even pages on the left (observed: each row centred, page 1
/// alone and centred in the even mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spread {
    #[default]
    None,
    Odd,
    Even,
}

impl Spread {
    /// pdf.js's number, which the session's memory keeps.
    pub fn mode(self) -> u64 {
        match self {
            Spread::None => 0,
            Spread::Odd => 1,
            Spread::Even => 2,
        }
    }

    pub fn from_mode(mode: u64) -> Spread {
        match mode {
            1 => Spread::Odd,
            2 => Spread::Even,
            _ => Spread::None,
        }
    }

    /// The pages of each row, in order.
    pub fn rows(self, count: usize) -> Vec<Vec<usize>> {
        let mut rows = Vec::new();
        let mut ix = 0;
        if self == Spread::Even && count > 0 {
            rows.push(vec![0]);
            ix = 1;
        }
        let per = if self == Spread::None { 1 } else { 2 };
        while ix < count {
            rows.push((ix..(ix + per).min(count)).collect());
            ix += per;
        }
        rows
    }
}

/// How the pages are scaled, in pdf.js's vocabulary: the Display options
/// menu offers *Fit width* and *Fit height*, and Zoom in and out step a
/// number from wherever the view is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zoom {
    /// `page-width`: the widest page fills the viewport.
    FitWidth,
    /// `page-fit`, "Fit height": the tallest page fits the viewport.
    FitHeight,
    /// A number, where `1.0` is 100 %.
    Percent(f32),
}

impl Zoom {
    /// pdf.js's value, for the session's memory and the socket.
    pub fn to_json(self) -> Value {
        match self {
            Zoom::FitWidth => json!("page-width"),
            Zoom::FitHeight => json!("page-fit"),
            Zoom::Percent(s) => json!(f64::from(s)),
        }
    }

    pub fn from_json(value: &Value) -> Option<Zoom> {
        match value {
            Value::String(s) => match s.as_str() {
                "page-width" => Some(Zoom::FitWidth),
                "page-fit" => Some(Zoom::FitHeight),
                other => other.parse::<f32>().ok().map(Zoom::Percent),
            },
            Value::Number(n) => n
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0)
                .map(|n| Zoom::Percent(n as f32)),
            _ => None,
        }
    }

    /// The mode's name for the socket.
    pub fn mode(self) -> &'static str {
        match self {
            Zoom::FitWidth => "page-width",
            Zoom::FitHeight => "page-fit",
            Zoom::Percent(_) => "custom",
        }
    }

    /// The pdf.js scale this zoom resolves to in a viewport of `viewport`
    /// logical pixels, fitting page `page` of `sizes`: pdf.js fits the page
    /// current when the zoom is set, not the widest (observed: fit-width
    /// over the same file is 0.791 set on an A4 page and 0.559 set on its
    /// landscape page). The fit modes need the viewport, and answer `1.0`
    /// until there is one.
    pub fn resolve(
        self,
        sizes: &[PageSize],
        page: usize,
        viewport: Size<f32>,
        spread: Spread,
    ) -> f32 {
        let (width, height) = sizes
            .get(page)
            .or(sizes.first())
            .map_or((0.0, 0.0), |s| (s.width, s.height));
        let can_fit = viewport.width > 0.0 && viewport.height > 0.0 && width > 0.0 && height > 0.0;
        // In a spread the width fits two of the page (observed: 0.791 to
        // 0.396 at the same width, pages 318 by 448).
        let across = if spread == Spread::None { 1.0 } else { 2.0 };
        let fit_width =
            || (viewport.width - SCROLLBAR_PADDING).max(1.0) / (width * CSS_UNITS * across);
        let fit_height = || (viewport.height - VERTICAL_PADDING).max(1.0) / (height * CSS_UNITS);
        let scale = match self {
            Zoom::Percent(s) => s,
            _ if !can_fit => 1.0,
            Zoom::FitWidth => fit_width(),
            Zoom::FitHeight => fit_width().min(fit_height()),
        };
        scale.clamp(MIN_SCALE, MAX_SCALE)
    }
}

/// One zoom step in, as pdf.js takes it: times [`SCALE_DELTA`], up to the
/// next tenth.
pub fn step_in(scale: f32) -> f32 {
    let next = (scale * SCALE_DELTA * 100.0).round() / 100.0;
    ((next * 10.0).ceil() / 10.0).min(MAX_SCALE)
}

/// One zoom step out: over [`SCALE_DELTA`], down to the previous tenth.
pub fn step_out(scale: f32) -> f32 {
    let next = (scale / SCALE_DELTA * 100.0).round() / 100.0;
    ((next * 10.0).floor() / 10.0).max(MIN_SCALE)
}

/// Where every page sits, in logical pixels from the top-left of the
/// column, at one scale.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Logical pixels per point.
    pub scale: f32,
    /// Each page's image, inside its [`PAGE_BORDER`].
    pub pages: Vec<Bounds<f32>>,
    /// The scroll extent: the viewport's width or the widest page's.
    pub content: Size<f32>,
}

impl Layout {
    /// Lays `sizes` out in a column of rows, the borders touching: one page
    /// to a row, or two in a spread. Each row is centred in the viewport's
    /// width on its own, and a row wider than the viewport sits flush left
    /// and overflows (observed: the landscape page at the viewer's left
    /// edge while the A4 pages centre); a page shorter than its row is
    /// centred in it (observed: the landscape page beside a portrait one).
    pub fn new(sizes: &[PageSize], scale: f32, viewport_width: f32, spread: Spread) -> Layout {
        let mut pages = vec![Bounds::default(); sizes.len()];
        let mut width = viewport_width;
        let mut y = 0.0;
        for row in spread.rows(sizes.len()) {
            let row_w: f32 = row
                .iter()
                .map(|&ix| sizes[ix].width * scale + 2.0 * PAGE_BORDER)
                .sum();
            let row_h = row
                .iter()
                .map(|&ix| sizes[ix].height * scale)
                .fold(0.0, f32::max);
            width = width.max(row_w);
            let mut x = ((viewport_width - row_w) / 2.0).max(0.0);
            for &ix in &row {
                let (w, h) = (sizes[ix].width * scale, sizes[ix].height * scale);
                pages[ix] = Bounds::new(
                    Point::new(x + PAGE_BORDER, y + PAGE_BORDER + (row_h - h) / 2.0),
                    Size::new(w, h),
                );
                x += w + 2.0 * PAGE_BORDER;
            }
            y += row_h + 2.0 * PAGE_BORDER;
        }
        Layout {
            scale,
            pages,
            content: Size::new(width, y),
        }
    }

    /// The page nearest the point `(x, y)` of the column, and the point in
    /// that page's own points.
    pub fn page_point(&self, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        let mut best: Option<(f32, usize)> = None;
        for (ix, b) in self.pages.iter().enumerate() {
            let dx = (b.left() - x).max(x - b.right()).max(0.0);
            let dy = (b.top() - y).max(y - b.bottom()).max(0.0);
            let d = dx * dx + dy * dy;
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, ix));
            }
        }
        let (_, ix) = best?;
        let b = self.pages[ix];
        if self.scale <= 0.0 {
            return None;
        }
        Some((ix, (x - b.left()) / self.scale, (y - b.top()) / self.scale))
    }

    /// The page the viewport is on, as pdf.js decides it: of the pages in
    /// view, the one showing the largest share of its own area (the first,
    /// on a tie) — unless `current` is still wholly in view, in which case
    /// it stays. Observed: with a landscape page half scrolled past and
    /// clipped at the sides, the portrait page under it wins.
    pub fn current_page(&self, scroll: Point<f32>, viewport: Size<f32>, current: usize) -> usize {
        let (top, bottom) = (scroll.y, scroll.y + viewport.height);
        let (left, right) = (scroll.x, scroll.x + viewport.width);
        let mut best: Option<(usize, f32)> = None;
        let mut current_whole = false;
        for (ix, b) in self.pages.iter().enumerate() {
            if b.bottom() <= top || b.top() >= bottom || b.size.height <= 0.0 || b.size.width <= 0.0
            {
                continue;
            }
            let visible_h = (b.bottom().min(bottom) - b.top().max(top)).max(0.0);
            let visible_w = (b.right().min(right) - b.left().max(left)).max(0.0);
            let percent = (visible_h * visible_w * 100.0 / (b.size.height * b.size.width)).floor();
            if ix == current && percent >= 100.0 {
                current_whole = true;
            }
            if best.is_none_or(|(_, p)| percent > p) {
                best = Some((ix, percent));
            }
        }
        if current_whole {
            return current;
        }
        best.map_or(current, |(ix, _)| ix)
    }

    /// The page at height `y` and how far down it `y` is, as a fraction of
    /// its height (negative in the border above it). The anchor a zoom
    /// change keeps still.
    pub fn locate(&self, y: f32) -> (usize, f32) {
        let ix = self
            .pages
            .iter()
            .rposition(|b| b.top() - PAGE_BORDER <= y)
            .unwrap_or(0);
        match self.pages.get(ix) {
            Some(b) if b.size.height > 0.0 => (ix, (y - b.top()) / b.size.height),
            _ => (ix, 0.0),
        }
    }

    /// The scroll top that puts `page`'s border at the top of the viewport,
    /// which is where a link to it lands (observed: the page's content 4 px
    /// below the viewport's top).
    pub fn top_of(&self, page: usize) -> f32 {
        self.pages
            .get(page)
            .map_or(0.0, |b| (b.top() - PAGE_BORDER).max(0.0))
    }
}

/// Where to scroll once the layout is known.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Target {
    /// This page at the top of the viewport.
    Page(usize),
    /// Keep the point `frac` of the way down `page` at `offset` pixels
    /// below the viewport's top: a zoom change's anchor. A negative `frac`
    /// is in the border above the page, and is kept as pixels rather than
    /// scaled with the page.
    Anchor { page: usize, frac: f32, offset: f32 },
    /// A link's `#page=N&selection=a,b,c,d`: the selection marked and its
    /// first line near the top, once the page's text is read.
    Selection { page: usize, selection: Selection },
}

/// Which menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    None,
    /// "Sidebar options", under its chevron.
    Sidebar,
    /// "Display options", under its chevron.
    Display,
    /// The selection's menu, where it was asked for.
    Selection,
}

/// What the sidebar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    /// Observed default.
    Thumbnails,
    /// "Table of contents".
    Outline,
}

/// The find bar's state.
struct FindState {
    query: String,
    /// Every match, as the page it is on, in document order.
    matches: Vec<usize>,
    /// Which of `matches` the view is on.
    current: usize,
    searching: bool,
    case_sensitive: bool,
    _task: Option<Task<()>>,
}

pub struct PdfView {
    focus: FocusHandle,
    path: Option<String>,
    absolute: Option<PathBuf>,
    /// The parsed file, once the cache has it.
    loaded: Option<Arc<Loaded>>,
    error: Option<String>,
    zoom: Zoom,
    /// The page a fit zoom was set on, which is the one it fits.
    fit_page: usize,
    /// What `zoom` resolved to at the last layout, as a pdf.js scale.
    resolved: f32,
    layout: Option<Layout>,
    /// The page the viewport is on, zero-based.
    page: usize,
    /// The column's offset under the viewport, in logical pixels.
    scroll: Point<f32>,
    /// The pages area, as last measured.
    viewport: Size<f32>,
    /// Its origin in the window, for zooming about the pointer.
    origin: Point<f32>,
    /// The toolbar's origin in the window, for hanging its menus.
    toolbar_origin: Point<f32>,
    pending: Option<Target>,
    sidebar: bool,
    sidebar_mode: SidebarMode,
    sidebar_scroll: ScrollHandle,
    /// The page the sidebar last followed, so it scrolls once per change.
    sidebar_followed: Option<usize>,
    /// Outline items whose children are shown; every item starts collapsed,
    /// as observed.
    expanded: HashSet<usize>,
    menu: Menu,
    page_input: Entity<TextInput>,
    find_input: Entity<TextInput>,
    find: Option<FindState>,
    /// Where the view was in each file it showed this session, by
    /// vault-relative path, in the shape of [`PdfView::view_state_json`].
    positions: HashMap<String, Value>,
    /// Pages with an image this frame, pages whose image the cache holds,
    /// pages intersecting the viewport.
    requested: usize,
    rendered: usize,
    visible: usize,
    /// Wheel pixels under Ctrl since the last zoom step.
    wheel_accum: f32,
    /// Take the keyboard back from a field on the next frame.
    refocus: bool,
    settle_task: Option<Task<()>>,
    spread: Spread,
    /// The selection, anchor then head, in either order.
    selection: Option<(TextPos, TextPos)>,
    /// A drag is selecting.
    selecting: bool,
    /// What a `#selection=` link marked, start then end.
    highlight: Option<(TextPos, TextPos)>,
    /// Where the selection's menu was asked for, in the window.
    menu_at: Point<f32>,
    /// The file wants a password: `Some(wrong)` once asked, `wrong` after a
    /// refused try.
    locked: Option<bool>,
    password: String,
    password_focus: FocusHandle,
    /// Focus the password field on the next frame.
    focus_password: bool,
    /// Whether it has the keyboard this frame, for its ring and caret.
    focus_password_now: bool,
    _subscriptions: Vec<Subscription>,
}

impl PdfView {
    pub fn new(cx: &mut Context<Self>) -> PdfView {
        let page_input = cx.new(|cx| {
            let mut input = TextInput::new("", cx);
            input.set_pad_left(px(8.));
            input.set_pad_right(px(8.));
            input
        });
        let find_input = cx.new(|cx| {
            let mut input = TextInput::new("Search...", cx);
            input.set_pad_left(px(12.));
            input.set_pad_right(px(34.));
            input
        });
        let subscriptions = vec![
            cx.subscribe(
                &page_input,
                |this, input, event: &InputEvent, cx| match event {
                    // Observed: Enter on a number in range goes there; 0, a
                    // number past the end and letters do nothing, and the box
                    // shows the page again.
                    InputEvent::Submit => {
                        let text = input.read(cx).text().trim().to_string();
                        if let Ok(n) = text.parse::<usize>() {
                            if n >= 1 {
                                this.go_to_page(n - 1, false, cx);
                            }
                        }
                        this.refocus = true;
                        cx.notify();
                    }
                    InputEvent::Cancel => {
                        this.refocus = true;
                        cx.notify();
                    }
                    InputEvent::Changed | InputEvent::Up | InputEvent::Down => {}
                },
            ),
            cx.subscribe(
                &find_input,
                |this, input, event: &InputEvent, cx| match event {
                    InputEvent::Submit => {
                        let text = input.read(cx).text().to_string();
                        this.search(text, true, cx);
                    }
                    InputEvent::Cancel => this.close_find(cx),
                    InputEvent::Changed | InputEvent::Up | InputEvent::Down => {}
                },
            ),
        ];
        PdfView {
            focus: cx.focus_handle(),
            path: None,
            absolute: None,
            loaded: None,
            error: None,
            zoom: DEFAULT_ZOOM,
            fit_page: 0,
            resolved: 1.0,
            layout: None,
            page: 0,
            scroll: Point::default(),
            viewport: Size::default(),
            origin: Point::default(),
            toolbar_origin: Point::default(),
            pending: None,
            sidebar: false,
            sidebar_mode: SidebarMode::Thumbnails,
            sidebar_scroll: ScrollHandle::new(),
            sidebar_followed: None,
            expanded: HashSet::new(),
            menu: Menu::None,
            page_input,
            find_input,
            find: None,
            positions: HashMap::new(),
            requested: 0,
            rendered: 0,
            visible: 0,
            wheel_accum: 0.0,
            refocus: false,
            settle_task: None,
            spread: Spread::None,
            selection: None,
            selecting: false,
            highlight: None,
            menu_at: Point::default(),
            locked: None,
            password: String::new(),
            password_focus: cx.focus_handle(),
            focus_password: false,
            focus_password_now: false,
            _subscriptions: subscriptions,
        }
    }

    /// Shows the file at `path` (vault-relative) and `absolute`. The cache
    /// is asked for it on the next frame; until then the toolbar is drawn
    /// over nothing. A file shown earlier this session comes back to the
    /// page and zoom it was left at.
    pub fn load(&mut self, path: &str, absolute: PathBuf, cx: &mut Context<Self>) {
        self.path = Some(path.to_string());
        self.absolute = Some(absolute);
        self.loaded = None;
        self.error = None;
        self.layout = None;
        self.zoom = DEFAULT_ZOOM;
        self.page = 0;
        self.scroll = Point::default();
        self.pending = None;
        self.menu = Menu::None;
        self.find = None;
        self.expanded.clear();
        self.sidebar_followed = None;
        self.settle_task = None;
        self.spread = Spread::None;
        self.selection = None;
        self.selecting = false;
        self.highlight = None;
        self.locked = None;
        self.password.clear();
        if let Some(state) = self.positions.get(path).cloned() {
            self.apply_view_state(&state);
        }
        self.fit_page = self.page;
        cx.notify();
    }

    /// What the session remembered, in the shape [`PdfView::view_state_json`]
    /// writes: `page` one-based as a link's `page=` is, `zoom` pdf.js's
    /// value.
    fn apply_view_state(&mut self, state: &Value) {
        if let Some(zoom) = state.get("zoom").and_then(Zoom::from_json) {
            self.zoom = zoom;
        }
        if let Some(mode) = state.get("spread").and_then(Value::as_u64) {
            self.spread = Spread::from_mode(mode);
        }
        if let Some(page) = state.get("page").and_then(Value::as_u64) {
            if page >= 1 {
                self.page = (page - 1) as usize;
                self.pending = Some(Target::Page(self.page));
            }
        }
    }

    /// The file changed underneath the view: forget what was read and ask
    /// again, keeping the page and the zoom.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(absolute) = self.absolute.clone() else {
            return;
        };
        documents(cx).invalidate(&absolute, cx);
        self.loaded = None;
        self.error = None;
        self.locked = None;
        self.layout = None;
        self.selection = None;
        self.highlight = None;
        self.pending = Some(Target::Page(self.page));
        if let Some(find) = self.find.as_mut() {
            find.matches.clear();
            find.current = 0;
        }
        cx.notify();
    }

    /// Lets go of the file, as the canvas does when another kind opens.
    /// Where the view was in it is remembered for the session.
    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.remember();
        self.path = None;
        self.absolute = None;
        self.loaded = None;
        self.error = None;
        self.layout = None;
        self.pending = None;
        self.find = None;
        self.menu = Menu::None;
        self.settle_task = None;
        self.selection = None;
        self.highlight = None;
        self.locked = None;
        cx.notify();
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// The socket's name for whichever of the view's fields has the
    /// keyboard: the page box or the find field.
    pub fn focused_field(&self, window: &Window, cx: &App) -> Option<&'static str> {
        if self.page_input.focus_handle(cx).is_focused(window) {
            Some("pdf-page-input")
        } else if self.find_input.focus_handle(cx).is_focused(window) {
            Some("pdf-find-input")
        } else {
            None
        }
    }

    /// Scrolls `page` (zero-based) to the top of the viewport, now or once
    /// the file is laid out. A page past the end is ignored when `clamp` is
    /// off, which is what a link to one and the page box do (observed:
    /// `#page=99` and typing 99 leave the view where it is), and the last
    /// page when it is on.
    pub fn go_to_page(&mut self, page: usize, clamp: bool, cx: &mut Context<Self>) {
        let count = self.loaded.as_ref().map(|l| l.sizes.len());
        let page = match count {
            Some(n) if n > 0 && page >= n => {
                if clamp {
                    n - 1
                } else {
                    return;
                }
            }
            _ => page,
        };
        self.page = page;
        self.pending = Some(Target::Page(page));
        self.settle(cx);
        cx.notify();
    }

    /// Follows a link's `#page=N&selection=a,b,c,d`: page `page`
    /// (zero-based), with the selection marked and its first line a little
    /// below the viewport's top, once the page's text is read. A page past
    /// the end leaves the view where it is, as `#page=` does; a selection
    /// naming items the page does not have marks nothing (observed).
    pub fn go_to_selection(&mut self, page: usize, selection: Selection, cx: &mut Context<Self>) {
        if let Some(n) = self.loaded.as_ref().map(|l| l.sizes.len()) {
            if page >= n {
                return;
            }
        }
        self.page = page;
        self.highlight = None;
        self.selection = None;
        self.pending = Some(Target::Selection { page, selection });
        self.settle(cx);
        cx.notify();
    }

    /// Sets the zoom, keeping the point `offset` pixels below the viewport's
    /// top where it is.
    pub fn set_zoom(&mut self, zoom: Zoom, offset: f32, cx: &mut Context<Self>) {
        if let Some(layout) = &self.layout {
            let (page, frac) = layout.locate(self.scroll.y + offset);
            self.pending = Some(Target::Anchor { page, frac, offset });
        }
        self.zoom = zoom;
        self.fit_page = self.page;
        self.menu = Menu::None;
        self.settle(cx);
        cx.notify();
    }

    /// Sets the spread from the Display options menu, keeping the page at
    /// the top; a fit zoom refits (observed: fit width halves in a spread).
    pub fn set_spread(&mut self, spread: Spread, cx: &mut Context<Self>) {
        if self.spread == spread {
            return;
        }
        self.spread = spread;
        self.fit_page = self.page;
        self.pending = Some(Target::Page(self.page));
        self.settle(cx);
        cx.notify();
    }

    /// Scrolls by `delta` logical pixels, clamped to the column.
    fn scroll_by(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        self.scroll.x += dx;
        self.scroll.y += dy;
        self.pending = None;
        self.clamp_scroll();
        self.settle(cx);
        cx.notify();
    }

    fn clamp_scroll(&mut self) {
        let Some(layout) = &self.layout else {
            return;
        };
        let max_x = (layout.content.width - self.viewport.width).max(0.0);
        let max_y = (layout.content.height - self.viewport.height).max(0.0);
        self.scroll.x = self.scroll.x.clamp(0.0, max_x);
        self.scroll.y = self.scroll.y.clamp(0.0, max_y);
    }

    /// Starts the debounce after which the file's position is remembered,
    /// the way the workspace's autosave waits for the typing to stop.
    fn settle(&mut self, cx: &mut Context<Self>) {
        self.settle_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(VIEW_STATE_DEBOUNCE).await;
            this.update(cx, |view, _| {
                view.settle_task = None;
                view.remember();
            })
            .ok();
        }));
    }

    /// Keeps where the view is in the open file, for the session.
    fn remember(&mut self) {
        if let Some(path) = self.path.clone() {
            if self.loaded.is_some() {
                self.positions.insert(path, self.view_state_json());
            }
        }
    }

    /// Where the view is: the page (one-based), the scroll and the zoom as
    /// pdf.js names it. Kept for the session only; none of it reaches the
    /// leaf on disk (observed: `state.state` is `{"file": ..}` before and
    /// after scrolling, zooming and a quit).
    pub fn view_state_json(&self) -> Value {
        json!({
            "page": self.page + 1,
            "left": json_number(self.scroll.x),
            "top": json_number(self.scroll.y),
            "zoom": self.zoom.to_json(),
            "spread": self.spread.mode(),
        })
    }

    /// What the control socket reports under `pdf`.
    pub fn state_json(&self, cx: &App) -> Value {
        let find = self.find.as_ref().map(|f| {
            json!({
                "query": f.query,
                "matches": f.matches.len(),
                "pages": f.matches.iter().map(|p| p + 1).collect::<Vec<_>>(),
                "current": (!f.matches.is_empty()).then_some(f.current + 1),
                "searching": f.searching,
                "case_sensitive": f.case_sensitive,
            })
        });
        let outline = self.loaded.as_ref().map_or(0, |l| l.outline.len());
        json!({
            "path": self.path,
            "loaded": self.loaded.is_some(),
            "error": self.error,
            "title": self.loaded.as_ref().and_then(|l| l.title.clone()),
            "pages": self.loaded.as_ref().map_or(0, |l| l.sizes.len()),
            "page": self.page + 1,
            "zoom": json_number(self.resolved),
            "zoom_mode": self.zoom.mode(),
            "zoom_value": self.zoom.to_json(),
            "scale": json_number(self.resolved * CSS_UNITS),
            "scroll": {"left": json_number(self.scroll.x), "top": json_number(self.scroll.y)},
            "viewport": {"width": json_number(self.viewport.width), "height": json_number(self.viewport.height)},
            "content": self.layout.as_ref().map(|l| json!({"width": json_number(l.content.width), "height": json_number(l.content.height)})),
            "sidebar": self.sidebar,
            "sidebar_mode": match self.sidebar_mode {
                SidebarMode::Thumbnails => "thumbnails",
                SidebarMode::Outline => "outline",
            },
            "outline": outline,
            "expanded": self.expanded.len(),
            "menu": match self.menu {
                Menu::None => Value::Null,
                Menu::Sidebar => json!("sidebar"),
                Menu::Display => json!("display"),
                Menu::Selection => json!("selection"),
            },
            "find": find,
            "spread": self.spread.mode(),
            "adapt_to_theme": adapt_to_theme(cx),
            "locked": self.locked.map(|wrong| json!({"wrong": wrong})),
            "selection": self.selection.and_then(|(a, b)| {
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                (start != end).then(|| json!({
                    "start": [start.page + 1, start.item, start.offset],
                    "end": [end.page + 1, end.item, end.offset],
                    "text": self.selected_text(cx),
                }))
            }),
            "highlight": self.highlight.map(|(a, b)| json!({
                "start": [a.page + 1, a.item, a.offset],
                "end": [b.page + 1, b.item, b.offset],
            })),
            "remembered": self.positions.len(),
            "visible": self.visible,
            "requested": self.requested,
            "rendered": self.rendered,
            "cached_bytes": cx.try_global::<crate::pdf::Documents>().map_or(0, |d| d.cached_bytes()),
        })
    }

    // ---- the text --------------------------------------------------------------

    /// Page `page`'s text items if they are read.
    fn items(&self, page: usize, cx: &App) -> Option<Arc<Vec<TextItem>>> {
        let absolute = self.absolute.as_ref()?;
        cx.try_global::<crate::pdf::Documents>()?
            .cached_text(absolute, page)
    }

    /// The selection, start first, when it covers anything.
    fn ordered_selection(&self) -> Option<(TextPos, TextPos)> {
        let (a, b) = self.selection?;
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        (start != end).then_some((start, end))
    }

    /// The selection as the text layer reads it (line breaks kept).
    fn selected_text(&self, cx: &App) -> String {
        let Some((start, end)) = self.ordered_selection() else {
            return String::new();
        };
        selection::selected_text(start, end, &|page| self.items(page, cx))
    }

    /// The place in the text under the window point `at`, on the nearest
    /// page; `None` while that page's text is not read or it has none.
    fn text_pos_at(&self, at: Point<Pixels>, cx: &mut Context<Self>) -> Option<TextPos> {
        let layout = self.layout.as_ref()?;
        let x = f32::from(at.x) - self.origin.x + self.scroll.x;
        let y = f32::from(at.y) - self.origin.y + self.scroll.y;
        let (page, px_, py_) = layout.page_point(x, y)?;
        let absolute = self.absolute.clone()?;
        let items = documents(cx).text(&absolute, page, cx)?;
        let (item, offset) = selection::hit(&items, px_, py_)?;
        Some(TextPos { page, item, offset })
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = Menu::None;
        self.highlight = None;
        if !self.focus.is_focused(window) {
            window.focus(&self.focus, cx);
        }
        let Some(at) = self.text_pos_at(event.position, cx) else {
            self.selection = None;
            cx.notify();
            return;
        };
        let items = self.items(at.page, cx);
        let item = items.as_ref().and_then(|items| items.get(at.item));
        match (event.click_count, item) {
            (2, Some(item)) => {
                let (from, to) = selection::word_at(item, at.offset);
                self.selection =
                    Some((TextPos { offset: from, ..at }, TextPos { offset: to, ..at }));
            }
            (n, Some(item)) if n >= 3 => {
                self.selection = Some((
                    TextPos { offset: 0, ..at },
                    TextPos {
                        offset: item.len(),
                        ..at
                    },
                ));
            }
            _ => {
                self.selection = Some((at, at));
                self.selecting = true;
            }
        }
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selecting || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        if let Some(head) = self.text_pos_at(event.position, cx) {
            if let Some((anchor, old)) = self.selection {
                if old != head {
                    self.selection = Some((anchor, head));
                    cx.notify();
                }
            }
        }
    }

    fn mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.selecting {
            self.selecting = false;
            if self.ordered_selection().is_none() {
                self.selection = None;
            }
            cx.notify();
        }
    }

    /// A right click: with a selection, its menu (observed: none without
    /// one, anywhere on the view).
    fn secondary_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if self.ordered_selection().is_some() {
            self.menu = Menu::Selection;
            self.menu_at = Point::new(f32::from(event.position.x), f32::from(event.position.y));
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// "Copy" and `Ctrl+C`.
    fn copy_selection(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.selected_text(cx);
        if text.is_empty() {
            cx.propagate();
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(selection::copy_text(&text)));
    }

    /// "Copy link to selection" and "Copy as quote": the subpath and alias
    /// go to the workspace, which knows the link path. A selection over
    /// two pages has no link (observed: the menu offers "Copy" alone).
    fn copy_link(&mut self, quote: bool, cx: &mut Context<Self>) {
        let Some((start, end)) = self.ordered_selection() else {
            return;
        };
        if start.page != end.page {
            return;
        }
        let Some(items) = self.items(start.page, cx) else {
            return;
        };
        let Some(sel) = selection::link_selection(&items, start, end) else {
            return;
        };
        let page = start.page + 1;
        let name = self
            .path
            .as_deref()
            .map(|p| p.rsplit('/').next().unwrap_or(p))
            .map(|n| {
                n.strip_suffix(".pdf")
                    .or(n.strip_suffix(".PDF"))
                    .unwrap_or(n)
            })
            .unwrap_or_default()
            .to_string();
        let text = self.selected_text(cx);
        cx.emit(PdfEvent::CopyLink {
            subpath: format!("page={page}&selection={}", sel.to_value()),
            alias: format!("{name}, page {page}"),
            quote: quote.then_some(text),
        });
    }

    // ---- the password ------------------------------------------------------------

    fn password_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let mods = keystroke.modifiers;
        match keystroke.key.as_str() {
            "enter" => {
                if let Some(absolute) = self.absolute.clone() {
                    let password = std::mem::take(&mut self.password);
                    documents(cx).unlock(&absolute, &password, cx);
                    self.locked = None;
                    self.error = None;
                    self.focus_password = true;
                }
            }
            "backspace" => {
                self.password.pop();
            }
            _ if mods.control || mods.platform || mods.alt => return,
            _ => match &keystroke.key_char {
                Some(ch) if !ch.chars().any(char::is_control) => self.password.push_str(ch),
                _ => return,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }

    // ---- the keys -------------------------------------------------------------

    fn scroll_up(&mut self, _: &ScrollUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(0.0, -SCROLL_LINE, cx);
    }

    fn scroll_down(&mut self, _: &ScrollDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(0.0, SCROLL_LINE, cx);
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(0.0, -self.viewport.height * SCROLL_PAGE, cx);
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(0.0, self.viewport.height * SCROLL_PAGE, cx);
    }

    fn scroll_to_top(&mut self, _: &ScrollToTop, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(0.0, -self.scroll.y, cx);
    }

    fn scroll_to_bottom(&mut self, _: &ScrollToBottom, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(0.0, f32::MAX / 4.0, cx);
    }

    fn next_page(&mut self, _: &NextPage, _: &mut Window, cx: &mut Context<Self>) {
        self.go_to_page(self.page + 1, true, cx);
    }

    fn previous_page(&mut self, _: &PreviousPage, _: &mut Window, cx: &mut Context<Self>) {
        self.go_to_page(self.page.saturating_sub(1), true, cx);
    }

    fn find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            self.find = Some(FindState {
                query: String::new(),
                matches: Vec::new(),
                current: 0,
                searching: false,
                case_sensitive: false,
                _task: None,
            });
        }
        self.menu = Menu::None;
        self.find_input.update(cx, |input, cx| input.select_all(cx));
        window.focus(&self.find_input.focus_handle(cx), cx);
        cx.notify();
    }

    fn find_next(&mut self, _: &FindNext, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.find_input.read(cx).text().to_string();
        self.search(text, true, cx);
    }

    fn find_previous(&mut self, _: &FindPrevious, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.find_input.read(cx).text().to_string();
        self.search(text, false, cx);
    }

    /// Escape: a menu first, then the find bar (observed: Escape closes it);
    /// with neither open the key goes on to the workspace.
    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu != Menu::None {
            self.menu = Menu::None;
            cx.notify();
        } else if self.find.is_some() {
            self.close_find(cx);
        } else {
            cx.propagate();
        }
    }

    fn close_find(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        self.refocus = true;
        cx.notify();
    }

    /// Searches for `query`, or moves to the next (`forward`) or previous
    /// match of the query already searched. The pages' text is read on the
    /// background executor; the bar says nothing until it is back.
    fn search(&mut self, query: String, forward: bool, cx: &mut Context<Self>) {
        let Some(loaded) = self.loaded.clone() else {
            return;
        };
        if self.find.is_none() {
            self.find = Some(FindState {
                query: String::new(),
                matches: Vec::new(),
                current: 0,
                searching: false,
                case_sensitive: false,
                _task: None,
            });
        }
        let find = self.find.as_mut().expect("just made");
        if query.is_empty() {
            find.query.clear();
            find.matches.clear();
            find.current = 0;
            cx.notify();
            return;
        }
        if find.query == query && !find.searching {
            if !find.matches.is_empty() {
                let n = find.matches.len();
                find.current = if forward {
                    (find.current + 1) % n
                } else {
                    (find.current + n - 1) % n
                };
                let page = find.matches[find.current];
                self.go_to_page(page, true, cx);
            }
            cx.notify();
            return;
        }
        find.query = query.clone();
        find.matches.clear();
        find.current = 0;
        find.searching = true;
        let case_sensitive = find.case_sensitive;
        let needle = if case_sensitive {
            query.clone()
        } else {
            query.to_lowercase()
        };
        find._task = Some(cx.spawn(async move |this, cx| {
            let doc = loaded.clone();
            let matches: Vec<usize> = cx
                .background_executor()
                .spawn(async move {
                    let mut found = Vec::new();
                    for ix in 0..doc.sizes.len() {
                        let Some(text) = doc.document.page_text(ix) else {
                            continue;
                        };
                        let hay = if case_sensitive {
                            text
                        } else {
                            text.to_lowercase()
                        };
                        found.extend(hay.matches(needle.as_str()).map(|_| ix));
                    }
                    found
                })
                .await;
            this.update(cx, |view, cx| {
                let Some(find) = view.find.as_mut() else {
                    return;
                };
                if find.query != query {
                    return;
                }
                find.searching = false;
                find.matches = matches;
                // The first match on or after the page the view is on, as
                // pdf.js starts from the current page.
                let from = view.page;
                find.current = find.matches.iter().position(|&p| p >= from).unwrap_or(0);
                if let Some(&page) = find.matches.get(find.current) {
                    view.go_to_page(page, true, cx);
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    // ---- the mouse ------------------------------------------------------------

    /// The wheel over the pages: scrolls, or under Ctrl zooms about the
    /// pointer one step per [`WHEEL_STEP`] pixels (observed: 100 px of wheel
    /// is three steps, about the pointer).
    fn wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        let (mut dx, mut dy) = (f32::from(delta.x), f32::from(delta.y));
        if event.modifiers.control || event.modifiers.platform {
            self.wheel_accum += dy;
            if self.wheel_accum.abs() >= WHEEL_STEP {
                let steps = (self.wheel_accum / WHEEL_STEP).trunc();
                self.wheel_accum -= steps * WHEEL_STEP;
                let mut scale = self.resolved;
                for _ in 0..(steps.abs() as usize) {
                    scale = if steps < 0.0 {
                        step_in(scale)
                    } else {
                        step_out(scale)
                    };
                }
                let offset = f32::from(event.position.y) - self.origin.y;
                self.set_zoom(Zoom::Percent(scale), offset, cx);
            }
            return;
        }
        if event.modifiers.shift && dx == 0.0 {
            std::mem::swap(&mut dx, &mut dy);
        }
        self.scroll_by(-dx, -dy, cx);
    }

    /// The pages area was laid out: its size decides the fit zooms and the
    /// current page, so a change asks for one more frame.
    fn measured(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.origin = Point::new(f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        let size = Size::new(f32::from(bounds.size.width), f32::from(bounds.size.height));
        if size != self.viewport {
            // Keep the page at the top where it is across the resize.
            if self.pending.is_none() {
                if let Some(layout) = &self.layout {
                    let (page, frac) = layout.locate(self.scroll.y);
                    self.pending = Some(Target::Anchor {
                        page,
                        frac,
                        offset: 0.0,
                    });
                }
            }
            self.viewport = size;
            self.fit_page = self.page;
            self.settle(cx);
            let this = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| {
                this.update(cx, |_, cx| cx.notify()).ok();
            });
        }
    }

    /// The toolbar was laid out: its menus hang from it.
    fn toolbar_measured(&mut self, bounds: Bounds<Pixels>) {
        self.toolbar_origin = Point::new(f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    }

    // ---- the sidebar -----------------------------------------------------------

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar = !self.sidebar;
        self.sidebar_followed = None;
        self.menu = Menu::None;
        cx.notify();
    }

    fn show_sidebar(&mut self, mode: SidebarMode, cx: &mut Context<Self>) {
        self.sidebar = true;
        self.sidebar_mode = mode;
        self.sidebar_followed = None;
        self.menu = Menu::None;
        cx.notify();
    }

    /// "Reveal page in table of contents": the outline, opened to the last
    /// item at or before the current page.
    fn reveal_in_outline(&mut self, cx: &mut Context<Self>) {
        self.show_sidebar(SidebarMode::Outline, cx);
        let Some(loaded) = self.loaded.clone() else {
            return;
        };
        let target = loaded
            .outline
            .iter()
            .rposition(|item| item.page.is_some_and(|p| p <= self.page));
        if let Some(ix) = target {
            // Expand every ancestor: the nearest shallower item before it,
            // and so on up.
            let mut depth = loaded.outline[ix].depth;
            for (jx, item) in loaded.outline[..ix].iter().enumerate().rev() {
                if item.depth < depth {
                    self.expanded.insert(jx);
                    depth = item.depth;
                    if depth == 0 {
                        break;
                    }
                }
            }
        }
    }

    /// For each outline item, whether it has children and whether it is
    /// hidden under a collapsed ancestor.
    fn outline_visibility(items: &[OutlineItem], expanded: &HashSet<usize>) -> Vec<(bool, bool)> {
        let mut out = Vec::with_capacity(items.len());
        // The collapsed depth, if any: items deeper than it are hidden.
        let mut hidden_below: Option<usize> = None;
        for (ix, item) in items.iter().enumerate() {
            if let Some(d) = hidden_below {
                if item.depth > d {
                    out.push((false, true));
                    continue;
                }
                hidden_below = None;
            }
            let has_children = items
                .get(ix + 1)
                .is_some_and(|next| next.depth > item.depth);
            if has_children && !expanded.contains(&ix) {
                hidden_below = Some(item.depth);
            }
            out.push((has_children, false));
        }
        out
    }

    // ---- the frame ------------------------------------------------------------

    /// Asks the cache for the file and lays the pages out at the current
    /// zoom, applying whatever scroll was waiting for the layout.
    fn prepare(&mut self, cx: &mut Context<Self>) {
        let Some(absolute) = self.absolute.clone() else {
            self.layout = None;
            return;
        };
        if self.loaded.is_none() && self.error.is_none() {
            match documents(cx).document(&absolute, cx) {
                DocState::Loading => {}
                DocState::Ready(loaded) => {
                    self.loaded = Some(loaded);
                    self.locked = None;
                }
                DocState::Failed(message) => self.error = Some(message),
                DocState::Locked { wrong } => {
                    if self.locked.is_none() {
                        // Observed: the field has the keyboard as soon as
                        // the prompt shows, and again after a refusal.
                        self.focus_password = true;
                    }
                    self.locked = Some(wrong);
                }
            }
        }
        let Some(loaded) = self.loaded.clone() else {
            self.layout = None;
            return;
        };
        let scale = self
            .zoom
            .resolve(&loaded.sizes, self.fit_page, self.viewport, self.spread);
        self.resolved = scale;
        let layout = Layout::new(
            &loaded.sizes,
            scale * CSS_UNITS,
            self.viewport.width,
            self.spread,
        );
        let fits_known = self.viewport.width > 0.0 && self.viewport.height > 0.0;
        if let Some(target) = self.pending {
            if fits_known {
                self.pending = None;
                match target {
                    Target::Selection { page, selection } => {
                        let items = documents(cx).text(&absolute, page, cx);
                        match items {
                            // Not read yet: wait at the page's top.
                            None => {
                                self.pending = Some(target);
                                self.scroll.y = layout.top_of(page);
                            }
                            Some(items) => {
                                let resolved =
                                    selection::resolve_selection(&items, page, selection);
                                self.highlight = resolved;
                                self.scroll.y = match (resolved, layout.pages.get(page)) {
                                    (Some((start, _)), Some(b)) => {
                                        let top = items[start.item].bounds()[1];
                                        (b.top() + top * layout.scale - FOLLOW_MARGIN).max(0.0)
                                    }
                                    _ => layout.top_of(page),
                                };
                            }
                        }
                    }
                    Target::Page(page) => self.scroll.y = layout.top_of(page),
                    Target::Anchor { page, frac, offset } => {
                        if let Some(b) = layout.pages.get(page) {
                            self.scroll.y = if frac < 0.0 {
                                // In the border above the page: so many
                                // pixels above it, whatever the zoom.
                                b.top() + frac.max(-1.0) * PAGE_BORDER - offset
                            } else {
                                b.top() + frac * b.size.height - offset
                            };
                        }
                        // The column re-centres under a zoom change.
                        self.scroll.x = (layout.content.width - self.viewport.width).max(0.0) / 2.0;
                    }
                }
            }
        }
        self.layout = Some(layout);
        self.clamp_scroll();
        if self.pending.is_none() {
            if let Some(layout) = &self.layout {
                self.page = layout.current_page(self.scroll, self.viewport, self.page);
            }
        }
    }

    // ---- rendering ------------------------------------------------------------

    /// A toolbar `clickable-icon`: a 16 px icon in a 28 by 24 box with a 4
    /// px radius, `--icon-color` at `--icon-opacity`; under the pointer
    /// `--icon-color-hover` on the hover wash, and while `active` (a menu
    /// it opened is up, the sidebar is open) `--icon-color-active` on
    /// `--background-modifier-active-hover` (probe of 2026-11-04).
    #[allow(clippy::too_many_arguments)]
    fn control(
        &self,
        id: &'static str,
        name: &str,
        tip: &'static str,
        enabled: bool,
        active: bool,
        theme: &Theme,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .w(px(CONTROL_WIDTH))
            .h(px(CONTROL_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .group(id)
            .when(active, |d| {
                d.bg(theme.tokens.background_modifier_active_hover)
            })
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        on_click(this, window, cx)
                    }))
            })
            .tooltip(tooltip(tip))
            .child(if !enabled {
                icon(name, ICON_SIZE, theme.tokens.text_faint)
            } else if active {
                icon(name, ICON_SIZE, theme.tokens.icon_color_active)
                    .opacity(theme.tokens.icon_opacity_active)
            } else {
                icon(name, ICON_SIZE, theme.tokens.icon_color)
                    .opacity(theme.tokens.icon_opacity)
                    .group_hover(id, |s| {
                        s.opacity(theme.tokens.icon_opacity_hover)
                            .text_color(theme.tokens.icon_color_hover)
                    })
            })
            .into_any_element()
    }

    /// A `pdf-toolbar-divider`: a 1 by 16 hairline.
    fn divider(theme: &Theme) -> AnyElement {
        div()
            .w(px(1.))
            .h(px(DIVIDER_HEIGHT))
            .flex_shrink_0()
            .bg(theme.tokens.background_modifier_border)
            .into_any_element()
    }

    /// The toolbar, as observed: sidebar toggle and its options; a spacer;
    /// zoom out, a divider, zoom in, display options; a spacer; the page
    /// box and "of N". The right-hand group is empty.
    fn render_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.global::<Theme>().clone();
        let pages = self.loaded.as_ref().map_or(0, |l| l.sizes.len());
        let ready = pages > 0;
        // Observed for a broken and for a locked file: `0`, `of 0`.
        let page_text = if ready {
            (self.page + 1).to_string()
        } else if self.error.is_some() || self.locked.is_some() {
            "0".to_string()
        } else {
            String::new()
        };
        if !self.page_input.focus_handle(cx).is_focused(window)
            && self.page_input.read(cx).text() != page_text
        {
            self.page_input
                .update(cx, |input, cx| input.set_text(page_text, cx));
        }
        let count: SharedString = if ready || self.error.is_some() || self.locked.is_some() {
            format!("of {pages}").into()
        } else {
            "".into()
        };
        let entity = cx.entity();
        div()
            .id("pdf-toolbar")
            .relative()
            .h(px(TOOLBAR_HEIGHT))
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(CONTROL_GAP))
            .px(px(TOOLBAR_PAD_X))
            .bg(theme.tokens.background_primary)
            .border_b_1()
            .border_color(theme.tokens.background_modifier_border)
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .child(
                canvas(
                    move |bounds, _, cx| entity.update(cx, |view, _| view.toolbar_measured(bounds)),
                    |_, _, _, _| (),
                )
                .absolute()
                .size_full(),
            )
            .child(self.control(
                "pdf-sidebar-toggle",
                "layout-list",
                "Toggle sidebar",
                ready,
                self.sidebar,
                &theme,
                |this, _, cx| this.toggle_sidebar(cx),
                cx,
            ))
            .child(self.control(
                "pdf-sidebar-options",
                "chevron-down",
                "Sidebar options",
                ready,
                self.menu == Menu::Sidebar,
                &theme,
                |this, _, cx| {
                    this.menu = if this.menu == Menu::Sidebar {
                        Menu::None
                    } else {
                        Menu::Sidebar
                    };
                    cx.notify();
                },
                cx,
            ))
            .child(div().w(px(SPACER_WIDTH)).flex_shrink_0())
            .child(self.control(
                "pdf-zoom-out",
                "zoom-out",
                "Zoom out",
                ready && self.resolved > MIN_SCALE,
                false,
                &theme,
                |this, _, cx| this.set_zoom(Zoom::Percent(step_out(this.resolved)), 0.0, cx),
                cx,
            ))
            .child(Self::divider(&theme))
            .child(self.control(
                "pdf-zoom-in",
                "zoom-in",
                "Zoom in",
                ready && self.resolved < MAX_SCALE,
                false,
                &theme,
                |this, _, cx| this.set_zoom(Zoom::Percent(step_in(this.resolved)), 0.0, cx),
                cx,
            ))
            .child(self.control(
                "pdf-display-options",
                "chevron-down",
                "Display options",
                ready,
                self.menu == Menu::Display,
                &theme,
                |this, _, cx| {
                    this.menu = if this.menu == Menu::Display {
                        Menu::None
                    } else {
                        Menu::Display
                    };
                    cx.notify();
                },
                cx,
            ))
            .child(div().w(px(SPACER_WIDTH)).flex_shrink_0())
            .child(
                div()
                    .w(px(PAGE_INPUT_WIDTH))
                    .flex_shrink_0()
                    .child(self.page_input.clone()),
            )
            .child(div().mr(px(4.)).whitespace_nowrap().child(count))
            .into_any_element()
    }

    /// A menu row that does what `on_click` says and closes the menu.
    #[allow(clippy::too_many_arguments)]
    fn menu_row(
        &self,
        prefix: &'static str,
        ix: usize,
        icon_name: &'static str,
        title: &'static str,
        checked: bool,
        theme: &Theme,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        menu_item(prefix, ix, Some(icon_name), title, checked, theme)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.menu = Menu::None;
                on_click(this, cx);
                cx.notify();
            }))
            .into_any_element()
    }

    fn menu_separator(theme: &Theme) -> AnyElement {
        div()
            .h(px(1.))
            .my(px(4.))
            .bg(theme.tokens.background_modifier_border)
            .into_any_element()
    }

    /// The open menu, hung from its control: 2 px under the control's
    /// bottom edge, at its left, as observed.
    fn render_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, left) = match self.menu {
            Menu::None => return None,
            Menu::Selection => ("pdf-selection-menu", 0.0),
            Menu::Sidebar => (
                "pdf-sidebar-menu",
                TOOLBAR_PAD_X + CONTROL_WIDTH + CONTROL_GAP,
            ),
            Menu::Display => (
                "pdf-display-menu",
                TOOLBAR_PAD_X
                    + 2.0 * (CONTROL_WIDTH + CONTROL_GAP)
                    + SPACER_WIDTH
                    + CONTROL_GAP
                    + CONTROL_WIDTH
                    + CONTROL_GAP
                    + 1.0
                    + CONTROL_GAP
                    + CONTROL_WIDTH
                    + CONTROL_GAP,
            ),
        };
        let at = if self.menu == Menu::Selection {
            point(px(self.menu_at.x), px(self.menu_at.y))
        } else {
            point(
                px(self.toolbar_origin.x + left),
                px(self.toolbar_origin.y
                    + (TOOLBAR_HEIGHT - CONTROL_HEIGHT) / 2.0
                    + CONTROL_HEIGHT
                    + 2.0),
            )
        };
        let mut body = menu(id, theme).on_mouse_down_out(cx.listener(|this, _, _, cx| {
            this.menu = Menu::None;
            cx.notify();
        }));
        body = match self.menu {
            Menu::Sidebar => body
                .child(self.menu_row(
                    "pdf-sidebar-menu-item",
                    0,
                    "layout-grid",
                    "Thumbnails",
                    self.sidebar_mode == SidebarMode::Thumbnails,
                    theme,
                    |this, cx| this.show_sidebar(SidebarMode::Thumbnails, cx),
                    cx,
                ))
                .child(self.menu_row(
                    "pdf-sidebar-menu-item",
                    1,
                    "list",
                    "Table of contents",
                    self.sidebar_mode == SidebarMode::Outline,
                    theme,
                    |this, cx| this.show_sidebar(SidebarMode::Outline, cx),
                    cx,
                ))
                .child(Self::menu_separator(theme))
                .child(self.menu_row(
                    "pdf-sidebar-menu-item",
                    2,
                    "list-start",
                    "Reveal page in table of contents",
                    false,
                    theme,
                    |this, cx| this.reveal_in_outline(cx),
                    cx,
                )),
            Menu::Display => body
                .child(self.menu_row(
                    "pdf-display-menu-item",
                    0,
                    "move-horizontal",
                    "Fit width",
                    self.zoom == Zoom::FitWidth,
                    theme,
                    |this, cx| this.set_zoom(Zoom::FitWidth, 0.0, cx),
                    cx,
                ))
                .child(self.menu_row(
                    "pdf-display-menu-item",
                    1,
                    "move-vertical",
                    "Fit height",
                    self.zoom == Zoom::FitHeight,
                    theme,
                    |this, cx| this.set_zoom(Zoom::FitHeight, 0.0, cx),
                    cx,
                ))
                .child(Self::menu_separator(theme))
                // The two-page rows carry Obsidian's own
                // `rectangle-vertical-double`, drawn in `assets`.
                .child(self.menu_row(
                    "pdf-display-menu-item",
                    2,
                    "rectangle-vertical",
                    "Single page",
                    self.spread == Spread::None,
                    theme,
                    |this, cx| this.set_spread(Spread::None, cx),
                    cx,
                ))
                .child(self.menu_row(
                    "pdf-display-menu-item",
                    3,
                    "rectangle-vertical-double",
                    "Two-page (odd)",
                    self.spread == Spread::Odd,
                    theme,
                    |this, cx| this.set_spread(Spread::Odd, cx),
                    cx,
                ))
                .child(self.menu_row(
                    "pdf-display-menu-item",
                    4,
                    "rectangle-vertical-double",
                    "Two-page (even)",
                    self.spread == Spread::Even,
                    theme,
                    |this, cx| this.set_spread(Spread::Even, cx),
                    cx,
                ))
                .child(Self::menu_separator(theme))
                .child(self.menu_row(
                    "pdf-display-menu-item",
                    5,
                    "palette",
                    "Adapt to theme",
                    adapt_to_theme(cx),
                    theme,
                    |_, cx| toggle_adapt_to_theme(cx),
                    cx,
                )),
            // Observed rows, all `lucide-copy`, in the `selection` section;
            // a selection over two pages offers "Copy" alone.
            Menu::Selection => {
                let one_page = self
                    .ordered_selection()
                    .is_some_and(|(a, b)| a.page == b.page);
                let body = body.child(self.menu_row(
                    "pdf-selection-menu-item",
                    0,
                    "copy",
                    "Copy",
                    false,
                    theme,
                    |this, cx| {
                        let text = this.selected_text(cx);
                        if !text.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(selection::copy_text(
                                &text,
                            )));
                        }
                    },
                    cx,
                ));
                if one_page {
                    body.child(self.menu_row(
                        "pdf-selection-menu-item",
                        1,
                        "copy",
                        "Copy as quote",
                        false,
                        theme,
                        |this, cx| this.copy_link(true, cx),
                        cx,
                    ))
                    .child(self.menu_row(
                        "pdf-selection-menu-item",
                        2,
                        "copy",
                        "Copy link to selection",
                        false,
                        theme,
                        |this, cx| this.copy_link(false, cx),
                        cx,
                    ))
                } else {
                    body
                }
            }
            Menu::None => body,
        };
        Some(
            deferred(
                anchored()
                    .position(at)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(body),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// The find bar, as observed: the match count ("1 of 3 matches",
    /// nothing while there are none), the 300 px "Search..." field with its
    /// match-case toggle, previous (Shift+F3) and next (F3), the search
    /// settings, a divider, close. Padded 6, gap 4, 13 px text; a query
    /// with no match rings the field red.
    fn render_find_bar(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let (status, missed): (Option<SharedString>, bool) = match &self.find {
            Some(f) if f.searching || f.query.is_empty() => (None, false),
            Some(f) if f.matches.is_empty() => (None, true),
            Some(f) => (
                Some(format!("{} of {} matches", f.current + 1, f.matches.len()).into()),
                false,
            ),
            None => (None, false),
        };
        let case_sensitive = self.find.as_ref().is_some_and(|f| f.case_sensitive);
        // `.pdf-findbar .mod-not-found`: the focus ring becomes
        // `--background-modifier-error` at 0.6.
        let mut miss_ring: Hsla = theme.tokens.background_modifier_error;
        miss_ring.a *= 0.6;
        div()
            .id("pdf-find-bar")
            .h(px(FIND_BAR_HEIGHT))
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .p(px(6.))
            .bg(theme.tokens.background_primary)
            .border_b_1()
            .border_color(theme.tokens.background_modifier_border)
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .children(status.map(|text| {
                div()
                    .mx(px(8.))
                    .whitespace_nowrap()
                    .text_color(theme.tokens.text_muted)
                    .child(text)
            }))
            .child(
                div()
                    .relative()
                    .w(px(FIND_INPUT_WIDTH))
                    .flex_shrink_0()
                    .rounded(px(5.))
                    .when(missed, |d| {
                        d.shadow(vec![BoxShadow {
                            color: miss_ring,
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(0.),
                            spread_radius: px(2.),
                        }])
                    })
                    .child(self.find_input.clone())
                    .child(
                        div()
                            .id("pdf-find-case")
                            .absolute()
                            .right(px(4.))
                            .top(px(5.))
                            .w(px(24.))
                            .h(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .when(case_sensitive, |d| {
                                d.bg(theme.tokens.background_modifier_active_hover)
                            })
                            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                            .tooltip(tooltip("Match case"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if let Some(find) = this.find.as_mut() {
                                    find.case_sensitive = !find.case_sensitive;
                                    // A new search with the other rule.
                                    find.query.clear();
                                }
                                let text = this.find_input.read(cx).text().to_string();
                                this.search(text, true, cx);
                            }))
                            .child(icon(
                                "case-sensitive",
                                ICON_SIZE,
                                if case_sensitive {
                                    theme.tokens.icon_color_active
                                } else {
                                    theme.tokens.icon_color
                                },
                            )),
                    ),
            )
            .child(self.control(
                "pdf-find-previous",
                "arrow-up",
                "Shift + F3",
                true,
                false,
                theme,
                |this, window, cx| this.find_previous(&FindPrevious, window, cx),
                cx,
            ))
            .child(self.control(
                "pdf-find-next",
                "arrow-down",
                "F3",
                true,
                false,
                theme,
                |this, window, cx| this.find_next(&FindNext, window, cx),
                cx,
            ))
            .child(self.control(
                "pdf-find-settings",
                "sliders-horizontal",
                "Search settings",
                true,
                false,
                theme,
                |_, _, cx| {
                    cx.emit(PdfEvent::Report(
                        "Search settings are not supported yet".into(),
                    ))
                },
                cx,
            ))
            .child(Self::divider(theme))
            .child(self.control(
                "pdf-find-close",
                "x",
                "Close",
                true,
                false,
                theme,
                |this, _, cx| this.close_find(cx),
                cx,
            ))
            .into_any_element()
    }

    /// The sidebar: 140 px of thumbnails, or the table of contents.
    fn render_sidebar(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut panel = div()
            .id("pdf-sidebar")
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .track_scroll(&self.sidebar_scroll)
            .bg(theme.tokens.pdf_sidebar_background)
            .border_r_1()
            .border_color(theme.tokens.divider_color)
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.menu = Menu::None;
                    window.focus(&this.focus, cx);
                }),
            );
        let Some(loaded) = self.loaded.clone() else {
            return panel.into_any_element();
        };
        match self.sidebar_mode {
            SidebarMode::Thumbnails => {
                panel = panel.child(self.render_thumbnails(&loaded, theme, window, cx));
            }
            SidebarMode::Outline => {
                panel = panel.child(self.render_outline(&loaded, theme, cx));
            }
        }
        panel.into_any_element()
    }

    /// Thumbnails, as observed: every page 98 px wide in an 8 px border
    /// with a 4 px radius, 4 px apart in a column padded 12, the current
    /// one's border washed (`rgba(255,255,255,0.067)` in the dark theme)
    /// and the image ringed by `--pdf-thumbnail-shadow`. Only the rows in
    /// or near view ask for an image, and the column follows the current
    /// page when it changes.
    fn render_thumbnails(
        &mut self,
        loaded: &Arc<Loaded>,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(absolute) = self.absolute.clone() else {
            return div().into_any_element();
        };
        let cache = documents(cx);
        let inner_w = THUMBNAIL_WIDTH;
        let heights: Vec<f32> = loaded
            .sizes
            .iter()
            .map(|s| {
                if s.width > 0.0 {
                    inner_w * s.height / s.width
                } else {
                    inner_w
                }
            })
            .collect();
        // Each row: the bordered thumbnail and the gap under it.
        let mut tops = Vec::with_capacity(heights.len());
        let mut y = THUMBNAIL_PAD;
        for h in &heights {
            tops.push(y);
            y += h + 2.0 * THUMBNAIL_BORDER + THUMBNAIL_GAP;
        }
        let bounds = self.sidebar_scroll.bounds();
        let view_h = f32::from(bounds.size.height);
        let mut top = -f32::from(self.sidebar_scroll.offset().y);
        if self.sidebar_followed != Some(self.page) && view_h > 0.0 {
            // Bring the current page's thumbnail into view.
            if let (Some(&t), Some(&h)) = (tops.get(self.page), heights.get(self.page)) {
                let outer = h + 2.0 * THUMBNAIL_BORDER;
                if t < top || t + outer > top + view_h {
                    top = (t - THUMBNAIL_PAD).max(0.0);
                    self.sidebar_scroll.set_offset(point(px(0.), px(-top)));
                }
            }
            self.sidebar_followed = Some(self.page);
        }
        // `.thumbnail.selected` and `:hover` colour the 8 px border with the
        // hover wash; the image is framed by `--pdf-thumbnail-shadow`.
        let selected: Hsla = theme.tokens.background_modifier_hover;
        let frame = crate::theme::box_shadows(&theme.tokens.pdf_thumbnail_shadow);
        let mut column = div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .gap(px(THUMBNAIL_GAP))
            .p(px(THUMBNAIL_PAD));
        for (ix, size) in loaded.sizes.iter().enumerate() {
            let h = heights[ix];
            let outer = h + 2.0 * THUMBNAIL_BORDER;
            let near = view_h <= 0.0
                || (tops[ix] + outer >= top - view_h && tops[ix] <= top + 2.0 * view_h);
            let current = ix == self.page;
            let mut image = div()
                .w(px(inner_w))
                .h(px(h))
                .bg(gpui::white())
                .shadow(frame.clone());
            if near && size.width > 0.0 {
                let scale = inner_w / size.width * window.scale_factor();
                // OBSERVE: whether Adapt to theme recolours the thumbnails;
                // they follow the pages here.
                let themed = theming(cx);
                let _ = cache.has_page(&absolute, ix, scale, themed);
                image = image.child(
                    img(page_source(absolute.clone(), ix, scale, themed))
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                );
            }
            column = column.child(
                div()
                    .id(("pdf-thumbnail", ix))
                    .flex_shrink_0()
                    .p(px(THUMBNAIL_BORDER))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(current, |d| d.bg(selected))
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.go_to_page(ix, true, cx);
                    }))
                    .child(image),
            );
        }
        column.into_any_element()
    }

    /// The table of contents, as observed: Obsidian's tree, padded 8, one
    /// `tree-item` per outline entry with a collapse chevron when it has
    /// children, the title, and the page number as its 12 px flair. Every
    /// item starts collapsed. OBSERVE: the empty state's wording.
    fn render_outline(
        &self,
        loaded: &Arc<Loaded>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = &loaded.outline;
        if items.is_empty() {
            return div()
                .p(px(OUTLINE_PAD))
                .text_color(theme.tokens.text_faint)
                .child("No outline available")
                .into_any_element();
        }
        let visibility = Self::outline_visibility(items, &self.expanded);
        let mut list = div().flex().flex_col().w_full().p(px(OUTLINE_PAD));
        for (ix, item) in items.iter().enumerate() {
            let (has_children, hidden) = visibility[ix];
            if hidden {
                continue;
            }
            let page = item.page;
            let expanded = self.expanded.contains(&ix);
            let on_page = page == Some(self.page);
            let mut row = div()
                .id(("pdf-outline-item", ix))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .pl(px(4. + 12. * item.depth as f32))
                .pr(px(4.))
                .py(px(4.))
                .rounded(px(4.))
                .text_color(if on_page {
                    theme.tokens.nav_item_color_active
                } else {
                    theme.tokens.nav_item_color
                })
                .child(
                    div()
                        .id(("pdf-outline-collapse", ix))
                        .size(px(16.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(has_children, |d| {
                            d.cursor_pointer()
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if !this.expanded.remove(&ix) {
                                        this.expanded.insert(ix);
                                    }
                                    cx.notify();
                                }))
                                .child(icon(
                                    if expanded {
                                        "chevron-down"
                                    } else {
                                        "chevron-right"
                                    },
                                    12.,
                                    theme.tokens.collapse_icon_color,
                                ))
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(SharedString::from(item.title.clone())),
                )
                .children(page.map(|p| {
                    div()
                        .flex_shrink_0()
                        .text_size(px(FONT_UI_SMALLER))
                        .text_color(theme.tokens.text_faint)
                        .child(SharedString::from((p + 1).to_string()))
                }));
            if let Some(page) = page {
                row = row
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.nav_item_background_hover))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.go_to_page(page, true, cx);
                    }));
            }
            list = list.child(row);
        }
        list.into_any_element()
    }

    /// The pages: a column at `-scroll` in an `overflow_hidden` box. Every
    /// page near the viewport is a box with its image; one further off is
    /// the box alone, so a thousand-page file asks for a handful of renders.
    /// The box is ringed by `--pdf-shadow`: a 1 px line in the dark theme,
    /// a faint line and a soft shadow in the light one. The pages sit on
    /// `--pdf-background`.
    fn render_pages(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let mut area = div()
            .id("pdf-pages")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .bg(theme.tokens.pdf_background)
            .on_scroll_wheel(cx.listener(|this, event, window, cx| this.wheel(event, window, cx)))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        entity.update(cx, |view, cx| view.measured(bounds, window, cx))
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .size_full(),
            );
        self.requested = 0;
        self.rendered = 0;
        self.visible = 0;
        if let Some(wrong) = self.locked {
            return area
                .child(self.render_password(wrong, theme, cx))
                .into_any_element();
        }
        area = area
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event, _, cx| this.secondary_down(event, cx)),
            )
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.mouse_up(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.mouse_up(cx)),
            );
        let (Some(layout), Some(absolute)) = (self.layout.clone(), self.absolute.clone()) else {
            return area.into_any_element();
        };
        let (vw, vh) = (self.viewport.width, self.viewport.height);
        let top = self.scroll.y;
        let scale = layout.scale * window.scale_factor();
        let cache = documents(cx);
        let themed = theming(cx);
        let ground = themed.map_or(gpui::white(), theming_colour);
        let shadow = crate::theme::box_shadows(&theme.tokens.pdf_shadow);
        let selection = self.ordered_selection();
        let mut accent = theme.tokens.color_accent;
        accent.a *= TEXT_LAYER_OPACITY;
        let mut mark: Hsla = gpui::rgb(FOLLOW_MARK).into();
        mark.a *= TEXT_LAYER_OPACITY;
        let mut column = div()
            .absolute()
            .left(px(-self.scroll.x))
            .top(px(-top))
            .w(px(layout.content.width))
            .h(px(layout.content.height));
        for (ix, b) in layout.pages.iter().enumerate() {
            // Boxes are made for a viewport either side; images asked for
            // half a viewport either side.
            let near = b.bottom() >= top - vh && b.top() <= top + 2.0 * vh;
            if !near {
                continue;
            }
            let wanted = b.bottom() >= top - vh / 2.0 && b.top() <= top + 1.5 * vh;
            if b.bottom() >= top && b.top() <= top + vh {
                self.visible += 1;
            }
            let mut page = div()
                .absolute()
                .left(px(b.origin.x))
                .top(px(b.origin.y))
                .w(px(b.size.width))
                .h(px(b.size.height))
                .bg(ground)
                .shadow(shadow.clone())
                .cursor_text();
            if wanted && vw > 0.0 {
                self.requested += 1;
                if cache.has_page(&absolute, ix, scale, themed) {
                    self.rendered += 1;
                }
                page = page.child(
                    img(page_source(absolute.clone(), ix, scale, themed))
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                );
                // The text is read for every page in view, so a press on it
                // finds it there.
                if let Some(items) = cache.text(&absolute, ix, cx) {
                    let k = layout.scale;
                    let boxes = |start: TextPos, end: TextPos, pad: f32, colour: Hsla| {
                        selection::page_ranges(&items, ix, start, end)
                            .into_iter()
                            .filter_map(|(item, from, to)| {
                                let it = &items[item];
                                let (x0, x1) = it.span(from, to)?;
                                let [_, t, _, bot] = it.bounds();
                                Some(
                                    div()
                                        .absolute()
                                        .left(px(x0 * k - pad))
                                        .top(px(t * k - pad))
                                        .w(px((x1 - x0) * k + 2.0 * pad))
                                        .h(px((bot - t) * k + 2.0 * pad))
                                        .when(pad > 0.0, |d| d.rounded(px(pad)))
                                        .bg(colour)
                                        .into_any_element(),
                                )
                            })
                            .collect::<Vec<_>>()
                    };
                    if let Some((a, z)) = self.highlight {
                        page = page.children(boxes(a, z, FOLLOW_PAD, mark));
                    }
                    if let Some((a, z)) = selection {
                        page = page.children(boxes(a, z, 0.0, accent));
                    }
                }
            }
            column = column.child(page);
        }
        area = area.child(column);
        area.into_any_element()
    }

    /// A file encrypted with a user password, as observed: centred over the
    /// pages area, a 32 px `lucide-lock`, "This PDF is password protected"
    /// (15 px), "Enter the password below" (13 px) or, after a refused try,
    /// "Invalid password" in `--text-warning`, and a 214 by 30 password
    /// field with the placeholder "Password", which has the keyboard.
    /// Enter tries it; Escape does nothing.
    fn render_password(&self, wrong: bool, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let focused = self.focus_password_now;
        let field = div()
            .id("pdf-password-input")
            .track_focus(&self.password_focus)
            .key_context("PdfPassword")
            .on_key_down(cx.listener(Self::password_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    window.focus(&this.password_focus, cx);
                }),
            )
            .w(px(214.))
            .h(px(30.))
            .mt(px(12.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(5.))
            .border_1()
            .border_color(if focused {
                theme.tokens.background_modifier_border_focus
            } else {
                theme.tokens.background_modifier_border
            })
            .bg(theme.tokens.background_modifier_form_field)
            .text_size(px(FONT_UI_SMALL))
            .cursor_text()
            // The caret sits before the placeholder, after the dots.
            .when(focused && self.password.is_empty(), |d| {
                d.child(div().w(px(1.)).h(px(15.)).bg(theme.tokens.text_normal))
            })
            .child(if self.password.is_empty() {
                div()
                    .text_color(theme.tokens.text_faint)
                    .child("Password")
                    .into_any_element()
            } else {
                div()
                    .text_color(theme.tokens.text_normal)
                    .child(SharedString::from(
                        "\u{2022}".repeat(self.password.chars().count()),
                    ))
                    .into_any_element()
            })
            .when(focused && !self.password.is_empty(), |d| {
                d.child(div().w(px(1.)).h(px(15.)).bg(theme.tokens.text_normal))
            });
        div()
            .absolute()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_color(theme.tokens.text_normal)
            .child(icon("lock", 32., theme.tokens.text_normal))
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(15.))
                    .child("This PDF is password protected"),
            )
            .child(if wrong {
                div()
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.tokens.text_warning)
                    .child("Invalid password")
            } else {
                div()
                    .text_size(px(FONT_UI_SMALL))
                    .child("Enter the password below")
            })
            .child(field)
            .into_any_element()
    }
}

impl EventEmitter<PdfEvent> for PdfView {}

impl Focusable for PdfView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PdfView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        if std::mem::take(&mut self.refocus) {
            window.focus(&self.focus, cx);
        }
        self.prepare(cx);
        if self.locked.is_some() && std::mem::take(&mut self.focus_password) {
            window.focus(&self.password_focus, cx);
        }
        self.focus_password_now = self.password_focus.is_focused(window);
        // A file that will not open, or one not yet read, is the toolbar
        // over nothing: observed for a broken file ("of 0", no message).
        let sidebar = (self.sidebar && self.loaded.is_some())
            .then(|| self.render_sidebar(&theme, window, cx));
        let pages = self.render_pages(&theme, window, cx);
        let body = div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_row()
            .children(sidebar)
            .child(pages)
            .into_any_element();
        let find_bar = self.find.as_ref().map(|_| self.render_find_bar(&theme, cx));
        let menu = self.render_menu(&theme, cx);
        div()
            .track_focus(&self.focus)
            .key_context("PdfView")
            .on_action(cx.listener(Self::scroll_up))
            .on_action(cx.listener(Self::scroll_down))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::scroll_to_top))
            .on_action(cx.listener(Self::scroll_to_bottom))
            .on_action(cx.listener(Self::next_page))
            .on_action(cx.listener(Self::previous_page))
            .on_action(cx.listener(Self::find))
            .on_action(cx.listener(Self::find_next))
            .on_action(cx.listener(Self::find_previous))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::copy_selection))
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(theme.tokens.background_primary)
            .text_color(theme.tokens.text_normal)
            .child(self.render_toolbar(window, cx))
            .children(find_bar)
            .child(body)
            .children(menu)
    }
}

/// A recolouring's ground as a gpui colour.
fn theming_colour(t: Theming) -> Hsla {
    gpui::rgb(
        (u32::from(t.background[0]) << 16)
            | (u32::from(t.background[1]) << 8)
            | u32::from(t.background[2]),
    )
    .into()
}

/// A finite number for JSON, rounded to a hundredth so a scroll settles to
/// the same value twice.
fn json_number(n: f32) -> Value {
    let rounded = (f64::from(n) * 100.0).round() / 100.0;
    serde_json::Number::from_f64(if rounded.is_finite() { rounded } else { 0.0 })
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a4() -> PageSize {
        PageSize {
            width: 595.2756,
            height: 841.8898,
        }
    }

    fn landscape() -> PageSize {
        PageSize {
            width: 841.8898,
            height: 595.2756,
        }
    }

    #[test]
    fn a_zoom_step_rounds_to_a_tenth_as_pdf_js_does() {
        assert_eq!(step_in(1.0), 1.1);
        assert_eq!(step_in(1.1), 1.3, "1.21 rounds up to the next tenth");
        assert_eq!(step_out(1.0), 0.9);
        assert_eq!(step_out(0.9), 0.8, "0.818 rounds down");
        assert_eq!(step_in(9.95), MAX_SCALE);
        assert_eq!(step_out(0.1), MIN_SCALE);
        // Observed: Zoom in from page-width at 0.791 goes 0.9, 1, 1.1, 1.3,
        // 1.5, 1.7; Zoom out goes 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.1.
        let mut scale = 0.79123;
        let mut seen = Vec::new();
        for _ in 0..6 {
            scale = step_in(scale);
            seen.push(scale);
        }
        assert_eq!(seen, vec![0.9, 1.0, 1.1, 1.3, 1.5, 1.7]);
        let mut scale = 0.79123;
        let mut seen = Vec::new();
        for _ in 0..8 {
            scale = step_out(scale);
            seen.push(scale);
        }
        assert_eq!(seen, vec![0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.1]);
    }

    #[test]
    fn the_fit_zooms_are_the_observed_formulas() {
        // Observed: a 668 px wide, 672 px tall viewer fits A4's width at
        // 0.7912, and a 683 px tall one fits its height at 0.5844.
        let viewport = Size::new(668.0, 672.0);
        let sizes = [a4(), landscape(), a4()];
        let scale = Zoom::FitWidth.resolve(&sizes, 0, viewport, Spread::None);
        assert!((scale - 0.79123).abs() < 1e-4, "{scale}");
        // Set on the landscape page it fits that one: observed 0.5595.
        let scale = Zoom::FitWidth.resolve(&sizes, 1, viewport, Spread::None);
        assert!((scale - 0.55946).abs() < 1e-4, "{scale}");
        let fit_height = Zoom::FitHeight.resolve(&[a4()], 0, Size::new(668.0, 683.0), Spread::None);
        assert!((fit_height - 0.5844).abs() < 1e-4, "{fit_height}");
        assert_eq!(
            Zoom::Percent(2.5).resolve(&[a4()], 0, viewport, Spread::None),
            2.5
        );
        assert_eq!(
            Zoom::FitWidth.resolve(&[a4()], 0, Size::default(), Spread::None),
            1.0,
            "no viewport yet: 100% until there is one"
        );
        assert_eq!(
            Zoom::FitWidth.resolve(&sizes, 99, viewport, Spread::None),
            Zoom::FitWidth.resolve(&sizes, 0, viewport, Spread::None),
            "a page past the end fits the first"
        );
    }

    #[test]
    fn pages_stack_in_their_borders_and_centre_in_the_column() {
        let layout = Layout::new(&[a4(), landscape()], 1.0, 1000.0, Spread::None);
        assert_eq!(layout.pages[0].top(), PAGE_BORDER);
        assert_eq!(layout.pages[1].top(), 3.0 * PAGE_BORDER + 841.8898);
        assert_eq!(
            layout.content.height,
            4.0 * PAGE_BORDER + 841.8898 + 595.2756
        );
        assert_eq!(layout.content.width, 1000.0);
        assert_eq!(layout.pages[0].origin.x, (1000.0 - 595.2756) / 2.0);
        // A page wider than the viewport widens the column instead.
        let wide = Layout::new(&[landscape(), a4()], 1.0, 700.0, Spread::None);
        assert_eq!(wide.content.width, 841.8898 + 2.0 * PAGE_BORDER);
        assert_eq!(wide.pages[0].origin.x, PAGE_BORDER);
        assert_eq!(
            wide.pages[1].origin.x,
            (700.0 - 595.2756) / 2.0,
            "a narrow page beside a wide one still centres in the viewport"
        );
        // Observed: a link to a page lands with its border at the top.
        assert_eq!(layout.top_of(0), 0.0);
        assert_eq!(layout.top_of(1), layout.pages[1].top() - PAGE_BORDER);
    }

    /// The observed boundary cases, at the probe's fit-width scale over its
    /// five A4 pages with a landscape third: 10 px above page 3's top is
    /// page 3; 5 px above page 2's bottom is page 3; half way down page 3
    /// is page 4, because page 3 is clipped at the sides and shows less of
    /// its area than page 4 does; half way down page 2 is page 2.
    #[test]
    fn the_current_page_is_the_one_showing_most_of_itself() {
        let sizes = [a4(), a4(), landscape(), a4(), a4()];
        let viewport = Size::new(668.0, 672.0);
        let scale = Zoom::FitWidth.resolve(&sizes, 0, viewport, Spread::None) * CSS_UNITS;
        let layout = Layout::new(&sizes, scale, viewport.width, Spread::None);
        let at = |y: f32| layout.current_page(Point::new(0.0, y), viewport, 0);
        let page3_top = layout.top_of(2);
        assert_eq!(at(page3_top - 10.0), 2);
        assert_eq!(at(page3_top + 10.0), 2);
        assert_eq!(at(layout.pages[1].bottom() - 5.0), 2);
        let page3_middle = layout.pages[2].top() + layout.pages[2].size.height / 2.0;
        assert_eq!(at(page3_middle), 3);
        let page2_middle = layout.pages[1].top() + layout.pages[1].size.height / 2.0;
        assert_eq!(at(page2_middle), 1);
        assert_eq!(at(0.0), 0);
        // The page the view is on stays while it is wholly in view.
        let tall = Size::new(668.0, 3000.0);
        assert_eq!(layout.current_page(Point::new(0.0, 0.0), tall, 1), 1);
        assert_eq!(layout.current_page(Point::new(0.0, 0.0), tall, 4), 0);
    }

    #[test]
    fn locating_a_height_gives_the_page_and_the_fraction_down_it() {
        let layout = Layout::new(&[a4(), a4()], 1.0, 1000.0, Spread::None);
        let (page, frac) = layout.locate(layout.pages[1].top() + 841.8898 / 2.0);
        assert_eq!(page, 1);
        assert!((frac - 0.5).abs() < 1e-6);
        // In the border above the first page is the first page, before its
        // top.
        let (page, frac) = layout.locate(0.0);
        assert_eq!(page, 0);
        assert!(frac < 0.0);
    }

    #[test]
    fn a_zoom_round_trips_through_the_session_memory() {
        for zoom in [Zoom::FitWidth, Zoom::FitHeight] {
            assert_eq!(Zoom::from_json(&zoom.to_json()), Some(zoom));
        }
        assert_eq!(Zoom::from_json(&json!(1.5)), Some(Zoom::Percent(1.5)));
        assert_eq!(Zoom::from_json(&json!("1.5")), Some(Zoom::Percent(1.5)));
        assert_eq!(Zoom::from_json(&json!(0)), None);
        assert_eq!(Zoom::from_json(&json!("nonsense")), None);
    }

    #[test]
    fn json_numbers_are_finite_hundredths() {
        assert_eq!(json_number(1.0 / 3.0), json!(0.33));
        assert_eq!(json_number(f32::NAN), json!(0.0));
        assert_eq!(json_number(120.0), json!(120.0));
    }

    #[test]
    fn spreads_pair_pages_as_pdf_js_does() {
        assert_eq!(Spread::None.rows(3), vec![vec![0], vec![1], vec![2]]);
        assert_eq!(Spread::Odd.rows(5), vec![vec![0, 1], vec![2, 3], vec![4]]);
        assert_eq!(Spread::Even.rows(5), vec![vec![0], vec![1, 2], vec![3, 4]]);
        assert_eq!(Spread::Even.rows(0), Vec::<Vec<usize>>::new());
        assert_eq!(Spread::from_mode(Spread::Even.mode()), Spread::Even);
    }

    #[test]
    fn a_spread_fits_two_pages_across_and_centres_its_rows() {
        // Observed at 2340 px: single page-width 2.8978, a spread 1.4489.
        let viewport = Size::new(2340.0, 1443.0);
        let sizes = [a4(), a4(), landscape(), a4(), a4()];
        let one = Zoom::FitWidth.resolve(&sizes, 0, viewport, Spread::None);
        let two = Zoom::FitWidth.resolve(&sizes, 0, viewport, Spread::Odd);
        assert!((one - 2.8978).abs() < 0.001, "{one}");
        assert!((two - 1.4489).abs() < 0.001, "{two}");
        let layout = Layout::new(&sizes, two * CSS_UNITS, viewport.width, Spread::Odd);
        let (p1, p2, p3, p4, p5) = (
            layout.pages[0],
            layout.pages[1],
            layout.pages[2],
            layout.pages[3],
            layout.pages[4],
        );
        // Side by side, borders touching, on one row.
        assert_eq!(p1.top(), p2.top());
        assert!((p2.left() - (p1.right() + 2.0 * PAGE_BORDER)).abs() < 0.01);
        // The landscape page sits centred in its row beside the portrait one.
        assert!((p3.top() - p4.top() - (p4.size.height - p3.size.height) / 2.0).abs() < 0.01);
        // Page 5 alone, centred.
        let centre = (p5.left() + p5.right()) / 2.0;
        assert!((centre - viewport.width / 2.0).abs() < 0.01);
        // The point in a page's own points.
        let (page, x, y) = layout
            .page_point(
                p2.left() + 10.0 * layout.scale,
                p2.top() + 20.0 * layout.scale,
            )
            .unwrap();
        assert_eq!(page, 1);
        assert!((x - 10.0).abs() < 0.01 && (y - 20.0).abs() < 0.01);
    }

    #[test]
    fn a_collapsed_outline_item_hides_its_descendants_only() {
        let item = |title: &str, depth: usize| OutlineItem {
            title: title.into(),
            page: Some(0),
            depth,
        };
        let items = vec![
            item("One", 0),
            item("One point one", 1),
            item("Deeper", 2),
            item("Two", 0),
            item("Two point one", 1),
        ];
        let all_collapsed = PdfView::outline_visibility(&items, &HashSet::new());
        assert_eq!(
            all_collapsed,
            vec![
                (true, false),
                (false, true),
                (false, true),
                (true, false),
                (false, true)
            ]
        );
        let one_open = PdfView::outline_visibility(&items, &HashSet::from([0]));
        assert_eq!(
            one_open,
            vec![
                (true, false),
                (true, false),
                (false, true),
                (true, false),
                (false, true)
            ]
        );
    }
}
