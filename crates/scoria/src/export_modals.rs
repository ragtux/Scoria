//! The two export modals: "Export to PDF" (`workspace:export-pdf`) and
//! "Export as image" (`canvas:export-as-image`), laid out as Obsidian 1.15.0
//! draws them ([[Specification#Export to PDF]],
//! [[Specification#Export as image]]): a title, a line naming the file in
//! curly quotes, setting rows with the control at the right, and the
//! buttons. The pages and the pixels are `scoria-export`'s; these only
//! gather the settings.
//!
//! The native `<select>`s are menus, as in the other modals; a slider is a
//! row of cells, one per step, so a click lands on a value without the
//! track being measured.

use crate::search::{menu, menu_item, toggle};
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER};
use gpui::{
    actions, anchored, deferred, div, prelude::*, px, Anchor, AnyElement, App, ClickEvent, Context,
    EventEmitter, FocusHandle, Focusable, FontWeight, KeyBinding, MouseButton, MouseMoveEvent,
    Pixels, Point, SharedString, Window,
};
use scoria_canvas::Viewport;
use scoria_export::{Area, ImageOptions, Margin, PageSize, PdfSettings};
use scoria_formats::canvas::Canvas;
use serde_json::{json, Value};

actions!(export_modals, [Dismiss]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("ExportModal"))]);
}

/// The width of a slider's track, as Obsidian's.
const TRACK: f32 = 100.0;

/// A `setting-item`: name (and description) at the left, the control at
/// the right, a hairline above every row.
fn row(
    name: &'static str,
    desc: Option<SharedString>,
    control: AnyElement,
    theme: &Theme,
) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .py(px(12.))
        .border_t_1()
        .border_color(theme.tokens.background_modifier_border)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(14.))
                        .text_color(theme.tokens.text_normal)
                        .child(name),
                )
                .children(desc.map(|d| {
                    div()
                        .text_size(px(FONT_UI_SMALLER))
                        .text_color(theme.tokens.text_muted)
                        .child(d)
                })),
        )
        .child(div().flex_shrink_0().child(control))
}

/// A button: `mod-cta` accented, `mod-cancel` plain.
fn button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(30.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(5.))
        .cursor_pointer()
        .text_size(px(FONT_UI_SMALL))
        .when(primary, |d| {
            d.bg(theme.tokens.interactive_accent)
                .text_color(gpui::white())
        })
        .when(!primary, |d| {
            d.bg(theme.tokens.background_modifier_hover)
                .text_color(theme.tokens.text_normal)
        })
        .child(label)
}

/// The modal's frame: the title and the close button over the body.
fn frame(
    id: &'static str,
    title: &'static str,
    width: f32,
    theme: &Theme,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .key_context("ExportModal")
        .w(px(width))
        .flex()
        .flex_col()
        .bg(theme.tokens.background_secondary)
        .border_1()
        .border_color(theme.tokens.background_modifier_border)
        .rounded_lg()
        .shadow_lg()
        .p_4()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pb(px(12.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.tokens.text_normal)
                        .child(title),
                )
                .child(
                    div()
                        .id((id, 1usize))
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                        .on_click(on_close)
                        .child(crate::assets::icon("x", 16., theme.tokens.text_muted)),
                ),
        )
}

/// A dropdown's field: the current label and a chevron pair.
fn select_field(
    id: (&'static str, usize),
    label: &'static str,
    open: bool,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(30.))
        .pl(px(10.))
        .pr(px(8.))
        .rounded(px(5.))
        .bg(theme.tokens.background_modifier_hover)
        .border_1()
        .border_color(if open {
            theme.tokens.interactive_accent
        } else {
            theme.tokens.background_modifier_border
        })
        .cursor_pointer()
        .text_size(px(FONT_UI_SMALL))
        .text_color(theme.tokens.text_normal)
        .child(div().whitespace_nowrap().child(label))
        .child(crate::assets::icon(
            "chevrons-up-down",
            12.,
            theme.tokens.text_muted,
        ))
}

/// A slider of `steps` cells, the first `filled` of them filled, its value
/// shown at its left.
fn slider_cells(
    id: &'static str,
    steps: usize,
    filled: usize,
    value: SharedString,
    theme: &Theme,
    mut cell: impl FnMut(usize, gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div>,
) -> AnyElement {
    let width = TRACK / steps as f32;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(FONT_UI_SMALL))
                .text_color(theme.tokens.text_normal)
                .child(value),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(16.))
                .children((0..steps).map(|ix| {
                    let c = div()
                        .id((id, ix))
                        .w(px(width))
                        .h(px(16.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .child(div().w_full().h(px(4.)).bg(if ix < filled {
                            theme.tokens.slider_fill_background
                        } else {
                            theme.tokens.slider_track_background
                        }));
                    cell(ix, c)
                })),
        )
        .into_any_element()
}

// ---- Export to PDF --------------------------------------------------------------

pub enum PdfExportEvent {
    /// "Export to PDF" with these settings.
    Export(PdfSettings),
    /// The close button or Escape; Cancel too.
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfSelect {
    PageSize,
    Margin,
}

pub struct PdfExport {
    focus_handle: FocusHandle,
    /// The note's basename.
    name: String,
    pub settings: PdfSettings,
    /// Whether "Include file name as title" was clicked since the modal
    /// opened: `includeName` joins `pdfExportSettings` from then on.
    pub include_touched: bool,
    open: Option<PdfSelect>,
    menu_at: Point<Pixels>,
}

impl EventEmitter<PdfExportEvent> for PdfExport {}

impl Focusable for PdfExport {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl PdfExport {
    pub fn new(cx: &mut Context<Self>) -> PdfExport {
        PdfExport {
            focus_handle: cx.focus_handle(),
            name: String::new(),
            settings: PdfSettings::default(),
            include_touched: false,
            open: None,
            menu_at: Point::default(),
        }
    }

    /// Opens over `name` with the last export's values.
    pub fn open(&mut self, name: String, settings: PdfSettings, cx: &mut Context<Self>) {
        self.name = name;
        self.settings = settings;
        self.include_touched = false;
        self.open = None;
        cx.notify();
    }

    pub fn describe(&self) -> Value {
        let s = &self.settings;
        json!({
            "title": "Export to PDF",
            "text": format!("Export \u{201c}{}\u{201d} to PDF with the settings below.", self.name),
            "include_name": s.include_name,
            "page_size": s.page_size.name(),
            "landscape": s.landscape,
            "margin": s.margin.label(),
            "downscale_percent": s.downscale_percent,
            "buttons": ["Export to PDF", "Cancel"],
        })
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.open.is_some() {
            self.open = None;
            cx.notify();
        } else {
            cx.emit(PdfExportEvent::Close);
        }
    }

    fn select(&self, which: PdfSelect, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let open = self.open == Some(which);
        let (ix, label) = match which {
            PdfSelect::PageSize => (0, self.settings.page_size.name()),
            PdfSelect::Margin => (1, self.settings.margin.label()),
        };
        let field = select_field(("pdf-export-select", ix), label, open, theme).on_click(
            cx.listener(move |this, event: &ClickEvent, _, cx| {
                this.menu_at = event.position();
                this.open = if this.open == Some(which) {
                    None
                } else {
                    Some(which)
                };
                cx.notify();
            }),
        );
        let popup = open.then(|| {
            let mut list =
                menu("pdf-export-menu", theme).on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.open = None;
                    cx.notify();
                }));
            match which {
                PdfSelect::PageSize => {
                    for (ix, size) in PageSize::ALL.into_iter().enumerate() {
                        list = list.child(
                            menu_item(
                                "pdf-export-size",
                                ix,
                                None,
                                size.name(),
                                size == self.settings.page_size,
                                theme,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.settings.page_size = size;
                                    this.open = None;
                                    cx.notify();
                                },
                            )),
                        );
                    }
                }
                PdfSelect::Margin => {
                    for (ix, margin) in Margin::ALL.into_iter().enumerate() {
                        list = list.child(
                            menu_item(
                                "pdf-export-margin",
                                ix,
                                None,
                                margin.label(),
                                margin == self.settings.margin,
                                theme,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.settings.margin = margin;
                                    this.open = None;
                                    cx.notify();
                                },
                            )),
                        );
                    }
                }
            }
            deferred(
                anchored()
                    .position(self.menu_at)
                    .anchor(Anchor::TopRight)
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(2)
            .into_any_element()
        });
        div()
            .relative()
            .child(field)
            .children(popup)
            .into_any_element()
    }
}

impl Render for PdfExport {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let s = self.settings;
        let percent = usize::from(s.downscale_percent.clamp(10, 100));
        let downscale = slider_cells(
            "pdf-export-downscale",
            91,
            percent - 9,
            percent.to_string().into(),
            &theme,
            |ix, cell| {
                let value = (ix + 10) as u8;
                cell.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.settings.downscale_percent = value;
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(
                    move |this, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left)
                            && this.settings.downscale_percent != value
                        {
                            this.settings.downscale_percent = value;
                            cx.notify();
                        }
                    },
                ))
            },
        );
        frame(
            "pdf-export",
            "Export to PDF",
            560.,
            &theme,
            cx.listener(|_, _, _, cx| cx.emit(PdfExportEvent::Close)),
        )
        .track_focus(&self.focus_handle)
        .on_action(cx.listener(Self::dismiss))
        .child(
            div()
                .pb(px(12.))
                .text_size(px(14.))
                .text_color(theme.tokens.text_normal)
                .child(format!(
                    "Export \u{201c}{}\u{201d} to PDF with the settings below.",
                    self.name
                )),
        )
        .child(row(
            "Include file name as title",
            None,
            toggle(("pdf-export-toggle", 0), s.include_name, &theme)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.include_name = !this.settings.include_name;
                    this.include_touched = true;
                    cx.notify();
                }))
                .into_any_element(),
            &theme,
        ))
        .child(row(
            "Page size",
            None,
            self.select(PdfSelect::PageSize, &theme, cx),
            &theme,
        ))
        .child(row(
            "Landscape",
            None,
            toggle(("pdf-export-toggle", 1), s.landscape, &theme)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.landscape = !this.settings.landscape;
                    cx.notify();
                }))
                .into_any_element(),
            &theme,
        ))
        .child(row(
            "Margin",
            None,
            self.select(PdfSelect::Margin, &theme, cx),
            &theme,
        ))
        .child(row("Downscale percent", None, downscale, &theme))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.))
                .pt(px(12.))
                .child(
                    button("pdf-export-go", "Export to PDF", true, &theme).on_click(
                        cx.listener(|this, _, _, cx| {
                            cx.emit(PdfExportEvent::Export(this.settings))
                        }),
                    ),
                )
                .child(
                    button("pdf-export-cancel", "Cancel", false, &theme)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PdfExportEvent::Close))),
                ),
        )
    }
}

// ---- Export as image ------------------------------------------------------------

pub enum ImageExportEvent {
    /// "Save" with these options.
    Save(ImageOptions),
    /// The close button or Escape.
    Close,
}

/// The Zoom slider's cells: -1 to 1 in steps of 0.05.
const ZOOM_STEPS: usize = 41;

fn zoom_of(cell: usize) -> f64 {
    -1.0 + cell as f64 * 0.05
}

pub struct ImageExport {
    focus_handle: FocusHandle,
    /// The canvas's basename.
    name: String,
    /// The canvas and the view, for the size estimate.
    canvas: Option<Canvas>,
    view: (Viewport, (f64, f64)),
    pub options: ImageOptions,
    open: bool,
    menu_at: Point<Pixels>,
}

impl EventEmitter<ImageExportEvent> for ImageExport {}

impl Focusable for ImageExport {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ImageExport {
    pub fn new(cx: &mut Context<Self>) -> ImageExport {
        ImageExport {
            focus_handle: cx.focus_handle(),
            name: String::new(),
            canvas: None,
            view: (Viewport::default(), (0.0, 0.0)),
            options: ImageOptions::default(),
            open: false,
            menu_at: Point::default(),
        }
    }

    /// Opens over a canvas, every row at its default: nothing is
    /// remembered between openings (observed).
    pub fn open(
        &mut self,
        name: String,
        canvas: Canvas,
        view: (Viewport, (f64, f64)),
        cx: &mut Context<Self>,
    ) {
        self.name = name;
        self.canvas = Some(canvas);
        self.view = view;
        self.options = ImageOptions::default();
        self.open = false;
        cx.notify();
    }

    fn viewport_only(&self) -> bool {
        matches!(self.options.area, Area::Viewport { .. })
    }

    fn set_viewport_only(&mut self, on: bool) {
        self.options.area = if on {
            Area::Viewport {
                viewport: self.view.0,
                size: self.view.1,
            }
        } else {
            Area::Full
        };
    }

    fn estimate(&self) -> String {
        let size = self
            .canvas
            .as_ref()
            .map(|c| scoria_export::estimated_size(c, &self.options))
            .unwrap_or((0, 0));
        scoria_export::estimate_text(size)
    }

    pub fn describe(&self) -> Value {
        let o = &self.options;
        json!({
            "title": "Export as image",
            "text": format!("Export \u{201c}{}\u{201d} as a PNG file with the settings below.", self.name),
            "viewport": if self.viewport_only() { "Viewport only" } else { "Full canvas" },
            "zoom": format!("{:.2}", 2f64.powf(o.zoom)),
            "estimate": self.estimate(),
            "show_logo": o.show_logo,
            "privacy": o.privacy,
            "buttons": ["Save"],
        })
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        } else {
            cx.emit(ImageExportEvent::Close);
        }
    }

    fn area_select(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let label = if self.viewport_only() {
            "Viewport only"
        } else {
            "Full canvas"
        };
        let field = select_field(("image-export-select", 0), label, self.open, theme).on_click(
            cx.listener(|this, event: &ClickEvent, _, cx| {
                this.menu_at = event.position();
                this.open = !this.open;
                cx.notify();
            }),
        );
        let popup = self.open.then(|| {
            let mut list = menu("image-export-menu", theme).on_mouse_down_out(cx.listener(
                |this, _, _, cx| {
                    this.open = false;
                    cx.notify();
                },
            ));
            for (ix, (title, on)) in [("Full canvas", false), ("Viewport only", true)]
                .into_iter()
                .enumerate()
            {
                list = list.child(
                    menu_item(
                        "image-export-area",
                        ix,
                        None,
                        title,
                        on == self.viewport_only(),
                        theme,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_viewport_only(on);
                        this.open = false;
                        cx.notify();
                    })),
                );
            }
            deferred(
                anchored()
                    .position(self.menu_at)
                    .anchor(Anchor::TopRight)
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(2)
            .into_any_element()
        });
        div()
            .relative()
            .child(field)
            .children(popup)
            .into_any_element()
    }
}

impl Render for ImageExport {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let o = self.options;
        let cell = ((o.zoom + 1.0) / 0.05)
            .round()
            .clamp(0.0, (ZOOM_STEPS - 1) as f64) as usize;
        let zoom = slider_cells(
            "image-export-zoom",
            ZOOM_STEPS,
            cell + 1,
            format!("{:.2}", 2f64.powf(o.zoom)).into(),
            &theme,
            |ix, c| {
                let value = zoom_of(ix);
                c.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.options.zoom = value;
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(
                    move |this, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left) && this.options.zoom != value
                        {
                            this.options.zoom = value;
                            cx.notify();
                        }
                    },
                ))
            },
        );
        frame(
            "image-export",
            "Export as image",
            // `.modal.mod-narrow`.
            560.,
            &theme,
            cx.listener(|_, _, _, cx| cx.emit(ImageExportEvent::Close)),
        )
        .track_focus(&self.focus_handle)
        .on_action(cx.listener(Self::dismiss))
        .child(
            div()
                .pb(px(12.))
                .text_size(px(14.))
                .text_color(theme.tokens.text_muted)
                .child(format!(
                    "Export \u{201c}{}\u{201d} as a PNG file with the settings below.",
                    self.name
                )),
        )
        .child(row(
            "Viewport",
            Some("Choose to render the entire canvas or just the current visible viewport.".into()),
            self.area_select(&theme, cx),
            &theme,
        ))
        .child(row("Zoom", Some(self.estimate().into()), zoom, &theme))
        .child(row(
            "Show logo",
            // Obsidian's line names its own logo; the mark here is Scoria's.
            Some("This will add a Scoria logo to the bottom left.".into()),
            toggle(("image-export-toggle", 0), o.show_logo, &theme)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.options.show_logo = !this.options.show_logo;
                    cx.notify();
                }))
                .into_any_element(),
            &theme,
        ))
        .child(row(
            "Privacy mode",
            Some("This will obscure any text on your canvas.".into()),
            toggle(("image-export-toggle", 1), o.privacy, &theme)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.options.privacy = !this.options.privacy;
                    cx.notify();
                }))
                .into_any_element(),
            &theme,
        ))
        .child(div().flex().flex_row().justify_end().pt(px(12.)).child(
            button("image-export-save", "Save", true, &theme).on_click(
                cx.listener(|this, _, _, cx| cx.emit(ImageExportEvent::Save(this.options))),
            ),
        ))
    }
}
