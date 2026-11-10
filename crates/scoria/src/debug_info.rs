//! "Show debug info" (`app:show-debug-info`): a large modal titled "Debug
//! info" holding one read-only textarea and three buttons, laid out as
//! Obsidian 1.15.0 draws it
//! ([[Specification#Show debug info]],
//! [[Specification#Show debug info, the values and the buttons]]).
//!
//! The shape is Obsidian's — `SYSTEM INFO:`, a tab before every value
//! line, a blank line before `RECOMMENDATIONS:`, a final newline — and
//! the values are Scoria's own. A line that would name something Scoria
//! does not have (an installer apart from the app, an account, an insider
//! channel, community plugins it runs) is left out rather than filled
//! with Obsidian's words; the first line names Scoria and its version, so
//! a report pasted from it can never be mistaken for one from Obsidian.

use crate::theme::{Theme, FONT_UI_SMALL};
use gpui::{
    actions, div, prelude::*, px, relative, App, Context, EventEmitter, FocusHandle, Focusable,
    FontWeight, KeyBinding, SharedString, Window,
};
use serde_json::{json, Value};

actions!(debug_info, [Dismiss]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("DebugInfo"))]);
}

/// The recommendation Obsidian lists under a community theme or an
/// enabled snippet, word for word (observed). Scoria loads both, and has a
/// sandbox vault of its own for the second sentence to point at.
pub const THEME_RECOMMENDATION: &str = "Custom theme and snippets: for cosmetic issues, please first try updating your theme and disabling your snippets. If still not fixed, please try to make the issue happen in the Sandbox Vault or disable community theme and snippets.";

/// What the modal reports, gathered when it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemInfo {
    /// Scoria's own version, the workspace manifest's.
    pub version: String,
    /// The kernel's version and release on Linux, as Obsidian prints
    /// `os.version()` and `os.release()`; the platform's name elsewhere.
    pub os: String,
    /// `app.json`'s `livePreview`, on when absent.
    pub live_preview: bool,
    /// `appearance.json`'s `theme`.
    pub base_theme: Option<String>,
    /// `appearance.json`'s `cssTheme`; empty is none.
    pub community_theme: Option<String>,
    /// How many names `enabledCssSnippets` lists.
    pub snippets: usize,
}

/// `Base theme:`'s value. `moonstone` reads "light" and a missing key or
/// `system` "adapt to system" (observed); `obsidian` is "dark".
pub fn base_theme_label(theme: Option<&str>) -> &'static str {
    match theme {
        Some("moonstone") => "light",
        Some("obsidian") => "dark",
        _ => "adapt to system",
    }
}

/// The textarea's text.
pub fn debug_text(info: &SystemInfo) -> String {
    let theme = info.community_theme.as_deref().filter(|t| !t.is_empty());
    let mut lines = vec![
        "SYSTEM INFO:".to_string(),
        format!("\tScoria version: {}", info.version),
        format!("\tOperating system: {}", info.os),
        // Scoria's interface is in English only.
        "\tLanguage: en".to_string(),
        format!(
            "\tLive preview: {}",
            if info.live_preview { "on" } else { "off" }
        ),
        format!(
            "\tBase theme: {}",
            base_theme_label(info.base_theme.as_deref())
        ),
        format!("\tCommunity theme: {}", theme.unwrap_or("none")),
        format!("\tSnippets enabled: {}", info.snippets),
        // Scoria runs no community plugin, whatever the vault enables:
        // it is always what Obsidian calls restricted mode.
        "\tRestricted mode: on".to_string(),
        String::new(),
        "RECOMMENDATIONS:".to_string(),
    ];
    if theme.is_some() || info.snippets > 0 {
        lines.push(format!("\t{THEME_RECOMMENDATION}"));
    } else {
        lines.push("\tnone".to_string());
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// The operating system line: on Linux the kernel's `version` and
/// `osrelease`, which are what Obsidian's line held on the probe's
/// machine; elsewhere the platform's name and architecture.
pub fn operating_system() -> String {
    let read = |name: &str| {
        std::fs::read_to_string(format!("/proc/sys/kernel/{name}"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    if cfg!(target_os = "linux") {
        if let (Some(version), Some(release)) = (read("version"), read("osrelease")) {
            return format!("{version} {release}");
        }
    }
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

pub enum DebugInfoEvent {
    /// "Copy to clipboard": the text, after which the modal closes.
    Copy(String),
    /// "Sandbox vault": the modal closes and the sandbox vault opens.
    Sandbox,
    /// "Done" or Escape.
    Close,
}

pub struct DebugInfo {
    focus_handle: FocusHandle,
    text: String,
}

impl EventEmitter<DebugInfoEvent> for DebugInfo {}

impl Focusable for DebugInfo {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl DebugInfo {
    pub fn new(cx: &mut Context<Self>) -> DebugInfo {
        DebugInfo {
            focus_handle: cx.focus_handle(),
            text: String::new(),
        }
    }

    /// Fills the textarea afresh, as each run of the command does.
    pub fn open(&mut self, info: &SystemInfo, cx: &mut Context<Self>) {
        self.text = debug_text(info);
        cx.notify();
    }

    pub fn describe(&self) -> Value {
        json!({
            "title": "Debug info",
            "text": self.text,
            "buttons": ["Copy to clipboard", "Sandbox vault", "Done"],
        })
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DebugInfoEvent::Close);
    }
}

impl Render for DebugInfo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let text: SharedString = self.text.clone().into();
        // `mod-secondary`, a plain button and `mod-cancel`: none of the
        // three is the accented `mod-cta`.
        let button = |id: &'static str, label: &'static str, theme: &Theme| {
            div()
                .id(id)
                .h(px(30.))
                .px(px(14.))
                .flex()
                .items_center()
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(FONT_UI_SMALL))
                .bg(theme.tokens.background_modifier_hover)
                .text_color(theme.tokens.text_normal)
                .child(label)
        };
        div()
            .id("debug-info")
            .key_context("DebugInfo")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::dismiss))
            // `.modal.mod-lg`, and no close button.
            .w(px(800.))
            .max_w(relative(0.9))
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
                    .pb(px(12.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.text_normal)
                    .child("Debug info"),
            )
            .child(
                // `textarea.debug-textarea`, read-only.
                div()
                    .id("debug-info-text")
                    .h(px(320.))
                    .overflow_y_scroll()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(5.))
                    .bg(theme.tokens.background_modifier_hover)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .text_size(px(FONT_UI_SMALL))
                    .line_height(relative(1.5))
                    .text_color(theme.tokens.text_normal)
                    .whitespace_normal()
                    .child(text),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(8.))
                    .pt(px(12.))
                    .child(
                        button("debug-info-copy", "Copy to clipboard", &theme).on_click(
                            cx.listener(|this, _, _, cx| {
                                cx.emit(DebugInfoEvent::Copy(this.text.clone()))
                            }),
                        ),
                    )
                    .child(
                        button("debug-info-sandbox", "Sandbox vault", &theme)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DebugInfoEvent::Sandbox))),
                    )
                    .child(
                        button("debug-info-done", "Done", &theme)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DebugInfoEvent::Close))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> SystemInfo {
        SystemInfo {
            version: "0.5.0".into(),
            os: "#1-NixOS SMP 6.18.60".into(),
            live_preview: true,
            base_theme: None,
            community_theme: None,
            snippets: 0,
        }
    }

    /// Obsidian's shape with Scoria's values: a tab before every value
    /// line, a blank line before RECOMMENDATIONS:, a final newline.
    #[test]
    fn a_fresh_vaults_text_has_obsidians_shape_and_scorias_values() {
        assert_eq!(
            debug_text(&fresh()),
            "SYSTEM INFO:\n\
             \tScoria version: 0.5.0\n\
             \tOperating system: #1-NixOS SMP 6.18.60\n\
             \tLanguage: en\n\
             \tLive preview: on\n\
             \tBase theme: adapt to system\n\
             \tCommunity theme: none\n\
             \tSnippets enabled: 0\n\
             \tRestricted mode: on\n\
             \n\
             RECOMMENDATIONS:\n\
             \tnone\n"
        );
    }

    #[test]
    fn the_text_never_claims_to_be_obsidian() {
        let text = debug_text(&fresh());
        assert!(!text.contains("Obsidian"), "{text}");
        assert!(text.contains("Scoria version: "));
    }

    /// A theme or a snippet brings Obsidian's one recommendation for them,
    /// one tab in, in place of `none`.
    #[test]
    fn a_theme_or_a_snippet_brings_the_theme_recommendation() {
        let themed = SystemInfo {
            live_preview: false,
            base_theme: Some("moonstone".into()),
            community_theme: Some("Probe Theme".into()),
            ..fresh()
        };
        let text = debug_text(&themed);
        assert!(text.contains("\tLive preview: off\n"));
        assert!(text.contains("\tBase theme: light\n"));
        assert!(text.contains("\tCommunity theme: Probe Theme\n"));
        assert!(text.ends_with(&format!("RECOMMENDATIONS:\n\t{THEME_RECOMMENDATION}\n")));
        let snippet = SystemInfo {
            snippets: 2,
            ..fresh()
        };
        let text = debug_text(&snippet);
        assert!(text.contains("\tSnippets enabled: 2\n"));
        assert!(text.ends_with(&format!("\t{THEME_RECOMMENDATION}\n")));
        // An empty `cssTheme` is the default theme.
        let empty = SystemInfo {
            community_theme: Some(String::new()),
            ..fresh()
        };
        assert!(debug_text(&empty).contains("\tCommunity theme: none\n"));
        assert!(debug_text(&empty).ends_with("\tnone\n"));
    }

    #[test]
    fn the_base_theme_reads_as_the_appearance_tab_names_it() {
        assert_eq!(base_theme_label(None), "adapt to system");
        assert_eq!(base_theme_label(Some("system")), "adapt to system");
        assert_eq!(base_theme_label(Some("moonstone")), "light");
        assert_eq!(base_theme_label(Some("obsidian")), "dark");
    }

    #[test]
    fn the_operating_system_line_is_never_empty() {
        assert!(!operating_system().trim().is_empty());
    }
}
