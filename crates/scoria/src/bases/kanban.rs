//! The `kanban` layout: the engine's groups side by side, cards inside them.
//!
//! A kanban board **is** the grouping. One column per `groupBy` value, in the
//! order `groupOrder` and the `groupBy` direction put them in, with the
//! no-value group leading under an ascending `groupBy` — all of which
//! [`Table::groups`] already answers, so this layout never groups anything
//! itself. A view with no `groupBy` has one group holding every row, so such a
//! board is one column of the whole table. Each column holds one card per row
//! of its group, showing the same cells the table's row would show, with the
//! view's `limit` already applied by the engine.
//!
//! Its keys are observed, read two ways that agreed — off
//! `plugin.getRegistrations()` and off the file after driving the real
//! "Configure view" page (see [`scoria_bases::layout`] and
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//! `columnWidth` (200 to 500 step 10, default 280), `image` (a property id,
//! which the engine evaluates per row into [`Row::image`]), `imageFit` (`""`
//! Cover or `contain`) and `imageAspectRatio` (0.25 to 2.5 step 0.05, default
//! **0.5** — the one key whose default differs from the cards layout's).
//!
//! The board scrolls sideways through its columns and each column scrolls its
//! own cards, which is what makes a column a [`gpui::uniform_list`]: every
//! card of a board draws the same cells over the same image box, so they are
//! all one height ([`card_height`]) and a column with ten thousand cards costs
//! only the ones on screen. The gap is drawn inside the item, because a
//! uniform list measures one item and lays the rest out on that measurement.
//!
//! **Observed on 2026-11-01** (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`,
//! `s10-kanban`, `s20-fix-kanban`, `s26-kanban-rest`): a column is 280 px
//! with a 46 px header -- the group's property and value as a
//! `bases-group-heading`, a count, and a `plus` **Create new note** -- a
//! body of cards and a 41 px footer reading *New*; the header wears the
//! grab cursor and drags, and a column dropped elsewhere reorders the board
//! **in memory** (the file did not change) until the order is Manual. A card
//! is 8 px rounded on `#1c1c1c`, drags with `is-being-dragged` while a
//! `bases-kanban-card-drop-placeholder` of its own size stands in the target
//! column, and the drop **writes the grouped property to the note**
//! (`status: done`). Any click on a card opens its note. A right click on a
//! column header or a card raises *Add item* (`plus`) | *Hide column*
//! (`eye-off`) · *Edit groups...* (`stretch-horizontal`): *Add item*
//! creates a note with the column's value prefilled (`status: done` beside
//! the filters' keys), *Hide column* writes `groupOrder` without the
//! column, which makes the order Manual, and *Edit groups...* opens the
//! Group panel. **Unprobed**, and read here: where a dragged card lands
//! inside its new column (the sort decides on the next resolve anyway),
//! that a right click on the card's title link raises the file menu the
//! way a list line's link does, and that a column drop under a Manual order
//! writes the new `groupOrder`.
//!
//! The rest below the keys is still a reading: which cell is a card's title,
//! the card's own box, and the two scroll directions above.

use crate::assets::icon;
use crate::bases::cells::{cell_text, checkbox, image_ref, renders_as_link, ImageRef, CELL_PAD};
use crate::bases::view::BasesView;
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER, ICON_SIZE};
use gpui::{
    anchored, deferred, div, img, prelude::*, px, uniform_list, AnyElement, Context, DragMoveEvent,
    MouseButton, MouseDownEvent, ObjectFit, Pixels, Point, ScrollHandle, SharedString,
    UniformListScrollHandle, Window,
};
use scoria_bases::{CellKind, Column, ImageFit, KanbanLayout, Layout, Row, Table, Value};
use serde_json::{json, Value as Json};
use std::ops::Range;

/// The height of a column's header.
///
/// **Unprobed**: the kanban's own header was never rendered. The table's
/// header row measured 30 px
/// ([`HEADER_HEIGHT`](crate::bases::table::HEADER_HEIGHT)), and a group header
/// at the same step is what the rest of the shell reads in.
pub const COLUMN_HEADER_HEIGHT: f32 = 30.;

/// What a column with no key reads as when the view groups: Obsidian's
/// "no value for this property" group.
///
/// **Unprobed**: the word Obsidian uses was never read off a running app, in
/// the table layout either. "Empty" is what the property editor writes for a
/// blank value, and it is the same word
/// [`render_group`](crate::bases::table) draws for one, so the two layouts
/// agree.
pub const EMPTY_GROUP: &str = "Empty";

/// The narrowest a column is drawn, whatever the file asks for.
///
/// Not a clamp of `columnWidth`: the reader never clamps, and the slider's own
/// floor is 200. This is only a guard, so a hand-written `columnWidth: 0`
/// draws a narrow column rather than none at all, and 88 px is the floor the
/// table's columns were observed to have.
pub const MIN_COLUMN_WIDTH: f32 = 88.;

/// Padding inside a column, around its cards.
pub const COLUMN_PAD: f32 = 8.;

/// The gap between two columns, and between two cards.
pub const CARD_GAP: f32 = 8.;

/// The line a card's title takes, and the line each of its other cells takes.
/// **Unprobed**, like the rest of the card's box.
pub const CARD_TITLE_HEIGHT: f32 = 20.;
pub const CARD_LINE_HEIGHT: f32 = 18.;

/// The hairline around a card, which counts towards its height.
pub const CARD_BORDER: f32 = 1.;

/// The column's footer, which reads *New*: observed 41 px.
pub const COLUMN_FOOTER_HEIGHT: f32 = 41.;

/// What a card drag carries: which card, and the note it stands for.
#[derive(Clone)]
pub struct CardMove {
    pub column: usize,
    pub ix: usize,
    pub path: String,
    pub title: String,
}

impl gpui::Render for CardMove {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        div()
            .px(px(CELL_PAD))
            .py(px(4.))
            .rounded_md()
            // `.drag-reorder-ghost > .bases-kanban-card`: the card's own
            // ground and ring.
            .border_1()
            .border_color(crate::theme::ring(&theme.tokens.bases_kanban_card_shadow))
            .bg(theme.tokens.bases_kanban_card_background)
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .child(SharedString::from(self.title.clone()))
    }
}

/// What a column drag carries: which column is being moved.
#[derive(Clone)]
pub struct ColumnMove {
    pub column: usize,
}

impl gpui::Render for ColumnMove {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// The open column menu: which column, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanMenu {
    pub column: usize,
    pub position: Point<Pixels>,
}

/// One column of the board: one group of the resolved table.
#[derive(Debug, Clone, Default)]
pub struct BoardColumn {
    /// The group's key, as [`scoria_bases::Group::key`] has it; `None` is the
    /// no-value group — or the single column of a view that does not group.
    pub key: Option<String>,
    /// How many cards the column holds.
    pub rows: usize,
    /// The column scrolls its own cards.
    pub cards: UniformListScrollHandle,
}

/// What the kanban layout keeps between renders.
pub struct KanbanState {
    /// The board scrolls sideways through its columns.
    pub scroll: ScrollHandle,
    /// One per group, in the order the engine grouped them.
    pub columns: Vec<BoardColumn>,
    /// Whether the showing view has a `groupBy`, which is what decides
    /// whether a `None` key is the no-value group or just the one column of an
    /// ungrouped board.
    pub grouped: bool,
    /// How wide a column is drawn, from `columnWidth`.
    pub column_width: f32,
    /// How wide a card is drawn, which is the column less its padding.
    pub card_width: f32,
    /// How tall a card's image box is, or `None` when the view names no
    /// `image` property and a card therefore has none.
    pub image_height: Option<f32>,
    pub image_fit: ImageFit,
    /// How tall every card of this board is; see [`card_height`].
    pub card_height: f32,
    /// The columns' drawn order, as indexes into `columns`: the engine's
    /// order, until a column drag reorders the board in memory.
    pub order: Vec<usize>,
    /// The column menu, while open.
    pub menu: Option<KanbanMenu>,
    /// A card being dragged, by column and index, drawn faint.
    pub dragging_card: Option<(usize, usize)>,
    /// Where a dragged card would land: the column and the slot the
    /// placeholder stands in.
    pub card_drop: Option<(usize, usize)>,
    /// A column being dragged, drawn faint.
    pub dragging_column: Option<usize>,
    /// The slot in `order` a dragged column would land in.
    pub column_drop: Option<usize>,
    /// The group a *New* asked from a column prefills, taken by the next
    /// `take_new_item`.
    pub pending_group: Option<Option<String>>,
}

impl Default for KanbanState {
    fn default() -> KanbanState {
        let layout = KanbanLayout::default();
        let column_width = column_width(&layout);
        let card_width = card_width(column_width);
        KanbanState {
            scroll: ScrollHandle::default(),
            columns: Vec::new(),
            grouped: false,
            column_width,
            card_width,
            image_height: image_height(card_width, &layout),
            image_fit: ImageFit::default(),
            card_height: card_height(0, None),
            order: Vec::new(),
            menu: None,
            dragging_card: None,
            card_drop: None,
            dragging_column: None,
            column_drop: None,
            pending_group: None,
        }
    }
}

impl KanbanState {
    /// What the control socket reports for the kanban layout: the board's
    /// columns with their keys and row counts, and the measurements the keys
    /// decided.
    pub fn state_json(&self) -> Json {
        json!({
            "columns": self
                .columns
                .iter()
                .map(|c| json!({ "key": c.key, "rows": c.rows }))
                .collect::<Vec<_>>(),
            "grouped": self.grouped,
            "column_width": self.column_width,
            "card_width": self.card_width,
            "card_height": self.card_height,
            "image_height": self.image_height,
            "order": self.order,
            "menu": self.menu.as_ref().map(|m| json!({ "column": m.column })),
            "dragging_card": self.dragging_card,
            "card_drop": self.card_drop,
            "dragging_column": self.dragging_column,
            "column_drop": self.column_drop,
        })
    }
}

impl BasesView {
    /// Recomputes the board: its columns, and the sizes the layout keys give a
    /// card.
    ///
    /// The scroll handles are new ones, so a redraw starts the board at its
    /// first column and each column at its first card — the same choice the
    /// table layout makes when it resets its list, and for the same reason: a
    /// base is redrawn only when the file or the vault changed.
    pub(super) fn rebuild_kanban(&mut self) {
        let grouped = self.view().is_some_and(|v| v.group_by.is_some());
        let layout = match self.layout() {
            Layout::Kanban(kanban) => kanban.clone(),
            // A kanban board of another layout's keys is every default; this
            // is only reachable through the rebuild dispatch, which already
            // matched the layout.
            _ => KanbanLayout::default(),
        };
        let columns = self.table().map(board_columns).unwrap_or_default();
        let cells = self.table().map_or(0, |t| t.columns.len());
        let column_width = column_width(&layout);
        let card_width = card_width(column_width);
        let image_height = image_height(card_width, &layout);
        // A reorder made in memory survives a resolve: the old order's keys
        // are mapped onto the new columns, new keys going last.
        let old_keys: Vec<Option<String>> = self
            .kanban
            .order
            .iter()
            .filter_map(|&ix| self.kanban.columns.get(ix).map(|c| c.key.clone()))
            .collect();
        let mut order: Vec<usize> = old_keys
            .iter()
            .filter_map(|key| columns.iter().position(|c| &c.key == key))
            .collect();
        for ix in 0..columns.len() {
            if !order.contains(&ix) {
                order.push(ix);
            }
        }
        let pending_group = self.kanban.pending_group.take();
        self.kanban = KanbanState {
            scroll: ScrollHandle::default(),
            columns,
            grouped,
            column_width,
            card_width,
            image_height,
            image_fit: layout.image_fit,
            card_height: card_height(cells, image_height),
            order,
            menu: None,
            dragging_card: None,
            card_drop: None,
            dragging_column: None,
            column_drop: None,
            pending_group,
        };
    }

    // ----- what the board writes ----------------------------------------------

    /// The frontmatter key the view groups by, when it is a note property a
    /// card drop can write; a `file.*` group has no key in the note.
    fn group_key(&self) -> Option<String> {
        let property = self.view()?.group_by.as_ref()?.property.clone();
        let qualified = scoria_bases::layout::qualify(&property);
        qualified.strip_prefix("note.").map(str::to_string)
    }

    /// The keys of the columns as drawn, in the drawn order: what *Hide
    /// column* and a Manual reorder write as `groupOrder`.
    fn drawn_keys(&self) -> Vec<Option<String>> {
        self.kanban
            .order
            .iter()
            .filter_map(|&ix| self.kanban.columns.get(ix).map(|c| c.key.clone()))
            .collect()
    }

    /// A card dropped on column `to`: the grouped property of its note is
    /// set to that column's value (observed: `status: done` after a drop on
    /// *done*); the no-value column clears it. A drop on its own column
    /// writes nothing.
    pub(super) fn drop_card(&mut self, drag: &CardMove, to: usize, cx: &mut Context<Self>) {
        self.kanban.card_drop = None;
        self.kanban.dragging_card = None;
        if drag.column == to {
            cx.notify();
            return;
        }
        let (Some(key), Some(column)) = (self.group_key(), self.kanban.columns.get(to)) else {
            cx.notify();
            return;
        };
        let value = column.key.clone().unwrap_or_default();
        self.set_property(&drag.path, &key, &value, cx);
        cx.notify();
    }

    /// A column dropped at slot `to` of the drawn order: the board reorders
    /// in memory, and under a Manual order the file's `groupOrder` follows.
    pub(super) fn drop_board_column(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.kanban.column_drop = None;
        self.kanban.dragging_column = None;
        let Some(at) = self.kanban.order.iter().position(|&c| c == from) else {
            cx.notify();
            return;
        };
        let to = to.min(self.kanban.order.len());
        if at == to || at + 1 == to {
            cx.notify();
            return;
        }
        let moved = self.kanban.order.remove(at);
        let slot = if to > at { to - 1 } else { to };
        self.kanban
            .order
            .insert(slot.min(self.kanban.order.len()), moved);
        if self
            .view()
            .is_some_and(scoria_bases::layout::is_manual_group)
        {
            let keys = self.drawn_keys();
            self.edit_view(
                move |view| scoria_bases::layout::set_group_order(view, Some(keys)),
                cx,
            );
        }
        cx.notify();
    }

    /// *Hide column*: `groupOrder` written as every drawn column but this
    /// one, which also makes the group panel read Manual (observed, `s26`
    /// `hideColumn`: `groupOrder: [todo, doing]` after hiding *done*).
    fn hide_column(&mut self, column: usize, cx: &mut Context<Self>) {
        let Some(hidden) = self.kanban.columns.get(column).map(|c| c.key.clone()) else {
            return;
        };
        let keys: Vec<Option<String>> = self
            .drawn_keys()
            .into_iter()
            .filter(|k| *k != hidden)
            .collect();
        self.edit_view(
            move |view| scoria_bases::layout::set_group_order(view, Some(keys)),
            cx,
        );
    }

    /// *Add item*, the header's plus and the footer's *New*: a note with
    /// the column's value prefilled beside the filters' keys (observed:
    /// `status: done` for the *done* column).
    fn add_item_in(&mut self, column: usize, cx: &mut Context<Self>) {
        if let Some(col) = self.kanban.columns.get(column) {
            self.kanban.pending_group = Some(col.key.clone());
        }
        self.add_item(cx);
    }

    // ----- the column menu ----------------------------------------------------

    fn open_kanban_menu(&mut self, column: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.kanban.menu = Some(KanbanMenu { column, position });
        cx.notify();
    }

    pub(super) fn close_kanban_menu(&mut self, cx: &mut Context<Self>) {
        if self.kanban.menu.take().is_some() {
            cx.notify();
        }
    }

    fn kanban_menu_pick(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let Some(menu) = self.kanban.menu.take() else {
            return;
        };
        match id {
            "add-item" => self.add_item_in(menu.column, cx),
            "hide-column" => self.hide_column(menu.column, cx),
            "edit-groups" => self.show_panel(super::toolbar::Panel::Group, cx),
            _ => {}
        }
        cx.notify();
    }

    /// The column menu, observed (`s10` `columnRightClick`, `cardRightClick`):
    /// *Add item* | *Hide column* · *Edit groups...*, 134 px wide.
    fn render_kanban_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.kanban.menu.as_ref()?;
        let rows: [(&'static str, &'static str, &'static str); 3] = [
            ("add-item", "Add item", "plus"),
            ("hide-column", "Hide column", "eye-off"),
            ("edit-groups", "Edit groups...", "stretch-horizontal"),
        ];
        let mut list = div()
            .id("bases-kanban-menu")
            .flex()
            .flex_col()
            .min_w(px(134.))
            .py(px(6.))
            .bg(theme.tokens.menu_background)
            .border_1()
            .border_color(theme.tokens.menu_border_color)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_kanban_menu(cx)));
        for (ix, (id, label, icon_name)) in rows.into_iter().enumerate() {
            if ix == 1 {
                list = list.child(
                    div()
                        .my(px(6.))
                        .h(px(1.))
                        .bg(theme.tokens.background_modifier_border),
                );
            }
            list = list.child(
                div()
                    .id(("bases-kanban-menu-item", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(25.))
                    .mx(px(6.))
                    .px(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.kanban_menu_pick(id, cx)))
                    .child(icon(icon_name, 16., theme.tokens.text_muted))
                    .child(SharedString::from(label)),
            );
        }
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// Where a dragged column would land, from the pointer's x against the
    /// drawn columns' edges.
    fn column_drag_over(&mut self, pointer_x: Pixels, board_left: Pixels, cx: &mut Context<Self>) {
        let x = f32::from(pointer_x) - f32::from(board_left) - COLUMN_PAD
            + f32::from(self.kanban.scroll.offset().x).abs();
        let step = self.kanban.column_width + CARD_GAP;
        let slot = ((x + step / 2.) / step).floor().max(0.) as usize;
        let slot = slot.min(self.kanban.order.len());
        if self.kanban.column_drop != Some(slot) {
            self.kanban.column_drop = Some(slot);
            cx.notify();
        }
    }

    /// Where a dragged card would land, from the pointer against the board:
    /// the column under it by x, the slot by y against that column's cards.
    /// Computed on the board because a drag-move listener fires only for the
    /// element the drag started on, so a column cannot watch for another's
    /// card. **Unprobed**: where the card lands inside its new column; the
    /// view's sort decides on the next resolve anyway.
    fn card_drag_over(
        &mut self,
        drag: &CardMove,
        position: Point<Pixels>,
        board: gpui::Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.kanban.dragging_card != Some((drag.column, drag.ix)) {
            self.kanban.dragging_card = Some((drag.column, drag.ix));
            cx.notify();
        }
        let x = f32::from(position.x) - f32::from(board.origin.x) - COLUMN_PAD
            + f32::from(self.kanban.scroll.offset().x).abs();
        let column_step = self.kanban.column_width + CARD_GAP;
        let drawn = (x / column_step).floor().max(0.) as usize;
        let Some(&g) = self.kanban.order.get(drawn) else {
            if self.kanban.card_drop.take().is_some() {
                cx.notify();
            }
            return;
        };
        let step = self.kanban.card_height + CARD_GAP;
        let y =
            f32::from(position.y) - f32::from(board.origin.y) - COLUMN_PAD - COLUMN_HEADER_HEIGHT;
        let rows = self.kanban.columns.get(g).map_or(0, |c| c.rows);
        let slot = ((y + step / 2.) / step).floor().max(0.) as usize;
        let slot = slot.min(rows);
        if self.kanban.card_drop != Some((g, slot)) {
            self.kanban.card_drop = Some((g, slot));
            cx.notify();
        }
    }

    /// Draws the board: the columns side by side, scrolling sideways.
    pub(super) fn render_kanban(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if self.table().is_none() {
            return Vec::new();
        }
        // No column holding a card is the same nothing the table draws for a
        // view that resolved to no rows — an ungrouped view still has its one
        // group, so the count is what says the board is empty.
        if self.kanban.columns.iter().all(|column| column.rows == 0) {
            return vec![self.render_empty(theme)];
        }
        let grouped = self.kanban.grouped;
        let width = self.kanban.column_width;
        let group_property = self.view().and_then(|v| v.group_by.as_ref()).map(|g| {
            let q = scoria_bases::layout::qualify(&g.property);
            q.strip_prefix("note.").unwrap_or(&q).to_string()
        });
        let dragging_column = self.kanban.dragging_column;
        let card_drop = self.kanban.card_drop;
        let columns: Vec<AnyElement> = self
            .kanban
            .order
            .iter()
            .filter_map(|&g| self.kanban.columns.get(g).map(|c| (g, c)))
            .map(|(g, column)| {
                // A placeholder the size of a card stands where the dragged
                // card would land, so the list holds one more item there.
                let extra = usize::from(matches!(card_drop, Some((c, _)) if c == g));
                let cards = uniform_list(
                    ("bases-kanban-cards", g),
                    column.rows + extra,
                    cx.processor(move |this, range: Range<usize>, window, cx| {
                        this.render_kanban_cards(g, range, window, cx)
                    }),
                )
                .track_scroll(&column.cards)
                .size_full();
                let title = column_title(column.key.as_deref(), grouped).map(str::to_string);
                div()
                    .id(("bases-kanban-column", g))
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .w(px(width))
                    .h_full()
                    .rounded(px(8.))
                    // `.bases-kanban-column`: its own ground in a 1 px
                    // `--background-modifier-border` frame.
                    .bg(theme.tokens.bases_kanban_column_background)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .when(dragging_column == Some(g), |d| d.opacity(0.4))
                    .child(self.render_column_header(
                        g,
                        title,
                        group_property.clone(),
                        column.rows,
                        theme,
                        cx,
                    ))
                    .child(
                        div()
                            .id(("bases-kanban-body", g))
                            .flex_1()
                            .min_h(px(0.))
                            .w_full()
                            .px(px(COLUMN_PAD))
                            .on_drop(cx.listener(move |this, drag: &CardMove, _, cx| {
                                this.drop_card(drag, g, cx)
                            }))
                            .child(cards),
                    )
                    .child(
                        // The footer's *New* (observed 41 px): a note in this
                        // column's group.
                        div()
                            .id(("bases-kanban-footer", g))
                            .flex_shrink_0()
                            .h(px(COLUMN_FOOTER_HEIGHT))
                            .w_full()
                            .px(px(CELL_PAD + 4.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .text_size(px(FONT_UI_SMALL))
                            .text_color(theme.tokens.text_muted)
                            .hover(|s| s.text_color(theme.tokens.text_normal))
                            .on_click(cx.listener(move |this, _, _, cx| this.add_item_in(g, cx)))
                            .child(icon("plus", ICON_SIZE, theme.tokens.text_muted))
                            .child(SharedString::from("New")),
                    )
                    .into_any_element()
            })
            .collect();
        // An embedded board is as tall as its tallest column, so every
        // card is on the note's own scroll; a leaf's board fills the leaf
        // and its columns scroll. **Unprobed**: the run embedded a table
        // and a cards view, not a board, so the height is this build's
        // reading of the other two.
        let embedded = self.is_embedded();
        let tallest = self
            .kanban
            .columns
            .iter()
            .map(|c| c.rows as f32 * (self.kanban.card_height + CARD_GAP))
            .fold(0., f32::max);
        let board_height = COLUMN_HEADER_HEIGHT + COLUMN_FOOTER_HEIGHT + tallest + 3. * COLUMN_PAD;
        let drop_line = self.kanban.column_drop.map(|slot| {
            COLUMN_PAD + slot as f32 * (width + CARD_GAP)
                - CARD_GAP / 2.
                - 1.
                - f32::from(self.kanban.scroll.offset().x).abs()
        });
        let mut out = vec![div()
            .id("bases-kanban")
            .relative()
            .when(!embedded, |d| d.flex_1().min_h(px(0.)))
            .when(embedded, |d| d.h(px(board_height)))
            .w_full()
            .overflow_x_scroll()
            .track_scroll(&self.kanban.scroll)
            .flex()
            .flex_row()
            .items_stretch()
            .gap(px(CARD_GAP))
            .p(px(COLUMN_PAD))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<ColumnMove>, _, cx| {
                    let column = event.drag(cx).column;
                    if this.kanban.dragging_column != Some(column) {
                        this.kanban.dragging_column = Some(column);
                    }
                    this.column_drag_over(event.event.position.x, event.bounds.left(), cx);
                }),
            )
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<CardMove>, _, cx| {
                let drag = event.drag(cx).clone();
                this.card_drag_over(&drag, event.event.position, event.bounds, cx);
            }))
            .on_drop(cx.listener(|this, drag: &ColumnMove, _, cx| {
                if let Some(to) = this.kanban.column_drop {
                    this.drop_board_column(drag.column, to, cx);
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.kanban.card_drop = None;
                    this.kanban.column_drop = None;
                    this.kanban.dragging_card = None;
                    this.kanban.dragging_column = None;
                    cx.notify();
                }),
            )
            .children(columns)
            .when_some(drop_line, |d, x| {
                d.child(
                    div()
                        .absolute()
                        .top(px(COLUMN_PAD))
                        .bottom(px(COLUMN_PAD))
                        .left(px(x))
                        .w(px(2.))
                        .bg(theme.tokens.interactive_accent),
                )
            })
            .into_any_element()];
        out.extend(self.render_kanban_menu(theme, cx));
        out
    }

    /// A column's header (observed 46 px): the group's property and value,
    /// the count, and *Create new note* (`plus`) on hover; it wears the grab
    /// cursor, drags the column, and its right click is the column menu.
    #[allow(clippy::too_many_arguments)]
    fn render_column_header(
        &self,
        g: usize,
        title: Option<String>,
        property: Option<String>,
        rows: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(("bases-kanban-header", g))
            .group("bases-kanban-header")
            .flex()
            .flex_row()
            .items_center()
            .flex_shrink_0()
            .w_full()
            .h(px(COLUMN_HEADER_HEIGHT))
            .px(px(CELL_PAD))
            .gap(px(6.))
            .cursor_grab()
            .on_drag(ColumnMove { column: g }, |drag, _, _, cx| {
                cx.new(|_| drag.clone())
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_kanban_menu(g, event.position, cx)
                }),
            )
            .children(property.filter(|_| title.is_some()).map(|property| {
                div()
                    .flex_shrink_0()
                    .text_size(px(FONT_UI_SMALLER))
                    .text_color(theme.tokens.bases_group_heading_property_color)
                    .child(SharedString::from(property))
            }))
            .children(title.map(|title| {
                div()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.tokens.text_normal)
                    .child(SharedString::from(title))
            }))
            .child(
                // `.bases-kanban-column-count`: muted.
                div()
                    .flex_shrink_0()
                    .text_size(px(FONT_UI_SMALLER))
                    .text_color(theme.tokens.text_muted)
                    .child(SharedString::from(rows.to_string())),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id(("bases-kanban-create", g))
                    .flex_shrink_0()
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .opacity(0.)
                    .group_hover("bases-kanban-header", |s| s.opacity(0.85))
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .tooltip(crate::canvas::menu::tooltip("Create new note"))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.add_item_in(g, cx)
                    }))
                    .child(icon("plus", ICON_SIZE, theme.tokens.icon_color)),
            )
            .into_any_element()
    }

    /// The cards of column `g` the list asked for.
    fn render_kanban_cards(
        &mut self,
        g: usize,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = cx.global::<Theme>().clone();
        let Some(table) = self.table() else {
            return Vec::new();
        };
        let columns = table.columns.clone();
        let all = rows_of(table, g).to_vec();
        let slot = match self.kanban.card_drop {
            Some((c, slot)) if c == g => Some(slot),
            _ => None,
        };
        range
            .filter_map(|item| {
                // The placeholder takes slot `slot`; the cards after it shift
                // down by one.
                let ix = match slot {
                    Some(s) if item == s => return Some(self.render_drop_placeholder(&theme)),
                    Some(s) if item > s => item - 1,
                    _ => item,
                };
                all.get(ix)
                    .map(|row| self.render_kanban_card(g, ix, row, &columns, &theme, cx))
            })
            .collect()
    }

    /// Obsidian's `bases-kanban-card-drop-placeholder`: a card-sized box
    /// where the dragged card would land.
    fn render_drop_placeholder(&self, theme: &Theme) -> AnyElement {
        div()
            .w_full()
            .h(px(self.kanban.card_height + CARD_GAP))
            .child(
                div()
                    .w_full()
                    .h(px(self.kanban.card_height))
                    // `.bases-kanban-card-drop-placeholder`: the hover wash
                    // alone, no frame.
                    .rounded_md()
                    .bg(theme.tokens.background_modifier_hover),
            )
            .into_any_element()
    }

    /// One card: the row's image over its cells, opening the row's file.
    ///
    /// **Unprobed.** The whole box is a reading: the image full-bleed across
    /// the card's top at [`KanbanState::image_height`], the **first** cell as
    /// the card's title (the plain reading of "which cell names a card", and
    /// the one cell the table was observed to draw as a link), the rest one
    /// faint line each, and a click opening the file — which is what a table
    /// row does and the only thing a card could usefully do while the board is
    /// read-only. A line is drawn for every cell even when the cell is empty,
    /// because a card the same height as every other card is what lets the
    /// column virtualise.
    fn render_kanban_card(
        &self,
        g: usize,
        ix: usize,
        row: &Row,
        columns: &[Column],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = row.file.path.clone();
        let drag_path = row.file.path.clone();
        let menu_path = row.file.path.clone();
        let faint = self.kanban.dragging_card == Some((g, ix));
        let title = row
            .cells
            .first()
            .map(cell_text)
            // A view whose columns came out empty has nothing to name a card
            // with; the file's own base name is the only thing left, and is
            // itself **unprobed**.
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| row.file.basename.clone());
        let drag_title = title.clone();
        let mut card = div()
            .id(("bases-kanban-card", g * 65536 + ix))
            .flex()
            .flex_col()
            .w_full()
            .h(px(self.kanban.card_height))
            .overflow_hidden()
            .rounded(px(8.))
            // `.bases-kanban-card`: its ground in the
            // `--bases-kanban-card-shadow` ring and drop; under the pointer
            // the ring and the drop deepen and the ground stays.
            .border_1()
            .border_color(crate::theme::ring(&theme.tokens.bases_kanban_card_shadow))
            .shadow(drop_of(&theme.tokens.bases_kanban_card_shadow))
            .bg(theme.tokens.bases_kanban_card_background)
            .cursor_pointer()
            .when(faint, |d| d.opacity(0.4))
            .hover(|s| {
                s.border_color(crate::theme::ring(
                    &theme.tokens.bases_kanban_card_shadow_hover,
                ))
                .shadow(drop_of(&theme.tokens.bases_kanban_card_shadow_hover))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.open(path.clone(), cx)))
            // A press-and-move drags the card (`is-being-dragged`); the
            // source stays, faint, until the drop.
            .on_drag(
                CardMove {
                    column: g,
                    ix,
                    path: drag_path,
                    title: drag_title,
                },
                |drag, _, _, cx| cx.new(|_| drag.clone()),
            )
            // A right click on the card is the column menu (observed, `s10`
            // `cardRightClick`); on its title, the file menu.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_kanban_menu(g, event.position, cx)
                }),
            );
        if let Some(height) = self.kanban.image_height {
            card = card.child(self.render_kanban_image(row, height, theme));
        }
        card = card.child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .p(px(CELL_PAD))
                .child(
                    div()
                        .id(("bases-kanban-title", g * 65536 + ix))
                        .flex_shrink_0()
                        .h(px(CARD_TITLE_HEIGHT))
                        .w_full()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .text_size(px(FONT_UI_SMALL))
                        // `.bases-kanban-card-property.mod-title` makes the
                        // link colour the normal one.
                        .text_color(theme.tokens.text_normal)
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.file_menu(menu_path.clone(), event.position, cx)
                            }),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(SharedString::from(title)),
                        ),
                )
                .children(
                    row.cells
                        .iter()
                        .enumerate()
                        .skip(1)
                        .map(|(c, cell)| {
                            let line = div()
                                .flex_shrink_0()
                                .h(px(CARD_LINE_HEIGHT))
                                .w_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .overflow_hidden()
                                .text_size(px(FONT_UI_SMALLER));
                            match cell.kind {
                                // A boolean is a disabled checkbox, so there
                                // is no text even when the value is `true`.
                                CellKind::Checkbox => line.child(checkbox(
                                    matches!(cell.value, Value::Bool(true)),
                                    theme,
                                )),
                                CellKind::Number | CellKind::Text => {
                                    let link = columns
                                        .get(c)
                                        .is_some_and(|column| renders_as_link(column, cell));
                                    line.text_color(if link {
                                        theme.tokens.link_color
                                    } else {
                                        theme.tokens.text_normal
                                    })
                                    .child(
                                        div()
                                            .min_w_0()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(SharedString::from(cell_text(cell))),
                                    )
                                }
                            }
                        })
                        .collect::<Vec<_>>(),
                ),
        );
        // The gap lives inside the item: a uniform list measures one item and
        // lays the rest out on that measurement, so a margin between items is
        // not something it can see.
        div()
            .w_full()
            .h(px(self.kanban.card_height + CARD_GAP))
            .child(card)
            .into_any_element()
    }

    /// A card's image box.
    ///
    /// The box is drawn whether or not the row resolved one, so every card
    /// stays the same height. A URL is **not** drawn: [`gpui::img`] would have
    /// to fetch it over the network, which an offline app does not do, so an
    /// `image` property naming `https://…` leaves the box empty.
    fn render_kanban_image(&self, row: &Row, height: f32, theme: &Theme) -> AnyElement {
        let file = match (row.image.as_ref(), self.vault()) {
            (Some(cell), Some(vault)) => match image_ref(cell, &row.file.path, vault.as_ref()) {
                Some(ImageRef::File(path)) => Some(path),
                Some(ImageRef::Url(_)) | None => None,
            },
            _ => None,
        };
        div()
            .flex_shrink_0()
            .w_full()
            .h(px(height))
            .overflow_hidden()
            .bg(theme.tokens.bases_kanban_cover_background)
            .children(file.map(|path| {
                img(path)
                    .size_full()
                    .object_fit(match self.kanban.image_fit {
                        ImageFit::Cover => ObjectFit::Cover,
                        ImageFit::Contain => ObjectFit::Contain,
                    })
                    .into_any_element()
            }))
            .into_any_element()
    }
}

/// The board's columns: one per group, in the engine's own group order.
///
/// Nothing here orders or groups anything. `groupBy`, `groupOrder` and the
/// direction are all [`scoria_bases::table::build`]'s, and a view with no
/// `groupBy` arrives as one group of every row — so an ungrouped board is one
/// column, with the same cards a table would list. A grouped table whose
/// groups somehow came back empty falls back to one column of the flat rows,
/// the way the table's own body does, so nothing disappears.
pub fn board_columns(table: &Table) -> Vec<BoardColumn> {
    if table.groups.is_empty() {
        if table.rows.is_empty() {
            return Vec::new();
        }
        return vec![BoardColumn {
            key: None,
            rows: table.rows.len(),
            cards: UniformListScrollHandle::new(),
        }];
    }
    table
        .groups
        .iter()
        .map(|group| BoardColumn {
            key: group.key.clone(),
            rows: group.rows.len(),
            cards: UniformListScrollHandle::new(),
        })
        .collect()
}

/// The rows column `g` holds, through its group — or the flat rows under the
/// fallback [`board_columns`] makes for a table with no groups at all.
pub fn rows_of(table: &Table, g: usize) -> &[Row] {
    match table.groups.get(g) {
        Some(group) => &group.rows,
        None if g == 0 => &table.rows,
        None => &[],
    }
}

/// What a column's header says the group is.
///
/// A key is its own text. A `None` key is the no-value group when the view
/// groups — [`EMPTY_GROUP`] — and is nothing at all when it does not: the one
/// column of an ungrouped board is the whole table and has no group to name.
pub fn column_title(key: Option<&str>, grouped: bool) -> Option<&str> {
    match (key, grouped) {
        (Some(key), _) => Some(key),
        (None, true) => Some(EMPTY_GROUP),
        (None, false) => None,
    }
}

/// How wide a column is drawn, from `columnWidth`.
///
/// The key is taken as written — a hand-written value is never clamped, the
/// way the table takes `columnSize` as written — and only floored at
/// [`MIN_COLUMN_WIDTH`] so a zero or a negative draws something.
pub fn column_width(layout: &KanbanLayout) -> f32 {
    (layout.column_width as f32).max(MIN_COLUMN_WIDTH)
}

/// How wide a card is drawn: the column less the padding around its cards.
pub fn card_width(column_width: f32) -> f32 {
    (column_width - 2. * COLUMN_PAD).max(1.)
}

/// How tall a card's image box is, or `None` when the view names no `image`
/// property — a card then has no image box at all rather than an empty one.
///
/// **Unprobed**: `imageAspectRatio` is observed as a key, a range and a
/// default, but no view carrying one was ever rendered, so what the number
/// means is a reading. It is read as a ratio of **width to height**, which is
/// what the word means in CSS and what the [`cards`](crate::bases::cards)
/// layout reads the same key as, so the two layouts agree: the box is
/// `card_width / imageAspectRatio` tall, and the kanban default of 0.5 is
/// therefore a portrait box twice as tall as a card is wide — which is the
/// shape a narrow, tall column has room for, and the reason its default
/// differs from the cards layout's 1. A ratio of zero or less would have no
/// height at all and is floored.
pub fn image_height(card_width: f32, layout: &KanbanLayout) -> Option<f32> {
    layout.image.as_ref().map(|_| {
        let ratio = layout.image_aspect_ratio as f32;
        if ratio > 0. {
            (card_width / ratio).max(1.)
        } else {
            card_width
        }
    })
}

/// How tall every card of a board is.
///
/// One height for the whole board is what lets a column be a uniform list,
/// and it falls out of the card drawing the same cells for every row: the
/// image box when the view names one, then the title's line, then a line per
/// remaining cell, inside the card's padding and its hairline border — gpui
/// sizes a box the way CSS's `border-box` does, so both are part of the
/// height. A view with no columns still has a title line, which is where the
/// file's own name goes.
pub fn card_height(cells: usize, image_height: Option<f32>) -> f32 {
    let lines = cells.max(1);
    image_height.unwrap_or(0.)
        + 2. * CARD_BORDER
        + 2. * CELL_PAD
        + CARD_TITLE_HEIGHT
        + (lines - 1) as f32 * CARD_LINE_HEIGHT
}

/// The drop under a kanban card: every layer of its shadow token after the
/// first, which is the ring Scoria paints as the border.
fn drop_of(layers: &[crate::theme::value::ShadowLayer]) -> Vec<gpui::BoxShadow> {
    crate::theme::box_shadows(layers.get(1..).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_bases::{Cell, ColumnSource, FileData, Group};
    use std::sync::Arc;

    fn row(path: &str) -> Row {
        Row {
            file: Arc::new(FileData {
                path: path.to_string(),
                name: path.to_string(),
                basename: path.trim_end_matches(".md").to_string(),
                ..FileData::default()
            }),
            cells: vec![Cell {
                value: Value::Str(path.to_string()),
                error: None,
                kind: CellKind::Text,
                text: path.to_string(),
            }],
            image: None,
        }
    }

    fn table_of(rows: Vec<Row>, groups: Vec<Group>) -> Table {
        Table {
            columns: vec![Column {
                id: "file.name".to_string(),
                display: "file name".to_string(),
                source: ColumnSource::File("name".to_string()),
            }],
            rows,
            groups,
            limited: false,
            all_groups: Vec::new(),
            unknown_kind: None,
        }
    }

    fn group(key: Option<&str>, rows: Vec<Row>) -> Group {
        Group {
            key: key.map(str::to_string),
            rows,
        }
    }

    #[test]
    fn a_board_is_one_column_a_group() {
        let (a, b, c) = (row("A.md"), row("B.md"), row("C.md"));
        // The order is the engine's: the no-value group leads under an
        // ascending `groupBy`, and this layout never reorders it.
        let table = table_of(
            vec![a.clone(), b.clone(), c.clone()],
            vec![
                group(None, vec![a.clone()]),
                group(Some("todo"), vec![b.clone(), c.clone()]),
            ],
        );
        let columns = board_columns(&table);
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].key, None);
        assert_eq!(columns[0].rows, 1);
        assert_eq!(columns[1].key.as_deref(), Some("todo"));
        assert_eq!(columns[1].rows, 2);
        assert_eq!(rows_of(&table, 1).to_vec(), vec![b, c]);
    }

    #[test]
    fn a_view_with_no_group_by_is_one_column_of_every_row() {
        let rows = vec![row("A.md"), row("B.md")];
        // What the engine hands over for an ungrouped view: one group, no key.
        let table = table_of(rows.clone(), vec![group(None, rows.clone())]);
        let columns = board_columns(&table);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].rows, 2);
        assert_eq!(rows_of(&table, 0), rows.as_slice());
    }

    #[test]
    fn a_table_with_no_groups_still_puts_its_rows_in_a_column() {
        let rows = vec![row("A.md")];
        let table = table_of(rows.clone(), Vec::new());
        let columns = board_columns(&table);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].rows, 1);
        assert_eq!(rows_of(&table, 0), rows.as_slice());
        assert!(rows_of(&table, 1).is_empty());
    }

    #[test]
    fn a_view_that_resolved_to_nothing_has_no_columns() {
        let table = table_of(Vec::new(), Vec::new());
        assert!(board_columns(&table).is_empty());
        // And the empty group an ungrouped view hands over is still a column,
        // which is what the render reads the row count for.
        let table = table_of(Vec::new(), vec![group(None, Vec::new())]);
        assert_eq!(board_columns(&table).len(), 1);
        assert_eq!(board_columns(&table)[0].rows, 0);
    }

    #[test]
    fn a_null_group_reads_as_empty_only_when_the_view_groups() {
        assert_eq!(column_title(Some("todo"), true), Some("todo"));
        assert_eq!(column_title(None, true), Some(EMPTY_GROUP));
        assert_eq!(column_title(None, false), None);
        // An ungrouped board's one column is still named when the key is
        // somehow there, which only a hand-built table can do.
        assert_eq!(column_title(Some("todo"), false), Some("todo"));
    }

    #[test]
    fn a_column_is_as_wide_as_the_view_asks() {
        assert_eq!(column_width(&KanbanLayout::default()), 280.);
        let wide = KanbanLayout {
            column_width: 440.,
            ..KanbanLayout::default()
        };
        assert_eq!(column_width(&wide), 440.);
        // Taken as written past the slider's own 500, floored above zero.
        let silly = KanbanLayout {
            column_width: 4000.,
            ..KanbanLayout::default()
        };
        assert_eq!(column_width(&silly), 4000.);
        let zero = KanbanLayout {
            column_width: 0.,
            ..KanbanLayout::default()
        };
        assert_eq!(column_width(&zero), MIN_COLUMN_WIDTH);
        assert_eq!(card_width(280.), 280. - 2. * COLUMN_PAD);
    }

    #[test]
    fn a_card_reserves_an_image_box_only_when_the_view_names_one() {
        let bare = KanbanLayout::default();
        assert_eq!(image_height(264., &bare), None);
        let with_image = KanbanLayout {
            image: Some("note.cover".to_string()),
            ..KanbanLayout::default()
        };
        // The default ratio is 0.5, read as width over height, so the box is
        // twice as tall as the card is wide.
        assert_eq!(image_height(264., &with_image), Some(528.));
        let square = KanbanLayout {
            image: Some("note.cover".to_string()),
            image_aspect_ratio: 1.,
            ..KanbanLayout::default()
        };
        assert_eq!(image_height(264., &square), Some(264.));
    }

    #[test]
    fn every_card_of_a_board_is_the_same_height() {
        // One column is the title's line and nothing else, inside the card's
        // padding and its border.
        assert_eq!(
            card_height(1, None),
            2. * CARD_BORDER + 2. * CELL_PAD + CARD_TITLE_HEIGHT
        );
        // A view with no columns still has the title's line.
        assert_eq!(card_height(0, None), card_height(1, None));
        // Every further column is one more line, whether its cell has text or
        // not — which is what keeps the cards one height.
        assert_eq!(
            card_height(3, None),
            card_height(1, None) + 2. * CARD_LINE_HEIGHT
        );
        assert_eq!(card_height(1, Some(528.)), card_height(1, None) + 528.);
    }

    #[test]
    fn a_boards_state_reports_its_columns_with_their_keys_and_counts() {
        let (a, b) = (row("A.md"), row("B.md"));
        let table = table_of(
            vec![a.clone(), b.clone()],
            vec![group(None, vec![a]), group(Some("todo"), vec![b])],
        );
        let state = KanbanState {
            columns: board_columns(&table),
            grouped: true,
            ..KanbanState::default()
        };
        let json = state.state_json();
        assert_eq!(json["columns"][0]["key"], Json::Null);
        assert_eq!(json["columns"][0]["rows"], 1);
        assert_eq!(json["columns"][1]["key"], "todo");
        assert_eq!(json["columns"][1]["rows"], 1);
        assert_eq!(json["column_width"], 280.);
        assert_eq!(json["image_height"], Json::Null);
    }
}
