//! Stacked tabs (`workspace:toggle-stacked-tabs`, the tab list's "Stack
//! tabs"): a tab group drawn as Obsidian 1.15.0 draws one with
//! `mod-stacked` (`tools/obsidian-probe/results/2026-11-09-stacked-tabs.json`,
//! [[Specification#Observed stacked tabs, Obsidian 1.15.0]]).
//!
//! The strip keeps its new-tab button, tab list and sidebar toggle but loses
//! its tabs; under it every tab of the group is a pane 700 px wide behind a
//! 40 px header (the "spine": close button, type icon, the title written top
//! to bottom), all in one row that scrolls sideways. Headers and panes are
//! `position: sticky` in Obsidian, and [`Stack`] is that layout worked out
//! by hand: header *i* never leaves `[i × 40, W − (n − i) × 40]`, pane *i*
//! never goes left of `(i + 1) × 40`, and a later one paints over an
//! earlier one. Activating a tab scrolls the least that shows its pane
//! whole ([`Stack::reveal`]). `"stacked": true` is the group node's own key
//! (it rides in [`TabGroup::node`] and `tabs_node_json` puts it last).
//!
//! What Scoria draws differently: the active pane is the group's live view,
//! while every other pane draws its note through a reading view of its own
//! (Obsidian keeps an editor live in each), and a pane that is not a note
//! draws its header over an empty body. The scroll jumps rather than
//! easing over Obsidian's ~0.4 s, the stack's scroll bar is not drawn, and
//! the title is rasterized from the bundled Inter whatever the interface
//! font, gpui having no way to turn text on its side.

use super::*;
use gpui::{DispatchPhase, ScrollWheelEvent};

/// `--tab-stacked-header-width`.
pub(super) const HEADER_WIDTH: f32 = 40.;
/// `--tab-stacked-pane-width`.
pub(super) const PANE_WIDTH: f32 = 700.;
/// `--tab-stacked-font-size`.
const TITLE_SIZE: f32 = 13.;
/// The header's padding: `8px 8px 16px`.
const HEADER_PAD_TOP: f32 = 8.;
const HEADER_PAD_BOTTOM: f32 = 16.;
/// Where the title's box starts, down from the header's top: the close
/// button (20 px) and the icon (22 px) above it, measured at 54.
const TITLE_TOP: f32 = 54.;
/// The title's own `padding: 4px 0`.
const TITLE_PAD: f32 = 4.;

/// The layout of one stacked group: `count` tabs in a container `width`
/// pixels wide. Pure, so the observed numbers can be checked without a
/// window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Stack {
    pub width: f32,
    pub count: usize,
}

impl Stack {
    /// A pane's width: 700 px, held between `(W − n × 40) / n` and
    /// `W − n × 40` — the minimum winning, as CSS's does.
    pub fn pane(&self) -> f32 {
        let n = self.count.max(1) as f32;
        let max = (self.width - n * HEADER_WIDTH).max(0.);
        PANE_WIDTH.min(max).max(max / n)
    }

    /// One header and its pane.
    fn step(&self) -> f32 {
        HEADER_WIDTH + self.pane()
    }

    /// How far the row can scroll.
    pub fn max_scroll(&self) -> f32 {
        (self.count as f32 * self.step() - self.width).max(0.)
    }

    /// Header `i`'s left edge in the container at `scroll`: where it would
    /// be, held inside its sticky band (the left bound winning).
    pub fn header_x(&self, i: usize, scroll: f32) -> f32 {
        let natural = i as f32 * self.step() - scroll;
        let right = self.width - (self.count - i) as f32 * HEADER_WIDTH;
        natural.min(right).max(i as f32 * HEADER_WIDTH)
    }

    /// Pane `i`'s left edge: it sticks once it reaches the headers piled
    /// on the left, its own the last of them.
    pub fn pane_x(&self, i: usize, scroll: f32) -> f32 {
        let natural = i as f32 * self.step() + HEADER_WIDTH - scroll;
        natural.max((i + 1) as f32 * HEADER_WIDTH)
    }

    /// The scroll that shows pane `i` whole between the headers left of it
    /// (its own included) and those right of it, moving the least from
    /// `scroll`.
    pub fn reveal(&self, scroll: f32, i: usize) -> f32 {
        let pane = self.pane();
        let left = i as f32 * self.step() + HEADER_WIDTH;
        let after = (self.count - 1 - i) as f32 * HEADER_WIDTH;
        let lo = left + pane - self.width + after;
        let hi = i as f32 * pane;
        scroll.min(hi).max(lo).clamp(0., self.max_scroll())
    }
}

/// One stacked group's scroll, and the tab it last scrolled to.
#[derive(Debug, Clone, Default)]
struct StackScroll {
    offset: f32,
    /// `None` until the active tab has been revealed. A group read from
    /// `workspace.json` starts with its active tab here, as Obsidian's
    /// relaunch left the stack at 0 without revealing it.
    revealed: Option<usize>,
    /// The tab that was active, with its path, when the frame before was
    /// drawn: the pane it leaves gets its preview read again.
    active: Option<(usize, Option<String>)>,
}

/// The workspace's stacked-tab state.
#[derive(Default)]
pub(super) struct Stacks {
    scroll: HashMap<u64, StackScroll>,
    /// Each stacked group's width, measured where it is painted.
    widths: Rc<RefCell<HashMap<u64, f32>>>,
    /// The reading views drawing the panes of tabs that are not active, by
    /// group and path.
    previews: HashMap<(u64, String), Entity<ReadingView>>,
}

impl Stacks {
    fn width(&self, group: u64) -> f32 {
        self.widths.borrow().get(&group).copied().unwrap_or(0.)
    }
}

/// Whether a `tabs` node carries `"stacked": true`.
pub(super) fn is_stacked(node: &Map<String, Value>) -> bool {
    node.get("stacked").and_then(Value::as_bool) == Some(true)
}

/// Turns a `tabs` node's stacking over: `"stacked": true` on, the key gone
/// off (Obsidian never writes `false`).
pub(super) fn toggle_stacked(node: &mut Map<String, Value>) -> bool {
    if is_stacked(node) {
        node.shift_remove("stacked");
        false
    } else {
        node.insert("stacked".into(), Value::Bool(true));
        true
    }
}

impl Workspace {
    /// `workspace:toggle-stacked-tabs`: the active group stacks or
    /// unstacks; the active tab stays the active tab. A group just stacked
    /// scrolls to its active pane (inferred: every case watched already had
    /// it in view).
    pub(super) fn toggle_stacked_tabs(&mut self, cx: &mut Context<Self>) {
        let ix = self.active_group;
        let Some(group) = self.groups.get_mut(ix) else {
            return;
        };
        let id = group.id;
        if toggle_stacked(&mut group.node) {
            self.stacks.scroll.insert(id, StackScroll::default());
        } else {
            self.stacks.scroll.remove(&id);
            self.stacks.previews.retain(|(g, _), _| *g != id);
        }
        cx.notify();
    }

    /// A stacked group's scroll, width and preview panes for the control
    /// socket's state dump; `null` for a group that is not stacked.
    pub(super) fn stack_json(&self, id: u64) -> Value {
        let Some(s) = self.stacks.scroll.get(&id) else {
            return Value::Null;
        };
        let mut previews: Vec<&str> = self
            .stacks
            .previews
            .keys()
            .filter(|(g, _)| *g == id)
            .map(|(_, p)| p.as_str())
            .collect();
        previews.sort();
        json!({
            "scroll": s.offset,
            "width": self.stacks.width(id),
            "revealed": s.revealed,
            "previews": previews,
        })
    }

    /// Before a frame: each stacked group's scroll follows its active tab,
    /// and the panes of its other notes have a reading view to draw them.
    pub(super) fn sync_stacks(&mut self, cx: &mut Context<Self>) {
        let mut live: HashSet<u64> = HashSet::new();
        let mut wanted: Vec<(u64, String, bool)> = Vec::new();
        for ix in 0..self.groups.len() {
            let g = &self.groups[ix];
            if !is_stacked(&g.node) {
                continue;
            }
            let id = g.id;
            live.insert(id);
            let (tabs, active) = if ix == self.active_group {
                (self.tab_snapshots(), self.active_tab)
            } else {
                (g.tabs.clone(), g.active_tab)
            };
            let width = self.stacks.width(id);
            let stack = Stack {
                width,
                count: tabs.len(),
            };
            let entry = self.stacks.scroll.entry(id).or_insert(StackScroll {
                revealed: Some(active),
                ..StackScroll::default()
            });
            if entry.revealed != Some(active) && width > 0. {
                entry.offset = stack.reveal(entry.offset, active);
                entry.revealed = Some(active);
            }
            if width > 0. {
                entry.offset = entry.offset.clamp(0., stack.max_scroll());
            }
            // The pane the active tab just left is read again: its note
            // may have been edited while it was live.
            let left = entry
                .active
                .take()
                .filter(|(i, _)| *i != active)
                .and_then(|(_, path)| path);
            entry.active = Some((active, tabs.get(active).and_then(|t| t.path.clone())));
            for (i, tab) in tabs.iter().enumerate() {
                let Some(path) = tab.path.clone() else {
                    continue;
                };
                if i == active || FileKind::from_path(&path) != FileKind::Markdown {
                    continue;
                }
                let stale = left.as_deref() == Some(path.as_str());
                wanted.push((id, path, stale));
            }
        }
        self.stacks.scroll.retain(|id, _| live.contains(id));
        let keep: HashSet<(u64, String)> = wanted.iter().map(|(g, p, _)| (*g, p.clone())).collect();
        self.stacks.previews.retain(|k, _| keep.contains(k));
        let Some(state) = self.state.clone() else {
            return;
        };
        for (group, path, stale) in wanted {
            let key = (group, path.clone());
            if self.stacks.previews.contains_key(&key) && !stale {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(state.vault().absolute(&path)) else {
                continue;
            };
            let view = match self.stacks.previews.get(&key) {
                Some(view) => view.clone(),
                None => {
                    let view = cx.new(ReadingView::new);
                    let readable = state.app().readable_line_length.unwrap_or(true);
                    let inline_title = state.app().show_inline_title.unwrap_or(true);
                    let properties = PropertiesInDocument::from_config(
                        state.app().properties_in_document.as_deref(),
                    );
                    let fold = state.app().fold_heading.unwrap_or(true);
                    let embeds = self.embed_views.clone();
                    view.update(cx, |r, _| {
                        r.set_embed_views(embeds);
                        r.set_readable_line_length(readable);
                        r.set_show_inline_title(inline_title);
                        r.set_property_types(state.property_types().clone());
                        r.set_properties_in_document(properties);
                        r.set_fold_headings(fold);
                    });
                    self.stacks.previews.insert(key, view.clone());
                    view
                }
            };
            let state = state.clone();
            view.update(cx, |r, cx| r.set_note_later(path, text, state, cx));
        }
    }

    /// A sideways wheel over a stack scrolls it; Shift turns a vertical
    /// wheel sideways, as the browser does.
    fn scroll_stack(&mut self, group: u64, delta: f32, cx: &mut Context<Self>) {
        let ix = self.group_ix(group);
        let count = match ix {
            Some(ix) if ix == self.active_group => self.tabs.len(),
            Some(ix) => self.groups[ix].tabs.len(),
            None => return,
        };
        let stack = Stack {
            width: self.stacks.width(group),
            count,
        };
        let entry = self.stacks.scroll.entry(group).or_default();
        entry.offset = (entry.offset - delta).clamp(0., stack.max_scroll());
        cx.notify();
    }

    /// The stacked group's body: every tab's header and pane in a row.
    pub(super) fn render_stack(
        &self,
        view: &GroupView,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let id = view.id;
        let width = self.stacks.width(id);
        let scroll = self.stacks.scroll.get(&id).map(|s| s.offset).unwrap_or(0.);
        let stack = Stack {
            width,
            count: view.tabs.len(),
        };
        let pane_w = stack.pane();
        let widths = self.stacks.widths.clone();
        let weak = cx.weak_entity();
        let entity = cx.entity_id();
        let mut body = div()
            .id(("stack", id))
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.tokens.background_primary)
            // The row's width, for the next frame's layout; a change asks
            // for that frame at once. The wheel is taken in the capture
            // phase, ahead of the editor's own vertical scrolling.
            .child(
                canvas(
                    move |b, _, _| b,
                    move |b, _, window, _| {
                        let w = f32::from(b.size.width);
                        let old = widths.borrow_mut().insert(id, w);
                        if old != Some(w) {
                            window.on_next_frame(move |_, cx| cx.notify(entity));
                        }
                        let weak = weak.clone();
                        window.on_mouse_event(move |e: &ScrollWheelEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture || !b.contains(&e.position) {
                                return;
                            }
                            let d = e.delta.pixel_delta(px(20.));
                            let delta = if f32::from(d.x) != 0. {
                                f32::from(d.x)
                            } else if e.modifiers.shift {
                                f32::from(d.y)
                            } else {
                                return;
                            };
                            cx.stop_propagation();
                            weak.update(cx, |w, cx| w.scroll_stack(id, delta, cx)).ok();
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        if width <= 0. {
            return body.into_any_element();
        }
        for (i, tab) in view.tabs.iter().enumerate() {
            body = body
                .child(self.render_stack_header(view, i, tab, stack.header_x(i, scroll), theme, cx))
                .child(self.render_stack_pane(
                    view,
                    i,
                    tab,
                    stack.pane_x(i, scroll),
                    pane_w,
                    theme,
                    cx,
                ));
        }
        body.into_any_element()
    }

    /// Tab `i`'s header: a 40 px column the pane's colour, a hairline down
    /// its left edge, the close button at the top, the type icon under it,
    /// then the title read top to bottom. Every header shows its close
    /// button; the active one draws no differently (observed).
    fn render_stack_header(
        &self,
        view: &GroupView,
        i: usize,
        tab: &Tab,
        x: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let group = view.id;
        let t = &theme.tokens;
        let kind_icon = tab
            .path
            .as_deref()
            .map(leaf_icon)
            .unwrap_or("lucide-file")
            .trim_start_matches("lucide-");
        let pinned = tab.pinned;
        let title = tab.title();
        let color = t.text_normal;
        div()
            .id(("stack-header", i))
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(x))
            .w(px(HEADER_WIDTH))
            .bg(t.tab_background_active)
            .occlude()
            // `box-shadow: -1px 0 0 var(--tab-outline-color),
            // var(--tab-stacked-shadow)`.
            .shadow(vec![
                BoxShadow {
                    color: t.tab_outline_color,
                    blur_radius: px(0.),
                    spread_radius: px(0.),
                    offset: point(px(-1.), px(0.)),
                },
                BoxShadow {
                    color: gpui::hsla(0., 0., 0., 0.05),
                    blur_radius: px(8.),
                    spread_radius: px(0.),
                    offset: point(px(-8.), px(0.)),
                },
            ])
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_group(group, cx);
                this.activate_tab(i, cx);
            }))
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |this, _, _, cx| {
                    this.activate_group(group, cx);
                    this.close_tab(i, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_tab_menu(group, i, e.position, cx)
                }),
            )
            .child(
                div()
                    .id(("stack-close", i))
                    .group("stack-close")
                    .absolute()
                    .top(px(HEADER_PAD_TOP))
                    .left(px((HEADER_WIDTH - 20.) / 2.))
                    .w(px(20.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.activate_group(group, cx);
                        if pinned {
                            this.activate_tab(i, cx);
                            this.toggle_pin(cx);
                        } else {
                            this.close_tab(i, cx);
                        }
                    }))
                    .child(
                        icon(
                            if pinned { "pin" } else { "x" },
                            16.,
                            t.tab_text_color_focused_active,
                        )
                        .group_hover("stack-close", |s| s.text_color(t.text_normal)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(HEADER_PAD_TOP + 22.))
                    .left(px((HEADER_WIDTH - 22.) / 2.))
                    .w(px(22.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(kind_icon, 14., t.icon_color)),
            )
            .child(vertical_title(title, color))
    }

    /// Tab `i`'s pane: the group's live view for the active tab, a reading
    /// view of the note for the others, each under its own view header. A
    /// press anywhere on another pane makes its tab the active one first.
    #[allow(clippy::too_many_arguments)]
    fn render_stack_pane(
        &self,
        view: &GroupView,
        i: usize,
        tab: &Tab,
        x: f32,
        width: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let group = view.id;
        let active = i == view.active_tab;
        let pane = div()
            .id(("stack-pane", i))
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(x))
            .w(px(width))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.tokens.background_primary)
            .occlude();
        if active {
            return pane.child(self.render_header(view, theme, cx)).child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_group_main(view, theme, cx))
                    .when(self.dragging_tab, |d| {
                        d.child(self.render_drop_zones(group, theme, cx))
                    }),
            );
        }
        let pane_view = GroupView {
            id: group,
            active: false,
            tabs: view.tabs.clone(),
            active_tab: i,
            current: tab.path.clone(),
            mode: tab.mode,
            history_len: tab.history.len(),
            history_ix: tab.history_ix,
            detached: None,
            stacked: true,
            editor: view.editor.clone(),
            reading: view.reading.clone(),
            canvas: view.canvas.clone(),
            bases: view.bases.clone(),
            pdf: view.pdf.clone(),
        };
        let preview = tab
            .path
            .as_ref()
            .and_then(|p| self.stacks.previews.get(&(group, p.clone())))
            .cloned();
        pane.capture_any_mouse_down(cx.listener(move |this, _, _, cx| {
            this.activate_group(group, cx);
            this.activate_tab(i, cx);
        }))
        .child(self.render_header(&pane_view, theme, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .children(preview.map(|p| p.into_any_element())),
        )
    }
}

/// The header's title: drawn top to bottom with each glyph turned a
/// quarter clockwise (`writing-mode: vertical-lr`), 13 px, ellipsized
/// where the header's padding begins.
fn vertical_title(title: SharedString, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let scale = window.scale_factor();
            let room = f32::from(b.size.height) - TITLE_TOP - HEADER_PAD_BOTTOM - 2. * TITLE_PAD;
            if room <= 0. {
                return;
            }
            let rgba: gpui::Rgba = color.into();
            let key = TitleKey {
                text: title.to_string(),
                size_16: (TITLE_SIZE * scale * 16.).round() as u32,
                room: (room * scale).floor() as u32,
                color: u32::from_be_bytes([
                    (rgba.r * 255.).round() as u8,
                    (rgba.g * 255.).round() as u8,
                    (rgba.b * 255.).round() as u8,
                    (rgba.a * 255.).round() as u8,
                ]),
            };
            let Some(image) = title_image(&key) else {
                return;
            };
            let w = image.width as f32 / scale;
            let h = image.height as f32 / scale;
            let left = (f32::from(b.origin.x) + (HEADER_WIDTH - w) / 2.).round();
            let top = f32::from(b.origin.y) + TITLE_TOP + TITLE_PAD;
            let bounds = Bounds::new(point(px(left), px(top)), gpui::size(px(w), px(h)));
            let _ = window.paint_image(
                bounds,
                gpui::Corners::default(),
                image.image.clone(),
                0,
                false,
            );
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct TitleKey {
    text: String,
    /// Device pixels per em, in sixteenths.
    size_16: u32,
    /// Device pixels of room down the header.
    room: u32,
    color: u32,
}

struct TitleImage {
    image: Arc<gpui::RenderImage>,
    width: u32,
    height: u32,
}

fn title_image(key: &TitleKey) -> Option<Arc<TitleImage>> {
    type Cache = HashMap<TitleKey, Option<Arc<TitleImage>>>;
    thread_local! {
        static TITLES: RefCell<Cache> = RefCell::new(HashMap::new());
    }
    if let Some(hit) = TITLES.with(|t| t.borrow().get(key).cloned()) {
        return hit;
    }
    let built = rasterize_title(&key.text, key.size_16 as f32 / 16., key.room as f32).map(|t| {
        let [r, g, b, a] = key.color.to_be_bytes();
        let mut bytes = Vec::with_capacity(t.coverage.len() * 4);
        for &c in &t.coverage {
            bytes.extend_from_slice(&[b, g, r, ((c as u32 * a as u32) / 255) as u8]);
        }
        let buffer = image::RgbaImage::from_raw(t.width, t.height, bytes)
            .expect("the buffer is width * height * 4 bytes");
        Arc::new(TitleImage {
            image: Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buffer)])),
            width: t.width,
            height: t.height,
        })
    });
    TITLES.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() > 512 {
            t.clear();
        }
        t.insert(key.clone(), built.clone());
    });
    built
}

/// A coverage bitmap, one byte a pixel, row by row.
pub(super) struct Coverage {
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
}

/// `text` set on one line in Inter Regular at `size` device pixels, cut to
/// fit `room` device pixels with an ellipsis, and turned a quarter
/// clockwise: the line's start at the top, the glyphs' tops facing right.
pub(super) fn rasterize_title(text: &str, size: f32, room: f32) -> Option<Coverage> {
    use ab_glyph::{Font, FontRef, ScaleFont};
    let font = FontRef::try_from_slice(crate::assets::INTER_REGULAR).ok()?;
    let scaled = font.as_scaled(size);
    let advance = |s: &str| -> f32 {
        let mut x = 0.;
        let mut prev = None;
        for ch in s.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                x += scaled.kern(p, id);
            }
            x += scaled.h_advance(id);
            prev = Some(id);
        }
        x
    };
    let mut line: String = text.to_string();
    if advance(&line) > room {
        let mut chars: Vec<char> = text.chars().collect();
        loop {
            chars.pop();
            let candidate: String = chars.iter().collect::<String>().trim_end().to_string() + "…";
            if chars.is_empty() || advance(&candidate) <= room {
                line = candidate;
                break;
            }
        }
    }
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let w = advance(&line).ceil() as u32 + 2;
    let h = (ascent - descent).ceil() as u32 + 2;
    if w <= 2 {
        return None;
    }
    let mut flat = vec![0u8; (w * h) as usize];
    let mut x = 1.;
    let mut prev = None;
    for ch in line.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            x += scaled.kern(p, id);
        }
        let glyph = id.with_scale_and_position(size, ab_glyph::point(x, 1. + ascent));
        x += scaled.h_advance(id);
        prev = Some(id);
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bb = outlined.px_bounds();
            outlined.draw(|gx, gy, c| {
                let px_ = bb.min.x as i32 + gx as i32;
                let py = bb.min.y as i32 + gy as i32;
                if px_ >= 0 && py >= 0 && (px_ as u32) < w && (py as u32) < h {
                    let at = (py as u32 * w + px_ as u32) as usize;
                    flat[at] = flat[at].saturating_add((c * 255.).round() as u8);
                }
            });
        }
    }
    // A quarter turn clockwise: the new image is `h` wide and `w` tall,
    // and its pixel (x', y') is the line's (y', h − 1 − x').
    let mut turned = vec![0u8; (w * h) as usize];
    for ny in 0..w {
        for nx in 0..h {
            turned[(ny * h + nx) as usize] = flat[((h - 1 - nx) * w + ny) as usize];
        }
    }
    Some(Coverage {
        width: h,
        height: w,
        coverage: turned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The six-tab stack of the probe's `s02`: a 1552 px container, each
    /// tab activated in turn from scroll 0 (`2026-11-09-stacked-tabs.json`).
    #[test]
    fn six_tabs_reveal_where_obsidian_scrolled() {
        let s = Stack {
            width: 1552.,
            count: 6,
        };
        assert_eq!(s.pane(), 700.);
        assert_eq!(s.max_scroll(), 2888.);
        let mut scroll = 0.;
        let mut seen = Vec::new();
        for i in 0..6 {
            scroll = s.reveal(scroll, i);
            seen.push(scroll);
        }
        assert_eq!(seen, [0., 88., 788., 1488., 2188., 2888.]);
        // From 2188 a click on the first header went back to 0; from 0 one
        // on the fifth went to 2188 (`s03`).
        assert_eq!(s.reveal(2188., 0), 0.);
        assert_eq!(s.reveal(0., 4), 2188.);
    }

    /// The headers' and panes' left edges as measured, less the container's
    /// own x (44), with the third tab active at scroll 788.
    #[test]
    fn sticky_edges_match_the_measured_rects() {
        let s = Stack {
            width: 1552.,
            count: 6,
        };
        let headers: Vec<f32> = (0..6).map(|i| s.header_x(i, 788.) + 44.).collect();
        assert_eq!(headers, [44., 84., 736., 1476., 1516., 1556.]);
        let panes: Vec<f32> = (0..6).map(|i| s.pane_x(i, 788.) + 44.).collect();
        assert_eq!(panes, [84., 124., 776., 1516., 2256., 2996.]);
        // At scroll 0 with the first tab active.
        let headers: Vec<f32> = (0..6).map(|i| s.header_x(i, 0.) + 44.).collect();
        assert_eq!(headers, [44., 784., 1436., 1476., 1516., 1556.]);
    }

    /// The pane's clamp: `(W − n × 40) / n` to `W − n × 40`, read as each
    /// leaf's computed `min-width` and `max-width`.
    #[test]
    fn pane_width_is_clamped_as_measured() {
        let at = |width: f32, count: usize| Stack { width, count }.pane();
        assert_eq!(at(1552., 3), 700.);
        assert_eq!(at(1552., 7), 700.);
        // Narrower than a pane and its headers: the maximum.
        assert_eq!(at(600., 3), 480.);
        // The minimum is the maximum's n-th share, so it never binds.
        assert_eq!(at(400., 7), 120.);
        // Three tabs at 1552: the content is 3 × 740.
        let s = Stack {
            width: 1552.,
            count: 3,
        };
        assert_eq!(s.max_scroll(), 2220. - 1552.);
        assert_eq!(s.reveal(0., 1), 0., "Beta already in view (s01)");
        assert_eq!(s.reveal(0., 2), 668., "next-tab to Gamma (s04)");
    }

    /// Four tabs, a new one active at the end: 1408 (`s04` `newTab`).
    #[test]
    fn a_new_tab_at_the_end_is_revealed() {
        let s = Stack {
            width: 1552.,
            count: 4,
        };
        assert_eq!(s.reveal(0., 3), 1408.);
    }

    #[test]
    fn stacked_key_turns_on_and_is_removed_off() {
        let mut node: Map<String, Value> =
            serde_json::from_str(r#"{"id": "6c7edd019a3a8ae9", "type": "tabs"}"#).unwrap();
        assert!(!is_stacked(&node));
        assert!(toggle_stacked(&mut node));
        assert!(is_stacked(&node));
        assert!(!toggle_stacked(&mut node));
        assert!(!node.contains_key("stacked"), "no `false` is written");
    }

    /// The probe's `s01` bytes: three leaves, the second active, stacked.
    /// `"stacked": true` lands after `currentTab`; read back and written
    /// again it stays there; unstacked, the node is what it was.
    #[test]
    fn stacked_group_is_written_in_obsidians_order() {
        let mut node: Map<String, Value> =
            serde_json::from_str(r#"{"id": "6c7edd019a3a8ae9", "type": "tabs"}"#).unwrap();
        let leaves = vec![json!({"id": "a"}), json!({"id": "b"}), json!({"id": "c"})];
        let plain = tabs_node_json(node.clone(), leaves.clone(), 1);
        toggle_stacked(&mut node);
        let stacked = tabs_node_json(node.clone(), leaves.clone(), 1);
        let keys: Vec<&str> = stacked
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["id", "type", "children", "currentTab", "stacked"]);
        assert_eq!(stacked["stacked"], json!(true));
        // A node read from that file keeps `stacked` among its own keys,
        // before `children`; written again it goes back to the end.
        let mut read = stacked.as_object().unwrap().clone();
        read.shift_remove("children");
        read.shift_remove("currentTab");
        let again = tabs_node_json(read, leaves.clone(), 1);
        assert_eq!(
            serde_json::to_string(&again).unwrap(),
            serde_json::to_string(&stacked).unwrap()
        );
        // The first tab active: no `currentTab`, `stacked` after `children`.
        let first = tabs_node_json(node.clone(), leaves.clone(), 0);
        let keys: Vec<&str> = first
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["id", "type", "children", "stacked"]);
        toggle_stacked(&mut node);
        assert_eq!(tabs_node_json(node, leaves, 1), plain);
    }

    /// The stacked group of the probe's `s01`, read and written back: the
    /// same bytes, `"stacked": true` last.
    #[test]
    fn a_stacked_group_round_trips_obsidians_bytes() {
        let leaf = |id: &str, name: &str| {
            format!(
                r#"    {{
      "id": "{id}",
      "type": "leaf",
      "state": {{
        "type": "markdown",
        "state": {{
          "file": "{name}.md",
          "mode": "source",
          "source": false
        }},
        "icon": "lucide-file",
        "title": "{name}"
      }}
    }}"#
            )
        };
        let probed = format!(
            "{{\n  \"id\": \"6c7edd019a3a8ae9\",\n  \"type\": \"tabs\",\n  \"children\": [\n{},\n{},\n{}\n  ],\n  \"currentTab\": 1,\n  \"stacked\": true\n}}",
            leaf("f36c7ad4bb07ab0e", "Alpha"),
            leaf("2ae0c57159918b87", "Beta"),
            leaf("8955bccdbe8abb0c", "Gamma"),
        );
        let read: Value = serde_json::from_str(&probed).unwrap();
        let known = |_: &str| true;
        let Some(LayoutSpec::Group(spec)) = read_main(&read, ViewMode::LivePreview, &known) else {
            panic!("a tabs node");
        };
        assert!(is_stacked(&spec.node));
        assert_eq!(spec.active_tab, 1);
        let leaves: Vec<Value> = spec.tabs.iter().map(|(_, t)| leaf_json(t)).collect();
        let written = tabs_node_json(spec.node.clone(), leaves, spec.active_tab);
        assert_eq!(serde_json::to_string_pretty(&written).unwrap(), probed);
    }

    /// The title turned on its side: taller than wide, with ink in it, and
    /// cut with an ellipsis to the room it is given.
    #[test]
    fn the_title_is_turned_and_cut_to_its_room() {
        let full = rasterize_title("Alpha", 13., 1000.).unwrap();
        assert!(full.height > full.width, "{}x{}", full.width, full.height);
        assert!(full.coverage.iter().any(|&c| c > 128));
        let long = "A rather long note title for the stacked spine";
        let cut = rasterize_title(long, 13., 100.).unwrap();
        assert!(cut.height <= 100 + 2, "{}", cut.height);
        let whole = rasterize_title(long, 13., 10_000.).unwrap();
        assert!(whole.height > cut.height);
    }
}
