//! The views panel and its **Configure view** page.
//!
//! Observed (`2026-11-01-bases-ui.json`, steps `s03-view-switcher`,
//! `s07-view-config-per-layout` and `s15-fix-views`): the panel opens with
//! a `Search...` field that narrows the rows live, one row per view — the
//! layout's icon, the name in a medium weight, a `chevron-right` gear at the
//! right — the showing view washed, then a bordered group with **Add view**
//! (`plus`). A click on a row shows that view; a click on its gear swaps the
//! panel's body for the view's Configure page: a header of `chevron-left`
//! **Configure view**, a `more-vertical` menu and an `x`; a **View name**
//! input; the **Layout** combobox (Table / Cards / List / Kanban with their
//! icons); then the layout's own rows — **Row height** for a table; **Card
//! size**, **Image property**, **Image fit** and **Image aspect ratio** for
//! cards; **Markers**, **Indent properties** and **Property separator** for a
//! list; **Column width** and the three image rows for a kanban. The header's
//! menu is **Default view** (`layers-2`, checked), **Duplicate view**
//! (`files`), a separator, and **Delete view** (`trash-2`, warning).
//!
//! **Add view** opens that page with the name `View` prefilled and writes
//! `- type: table` / `name: View` — `View`, however many views the base
//! already has, the fifth included.
//!
//! Obsidian 1.14.5 marks *Table* as the Layout combobox's picked row whatever
//! the view's real layout is; that is a bug the run recorded as one, so the
//! real layout is marked here. **Indent properties** is not drawn: the key
//! the toggle writes was never observed (the toggle produced no write in
//! either run), and nothing should write a guessed key.

use crate::assets::icon;
use crate::bases::toolbar::{default_view, key, ComboOption, ComboSpec, Field, Page, ROW_HEIGHT};
use crate::bases::view::BasesView;
use crate::theme::{Theme, FONT_UI_SMALL, ICON_SIZE};
use gpui::{
    anchored, deferred, div, prelude::*, px, Anchor, AnyElement, Context, Point, SharedString,
    Window,
};
use scoria_bases::layout::{self, ImageFit, Layout, Markers, RowHeight, LAYOUTS};
use scoria_formats::base::{Base, View, DEFAULT_VIEW_NAME, DEFAULT_VIEW_TYPE};
use serde_json::json;

/// `--text-error`: the colour of a menu's `is-warning` row.
pub(super) const WARNING: gpui::Rgba = gpui::Rgba {
    r: 0xe9 as f32 / 255.,
    g: 0x31 as f32 / 255.,
    b: 0x47 as f32 / 255.,
    a: 1.,
};

impl BasesView {
    /// The rows the panel lists: every view, narrowed by the search field.
    fn views_rows(&self, base: &Base, cx: &Context<Self>) -> Vec<(usize, View)> {
        let views: Vec<View> = if base.views.is_empty() {
            vec![default_view()]
        } else {
            base.views.clone()
        };
        let query = match &self.toolbar.editing {
            Some(Field::Search) => self.field_text(cx).trim().to_ascii_lowercase(),
            _ => String::new(),
        };
        views
            .into_iter()
            .enumerate()
            .filter(|(_, v)| query.is_empty() || v.name.to_ascii_lowercase().contains(&query))
            .collect()
    }

    /// How many rows the arrows cycle over: the views, then **Add view**.
    pub(super) fn views_list_len(&self, cx: &Context<Self>) -> usize {
        let Some(base) = self.base() else {
            return 0;
        };
        self.views_rows(base, cx).len() + 1
    }

    /// Enter in the search field: shows the selected view, or adds one.
    pub(super) fn views_activate_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = self.base().cloned() else {
            return;
        };
        let rows = self.views_rows(&base, cx);
        let selected = self.toolbar.selected.unwrap_or(0);
        match rows.get(selected) {
            Some((ix, _)) => {
                let ix = *ix;
                self.show_view(ix, cx);
                self.close_panel(window, cx);
            }
            None => self.add_view_from_panel(cx),
        }
    }

    /// **Add view**: appends a view named `View` and opens its page.
    pub(super) fn add_view_from_panel(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.add_view(cx) else {
            return;
        };
        self.configure_view(ix, cx);
    }

    /// The panel: its list, or the page a gear opened.
    pub(super) fn render_views_panel(
        &mut self,
        base: &Base,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Page::Configure(ix) = self.toolbar.page {
            return self.render_configure(base, ix, theme, cx);
        }
        let rows = self.views_rows(base, cx);
        self.toolbar.set_rows(
            rows.iter()
                .map(|(_, v)| json!({ "name": v.name, "type": v.kind }))
                .collect(),
        );
        let showing = self.view_ix();
        let selected = self.toolbar.selected;
        let list: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(row, (ix, view))| {
                let ix = *ix;
                let icon_name = layout::kind_icon(&view.kind).unwrap_or("file-question");
                div()
                    .id(("bases-view-row", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .pl(px(6.))
                    .pr(px(4.))
                    .py(px(4.))
                    .mb(px(1.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .when(ix == showing || selected == Some(row), |d| {
                        d.bg(theme.bg_alt)
                    })
                    .hover(|s| s.bg(theme.bg_alt))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.show_view(ix, cx);
                        this.close_panel(window, cx);
                    }))
                    .child(icon(icon_name, ICON_SIZE, theme.text))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(SharedString::from(view.name.clone())),
                    )
                    .child(
                        div()
                            .id(("bases-view-configure", ix))
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .hover(|s| s.bg(theme.bg_active))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.show_view(ix, cx);
                                this.configure_view(ix, cx);
                            }))
                            .child(icon("chevron-right", ICON_SIZE, theme.text_muted)),
                    )
                    .into_any_element()
            })
            .collect();
        let add_selected = selected == Some(rows.len());
        let add = self.action_row(
            "bases-view-add",
            0,
            "plus",
            SharedString::from("Add view"),
            true,
            theme,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.add_view_from_panel(cx);
            }),
        );
        let add = if add_selected {
            div()
                .bg(theme.bg_alt)
                .rounded(px(4.))
                .child(add)
                .into_any_element()
        } else {
            add
        };
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.search_box(Field::Search, "Search...", theme, cx))
            .child(self.group(list, false, theme))
            .child(self.group(vec![add], true, theme))
            .into_any_element()
    }

    /// The Configure view page for the view at `ix`.
    fn render_configure(
        &mut self,
        base: &Base,
        ix: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = base.views.get(ix).cloned().unwrap_or_else(default_view);
        let kind = view.kind.clone();
        let selected_layout = LAYOUTS.iter().position(|(k, ..)| *k == kind);
        let shown_layout = layout::kind_label(&kind)
            .map(SharedString::from)
            .unwrap_or_else(|| SharedString::from(kind.clone()));
        self.toolbar
            .set_rows(vec![json!({ "control": "Layout", "value": kind })]);
        let name_now = match &self.toolbar.editing {
            Some(Field::ViewName) => self.field_text(cx),
            _ => view.name.clone(),
        };
        let mut rows: Vec<AnyElement> = vec![
            self.input_row(
                None,
                self.text_box(Field::ViewName, &name_now, "View name", theme, cx),
                theme,
            ),
            self.input_row(
                Some("Layout"),
                self.combobox(
                    ComboSpec {
                        icon: Some(layout::kind_icon(&kind).unwrap_or("file-question")),
                        shown: Some(shown_layout),
                        options: self.layout_options(),
                        selected: selected_layout,
                        ..ComboSpec::new("layout", 0)
                    },
                    theme,
                    cx,
                ),
                theme,
            ),
        ];
        match Layout::of(&view) {
            Layout::Table(table) => {
                let now = RowHeight::all().iter().position(|h| *h == table.row_height);
                self.toolbar
                    .record(json!({ "control": "rowHeight", "value": table.row_height.key() }));
                rows.push(
                    self.input_row(
                        Some("Row height"),
                        self.select(
                            "rowHeight",
                            SharedString::from(table.row_height.label()),
                            RowHeight::all()
                                .iter()
                                .map(|h| ComboOption::plain(h.label()))
                                .collect(),
                            now,
                            theme,
                            cx,
                        ),
                        theme,
                    ),
                );
            }
            Layout::Cards(cards) => {
                self.toolbar
                    .record(json!({ "control": "cardSize", "value": cards.card_size }));
                rows.push(self.input_row(
                    Some("Card size"),
                    self.slider(
                        "cardSize",
                        cards.card_size,
                        layout::CARD_SIZE_RANGE,
                        0,
                        theme,
                        cx,
                    ),
                    theme,
                ));
                rows.push(self.image_row(base, &view, cards.image.as_deref(), theme, cx));
                rows.push(self.image_fit_row(cards.image_fit, theme, cx));
                self.toolbar.record(
                    json!({ "control": "imageAspectRatio", "value": cards.image_aspect_ratio }),
                );
                rows.push(self.input_row(
                    Some("Image aspect ratio"),
                    self.slider(
                        "imageAspectRatio",
                        cards.image_aspect_ratio,
                        layout::ASPECT_RANGE,
                        2,
                        theme,
                        cx,
                    ),
                    theme,
                ));
            }
            Layout::Kanban(kanban) => {
                self.toolbar
                    .record(json!({ "control": "columnWidth", "value": kanban.column_width }));
                rows.push(self.input_row(
                    Some("Column width"),
                    self.slider(
                        "columnWidth",
                        kanban.column_width,
                        layout::COLUMN_WIDTH_RANGE,
                        0,
                        theme,
                        cx,
                    ),
                    theme,
                ));
                rows.push(self.image_row(base, &view, kanban.image.as_deref(), theme, cx));
                rows.push(self.image_fit_row(kanban.image_fit, theme, cx));
                self.toolbar.record(
                    json!({ "control": "imageAspectRatio", "value": kanban.image_aspect_ratio }),
                );
                rows.push(self.input_row(
                    Some("Image aspect ratio"),
                    self.slider(
                        "imageAspectRatio",
                        kanban.image_aspect_ratio,
                        layout::ASPECT_RANGE,
                        2,
                        theme,
                        cx,
                    ),
                    theme,
                ));
            }
            Layout::List(list) => {
                let now = Markers::all().iter().position(|m| *m == list.markers);
                self.toolbar
                    .record(json!({ "control": "markers", "value": list.markers.key() }));
                rows.push(
                    self.input_row(
                        Some("Markers"),
                        self.select(
                            "markers",
                            SharedString::from(list.markers.label()),
                            Markers::all()
                                .iter()
                                .map(|m| ComboOption::plain(m.label()))
                                .collect(),
                            now,
                            theme,
                            cx,
                        ),
                        theme,
                    ),
                );
                self.toolbar
                    .record(json!({ "control": "separator", "value": list.separator }));
                let separator = match &self.toolbar.editing {
                    Some(Field::Separator) => self.field_text(cx),
                    _ => list.separator.clone(),
                };
                rows.push(self.input_row(
                    Some("Property separator"),
                    self.text_box(Field::Separator, &separator, "", theme, cx),
                    theme,
                ));
            }
            // An unknown type never reaches the toolbar; the shell draws the
            // error block instead.
            Layout::Unknown(_) => {}
        }
        let menu = self.render_page_menu(ix, theme, cx);
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.page_header(SharedString::from("Configure view"), true, theme, cx))
            .child(self.page_form(rows))
            .children(menu)
            .into_any_element()
    }

    /// The **Image property** row: a clearable combobox over `file` and every
    /// property, writing a qualified id.
    fn image_row(
        &self,
        base: &Base,
        view: &View,
        current: Option<&str>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (ids, options) = self.image_options(base, view);
        let selected = current.and_then(|id| {
            ids.iter()
                .position(|p| scoria_bases::layout::same_property(p, id))
        });
        let shown = selected.map(|i| options[i].label.clone());
        let icon_name = selected.and_then(|i| options[i].icon);
        self.input_row(
            Some("Image property"),
            self.combobox(
                ComboSpec {
                    icon: icon_name,
                    shown,
                    options,
                    selected,
                    clearable: true,
                    ..ComboSpec::new("image", 0)
                },
                theme,
                cx,
            ),
            theme,
        )
    }

    fn image_fit_row(&self, fit: ImageFit, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let now = match fit {
            ImageFit::Cover => 0,
            ImageFit::Contain => 1,
        };
        let options = vec![ComboOption::plain("Cover"), ComboOption::plain("Contain")];
        self.input_row(
            Some("Image fit"),
            self.select(
                "imageFit",
                options[now].label.clone(),
                options,
                Some(now),
                theme,
                cx,
            ),
            theme,
        )
    }

    /// A `<select>`: a combobox with no icon and no search field.
    pub(super) fn select(
        &self,
        name: &'static str,
        shown: SharedString,
        options: Vec<ComboOption>,
        selected: Option<usize>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.combobox(
            ComboSpec {
                shown: Some(shown),
                options,
                selected,
                searchable: false,
                ..ComboSpec::new(name, 0)
            },
            theme,
            cx,
        )
    }

    /// The Layout combobox's rows.
    pub(super) fn layout_options(&self) -> Vec<ComboOption> {
        LAYOUTS
            .iter()
            .map(|(_, label, icon_name)| ComboOption {
                icon: Some(icon_name),
                label: SharedString::from(*label),
                aux: None,
            })
            .collect()
    }

    /// The **Image property** combobox's rows: `file`, then every formula
    /// and note property (the `file.*` properties are not offered).
    pub(super) fn image_options(
        &self,
        base: &Base,
        view: &View,
    ) -> (Vec<String>, Vec<ComboOption>) {
        let mut ids = vec!["file".to_string()];
        let mut options = vec![ComboOption {
            icon: Some("file"),
            label: SharedString::from("file"),
            aux: None,
        }];
        let props: Vec<_> = self
            .properties_of(base, view)
            .into_iter()
            .filter(|p| !p.id.starts_with("file."))
            .collect();
        options.extend(self.property_options(&props));
        ids.extend(props.into_iter().map(|p| p.id));
        (ids, options)
    }

    /// The page header's menu, hung under its `more-vertical` button.
    fn render_page_menu(
        &self,
        ix: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.toolbar.header_menu {
            return None;
        }
        let at = self
            .anchor(key("bases-page-menu", 0))
            .map(|b| Point::new(b.origin.x, b.origin.y + b.size.height + px(2.)))
            .unwrap_or_default();
        let is_default = ix == 0;
        let row = |id: &'static str,
                   icon_name: &'static str,
                   label: &'static str,
                   warning: bool,
                   checked: bool,
                   cx: &mut Context<Self>| {
            let color = if warning { WARNING.into() } else { theme.text };
            let icon_color = if warning {
                WARNING.into()
            } else {
                theme.text_muted
            };
            div()
                .id(("bases-page-menu-item", key(id, 0) as usize % 100_000))
                .flex()
                .flex_row()
                .items_center()
                .h(px(ROW_HEIGHT))
                .px(px(8.))
                .py(px(4.))
                .gap(px(8.))
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(color)
                .hover(|s| s.bg(theme.bg_alt))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.page_menu_pick(id, window, cx);
                }))
                .child(icon(icon_name, ICON_SIZE, icon_color))
                .child(
                    div()
                        .flex_1()
                        .whitespace_nowrap()
                        .child(SharedString::from(label)),
                )
                .when(checked, |d| d.child(icon("check", 14., theme.text_muted)))
        };
        let menu = div()
            .id("bases-page-menu-list")
            .relative()
            .flex()
            .flex_col()
            .min_w(px(153.))
            .p(px(6.))
            .bg(theme.bg_secondary)
            .border_1()
            .border_color(theme.tab_divider)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(FONT_UI_SMALL))
            .occlude()
            .child(self.measure(key("bases-page-menu-list", 0)))
            .child(row(
                "default",
                "layers-2",
                "Default view",
                false,
                is_default,
                cx,
            ))
            .child(row(
                "duplicate",
                "files",
                "Duplicate view",
                false,
                false,
                cx,
            ))
            .child(div().my(px(6.)).mx(px(-6.)).h(px(1.)).bg(theme.border))
            .child(row("delete", "trash-2", "Delete view", true, false, cx));
        Some(
            deferred(
                anchored()
                    .position(at)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    /// A row of the page header's menu.
    fn page_menu_pick(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.toolbar.header_menu = false;
        let Page::Configure(ix) = self.toolbar.page.clone() else {
            return;
        };
        match id {
            // The first view is the one a base opens on, so the default view
            // is the one moved to the front.
            "default" => {
                self.edit_base(move |base| layout::make_default_view(base, ix), cx);
                self.show_view(0, cx);
                self.toolbar.page = Page::Configure(0);
            }
            "duplicate" => {
                let mut copy = None;
                self.edit_base(|base| copy = layout::duplicate_view(base, ix), cx);
                if let Some(new_ix) = copy {
                    self.show_view(new_ix, cx);
                    self.configure_view(new_ix, cx);
                }
            }
            "delete" => {
                self.edit_base(move |base| layout::delete_view(base, ix), cx);
                let count = self.base().map_or(0, |b| b.views.len());
                self.show_view(ix.min(count.saturating_sub(1)), cx);
                self.close_panel(window, cx);
                return;
            }
            _ => {}
        }
        cx.notify();
    }

    /// The name field: a rename, written as typed.
    pub(super) fn commit_view_name(&mut self, text: &str, cx: &mut Context<Self>) {
        let Page::Configure(ix) = self.toolbar.page.clone() else {
            return;
        };
        let now = self
            .base()
            .and_then(|b| b.views.get(ix))
            .map(|v| v.name.clone());
        let text = text.trim();
        if text.is_empty() || now.as_deref() == Some(text) {
            return;
        }
        let name = text.to_string();
        // An empty base has no view yet; the default view is the one being
        // renamed, so it is materialized first.
        self.edit_base(
            move |base| {
                if base.views.is_empty() {
                    base.views.push(View {
                        kind: DEFAULT_VIEW_TYPE.to_string(),
                        name: DEFAULT_VIEW_NAME.to_string(),
                        ..View::default()
                    });
                }
                if let Some(view) = base.views.get_mut(ix) {
                    layout::rename_view(view, &name);
                }
            },
            cx,
        );
    }

    /// The list layout's separator field.
    pub(super) fn commit_separator(&mut self, text: &str, cx: &mut Context<Self>) {
        let now = self
            .base()
            .map(|base| self.showing_view(base))
            .and_then(|v| match Layout::of(&v) {
                Layout::List(list) => Some(list.separator),
                _ => None,
            });
        if now.as_deref() == Some(text) {
            return;
        }
        let separator = text.to_string();
        self.edit_view(move |view| layout::set_separator(view, &separator), cx);
    }

    /// A pick in one of this page's comboboxes; answers whether the name was
    /// one of its own.
    pub(super) fn views_pick(&mut self, name: &str, opt: usize, cx: &mut Context<Self>) -> bool {
        match name {
            // Observed: picking a layout rewrites `type` and leaves the
            // previous layout's keys in place.
            "layout" => {
                if let Some((kind, ..)) = LAYOUTS.get(opt) {
                    let kind = kind.to_string();
                    self.edit_view(move |view| layout::set_kind(view, &kind), cx);
                }
            }
            "rowHeight" => {
                if let Some(h) = RowHeight::all().get(opt).copied() {
                    self.edit_view(move |view| layout::set_row_height(view, h), cx);
                }
            }
            "markers" => {
                if let Some(m) = Markers::all().get(opt).copied() {
                    self.edit_view(move |view| layout::set_markers(view, m), cx);
                }
            }
            "imageFit" => {
                let fit = if opt == 1 {
                    ImageFit::Contain
                } else {
                    ImageFit::Cover
                };
                self.edit_view(move |view| layout::set_image_fit(view, fit), cx);
            }
            "image" => {
                let Some(base) = self.base().cloned() else {
                    return true;
                };
                let view = self.showing_view(&base);
                let (ids, _) = self.image_options(&base, &view);
                let id = ids.get(opt).cloned();
                self.edit_view(move |view| layout::set_image(view, id.as_deref()), cx);
            }
            _ => return false,
        }
        true
    }
}
