//! The "Download attachments" modal of `editor:download-attachments`, as
//! Obsidian 1.15.0 draws it
//! ([[Specification#Download attachments for current file]]): the title, a
//! gallery with one item per distinct remote image URL in document order,
//! each selected to begin with and a click away from deselected, then
//! Cancel and Download. Download hands the selected URLs to the workspace,
//! which fetches them; Cancel, the close button and Escape write nothing.

use crate::theme::{Theme, FONT_UI_SMALL};
use gpui::{
    actions, div, img, prelude::*, px, App, Context, EventEmitter, FocusHandle, Focusable,
    FontWeight, KeyBinding, ObjectFit, SharedString, SharedUri, Window,
};
use serde_json::{json, Value};

actions!(download_attachments, [Dismiss]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "escape",
        Dismiss,
        Some("DownloadAttachments"),
    )]);
}

pub enum DownloadEvent {
    /// Download pressed: the URLs still selected, in the gallery's order.
    Download(Vec<String>),
    Close,
}

pub struct DownloadModal {
    focus_handle: FocusHandle,
    items: Vec<(String, bool)>,
}

impl EventEmitter<DownloadEvent> for DownloadModal {}

impl Focusable for DownloadModal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl DownloadModal {
    pub fn new(urls: Vec<String>, cx: &mut Context<Self>) -> DownloadModal {
        DownloadModal {
            focus_handle: cx.focus_handle(),
            items: urls.into_iter().map(|u| (u, true)).collect(),
        }
    }

    pub fn selected(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|(_, on)| *on)
            .map(|(u, _)| u.clone())
            .collect()
    }

    /// An item's click: selected and deselected in turn.
    pub fn toggle(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.items.get_mut(ix) {
            item.1 = !item.1;
            cx.notify();
        }
    }

    pub fn describe(&self) -> Value {
        json!({
            "title": "Download attachments",
            "items": self.items.iter().map(|(url, on)| json!({"url": url, "selected": on})).collect::<Vec<_>>(),
            "buttons": ["Cancel", "Download"],
        })
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DownloadEvent::Close);
    }
}

impl Render for DownloadModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let t = &theme.tokens;
        let button = |id: &'static str, label: &'static str, primary: bool| {
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
                    d.bg(t.interactive_accent).text_color(gpui::white())
                })
                .when(!primary, |d| {
                    d.bg(t.background_modifier_hover).text_color(t.text_normal)
                })
                .child(label)
        };
        let mut gallery = div()
            .id("download-attachments-gallery")
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.))
            .max_h(px(420.))
            .overflow_y_scroll();
        for (ix, (url, on)) in self.items.iter().enumerate() {
            let on = *on;
            let check = div()
                .absolute()
                .top(px(6.))
                .left(px(6.))
                .size(px(16.))
                .rounded(px(3.))
                .border_1()
                .border_color(t.background_modifier_border)
                .bg(if on {
                    t.interactive_accent
                } else {
                    t.background_primary
                })
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| {
                    d.child(crate::assets::icon("check", 12., gpui::white()))
                });
            gallery = gallery.child(
                div()
                    .id(("download-attachment-item", ix))
                    .relative()
                    .size(px(120.))
                    .rounded(px(6.))
                    .overflow_hidden()
                    .cursor_pointer()
                    .border_2()
                    .border_color(if on {
                        t.interactive_accent
                    } else {
                        t.background_modifier_border
                    })
                    .bg(t.background_primary)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(ix, cx)))
                    .child(
                        img(SharedUri::from(url.clone()))
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(check),
            );
        }
        let title: SharedString = "Download attachments".into();
        div()
            .id("download-attachments")
            .key_context("DownloadAttachments")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::dismiss))
            .w(px(560.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(t.background_secondary)
            .border_1()
            .border_color(t.background_modifier_border)
            .rounded_lg()
            .shadow_lg()
            .p_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text_normal)
                            .child(title),
                    )
                    .child(
                        div()
                            .id("download-attachments-close")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(t.background_modifier_hover))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DownloadEvent::Close)))
                            .child(crate::assets::icon("x", 16., t.text_muted)),
                    ),
            )
            .child(gallery)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("download-attachments-cancel", "Cancel", false)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DownloadEvent::Close))),
                    )
                    .child(
                        button("download-attachments-download", "Download", true).on_click(
                            cx.listener(|this, _, _, cx| {
                                cx.emit(DownloadEvent::Download(this.selected()))
                            }),
                        ),
                    ),
            )
    }
}
