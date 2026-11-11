//! `slides:start` ("Slides: Start presentation") on the workspace's side:
//! which note, which window, and where the keyboard goes back to. The
//! presentation itself is [`crate::slides`]. Also the check `graph:animate`
//! runs under, the timelapse itself being [`GraphView::animate`].

use super::*;
use crate::slides::{SlidesEvent, SlidesView};

/// A presentation that is up: its view, the window it covers (`None` for
/// the vault window) and the subscription that hears it close.
pub(super) struct Presentation {
    pub(super) view: Entity<SlidesView>,
    window: Option<u64>,
    _closed: Subscription,
}

impl Workspace {
    /// `graph:animate`'s check: a global graph is the active leaf.
    pub(super) fn global_graph_active(&self, cx: &App) -> bool {
        self.main_view(cx) == MainView::Graph && !self.graph.read(cx).is_local()
    }

    /// `slides:start`: the active note, as the editor has it when it holds
    /// the note (an unsaved edit shows) and as the vault has it otherwise,
    /// cut into slides over the active group's window. Nothing is written
    /// and the leaf's mode is left as it was.
    pub(super) fn start_presentation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(state), Some(path)) = (self.state.clone(), self.current.clone()) else {
            return;
        };
        let text = {
            let editor = self.editor.read(cx);
            if editor.name.as_ref() == path.as_str() {
                editor.text().to_string()
            } else {
                match state.texts().get(&path) {
                    Some(text) => text.to_string(),
                    None => return,
                }
            }
        };
        let model = reading::build_with(&path, &text, state.as_ref(), state.build_options());
        self.close_editor_suggest(cx);
        let view = cx.new(|cx| SlidesView::new(path, &model, cx));
        let closed = cx.subscribe_in(&view, window, |this, _, event, window, cx| match event {
            SlidesEvent::Close => this.close_presentation(window, cx),
        });
        window.focus(&view.focus_handle(cx), cx);
        self.slides = Some(Presentation {
            view,
            window: self.window_of_group(self.active_group_id()),
            _closed: closed,
        });
        cx.notify();
    }

    /// Escape, or the close button: the presentation goes and the keyboard
    /// goes back to the view it was started from, in the mode it was in.
    pub(super) fn close_presentation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.slides.take().is_some() {
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    /// The presentation, when it covers window `here`.
    pub(super) fn presentation_in(&self, here: Option<u64>) -> Option<gpui::AnyElement> {
        let p = self.slides.as_ref()?;
        (p.window == here).then(|| p.view.clone().into_any_element())
    }
}
