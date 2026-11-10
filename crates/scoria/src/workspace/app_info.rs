//! `app:show-debug-info` ("Show debug info"), as Obsidian 1.15.0 did it for
//! the 2026-11-09 probe ([[Specification#Show debug info]]). The modal
//! itself is [`crate::debug_info`]; Show release notes is
//! [`super::release_notes`]; Open file is [`super::attachments`].

use super::*;
use crate::debug_info::{operating_system, SystemInfo};

impl Workspace {
    /// `app:show-debug-info`: the modal, its text gathered now.
    pub(super) fn show_debug_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let info = match self.state.as_ref() {
            Some(state) => {
                let live_preview = state.app().live_preview.unwrap_or(true);
                let appearance = state.appearance();
                SystemInfo {
                    version: release_notes::VERSION.to_string(),
                    os: operating_system(),
                    live_preview,
                    base_theme: appearance.theme.clone(),
                    community_theme: appearance.css_theme.clone(),
                    snippets: appearance.enabled_css_snippets.as_ref().map_or(0, Vec::len),
                }
            }
            None => SystemInfo {
                version: release_notes::VERSION.to_string(),
                os: operating_system(),
                live_preview: true,
                base_theme: None,
                community_theme: None,
                snippets: 0,
            },
        };
        self.close_editor_suggest(cx);
        self.debug_info.update(cx, |m, cx| m.open(&info, cx));
        self.modal = Modal::DebugInfo;
        window.focus(&self.debug_info.focus_handle(cx), cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three app commands are Obsidian's ids and run.
    #[test]
    fn the_app_info_commands_are_runnable() {
        let known = scoria_commands::known_ids();
        for id in [
            "app:show-debug-info",
            "app:show-release-notes",
            "app:open-file",
        ] {
            assert!(RUNNABLE.contains(&id), "{id} runnable");
            assert!(known.contains(&id), "{id} is Obsidian's");
        }
    }
}
