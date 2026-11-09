//! A popout window: one of the workspace's windows beyond the main one.
//!
//! Obsidian's popout is a root split of tab groups and nothing else — no
//! ribbon, no docks, no status bar — titled like the vault window (see
//! [[Specification#Observed popout windows, Obsidian 1.15.0]]). The groups
//! it shows are the workspace's own, listed in a `WindowNode` of its
//! `windows`, so this view holds nothing but the workspace and the window's
//! id: its render asks the workspace to draw that window's groups, and the
//! workspace's listeners, hotkeys and focus rules apply inside it unchanged
//! (see [[Development#Multi-window, the design, 2026-11-09]]).

use crate::theme::Theme;
use crate::workspace::{client_frame, frame_window_options, popout_margin, Workspace};
use gpui::{
    div, prelude::*, px, size, App, Entity, Subscription, WeakEntity, Window, WindowBounds,
    WindowHandle, WindowOptions,
};

/// The inner size Obsidian gave every popout after the first (1024 × 800;
/// the first was 980 × 759), in logical pixels.
pub const INNER_SIZE: (f32, f32) = (1024., 800.);

/// The popout window's root view: the frame around one window's groups.
pub struct PopoutWindow {
    workspace: WeakEntity<Workspace>,
    /// The `WindowNode` this window draws, by its id.
    id: u64,
    /// Redraws with the workspace, which is where everything shown here
    /// lives, and goes down with it.
    _subscriptions: Vec<Subscription>,
}

impl PopoutWindow {
    /// The workspace this window draws, for the control socket, which
    /// runs a command inside the window a person's keys would reach.
    pub fn workspace(&self) -> WeakEntity<Workspace> {
        self.workspace.clone()
    }

    /// Opens the window over `bounds` (the outer bounds, margin included),
    /// maximized when they say so.
    /// Closing it from the desktop closes its groups through the workspace,
    /// as Obsidian's `close-window` does.
    pub fn open(
        workspace: Entity<Workspace>,
        id: u64,
        bounds: WindowBounds,
        cx: &mut App,
    ) -> anyhow::Result<WindowHandle<PopoutWindow>> {
        let handle = cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                window_min_size: Some(size(px(480.), px(320.))),
                ..frame_window_options("Scoria")
            },
            {
                let workspace = workspace.clone();
                move |window, cx| {
                    // The inset before the first commit, not at the first
                    // render as `client_frame` sets it: a compositor that
                    // sizes a floating window from its first commit would
                    // otherwise take the margin for content and configure
                    // the window that much larger, every time it opens.
                    window.set_client_inset(popout_margin());
                    let weak = workspace.downgrade();
                    let closing = weak.clone();
                    window.on_window_should_close(cx, move |_, cx| {
                        closing.update(cx, |ws, cx| ws.drop_popout(id, cx)).ok();
                        true
                    });
                    cx.new(|cx| PopoutWindow {
                        workspace: weak,
                        id,
                        _subscriptions: vec![
                            cx.observe(&workspace, |_, _, cx| cx.notify()),
                            // The desktop's focus coming here brings the
                            // active group with it.
                            cx.observe_window_activation(
                                window,
                                move |this: &mut PopoutWindow, window, cx| {
                                    if window.is_window_active() {
                                        this.workspace
                                            .update(cx, |ws, cx| ws.window_activated(Some(id), cx))
                                            .ok();
                                    }
                                },
                            ),
                        ],
                    })
                }
            },
        )?;
        // Down with the workspace, as the Settings window goes.
        let window = handle;
        let gone = cx.observe_release(&workspace, move |_, cx| {
            window
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        });
        handle
            .update(cx, |root, _, _| root._subscriptions.push(gone))
            .ok();
        Ok(handle)
    }
}

impl Render for PopoutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let id = self.id;
        let content = self
            .workspace
            .upgrade()
            .and_then(|ws| ws.update(cx, |ws, cx| ws.render_popout(id, &theme, window, cx)));
        let content = match content {
            Some(content) => content,
            // The window's last group closed from inside it, where the
            // workspace could not take the window down itself (a window
            // cannot be updated from within its own update): it goes now.
            None => {
                cx.defer_in(window, |_, window, _| window.remove_window());
                div().size_full().into_any_element()
            }
        };
        client_frame(content, &theme, window)
    }
}
