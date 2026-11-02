//! Where a combobox's pick and its option list are routed: each panel
//! answers for the names it owns, and this is the one place that asks them
//! in turn.

use crate::bases::toolbar::{Combo, ComboOption};
use crate::bases::view::BasesView;
use gpui::{Context, Window};
use scoria_bases::layout;
use scoria_formats::base::{Base, View};

impl BasesView {
    /// A pick in a combobox, by the name its owner gave it.
    pub(super) fn pick(
        &mut self,
        name: &'static str,
        ix: usize,
        opt: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.views_pick(name, opt, cx) {
            return;
        }
        if self.lists_pick(name, ix, opt, cx) {
            return;
        }
        if self.properties_pick(name, opt, cx) {
            return;
        }
        let _ = self.filter_pick(name, ix, opt, cx);
    }

    /// The clear button of a clearable combobox.
    pub(super) fn clear(
        &mut self,
        name: &'static str,
        _ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if name == "image" {
            self.edit_view(|view| layout::set_image(view, None), cx);
        }
    }

    /// The options of the open combobox, as its owner built them, so the
    /// keyboard sees the same rows the pointer does.
    pub(super) fn options_of(
        &self,
        combo: Combo,
        base: &Base,
        view: &View,
        _cx: &Context<Self>,
    ) -> Vec<ComboOption> {
        match combo.name {
            "layout" => self.layout_options(),
            "rowHeight" => scoria_bases::layout::RowHeight::all()
                .iter()
                .map(|h| ComboOption::plain(h.label()))
                .collect(),
            "markers" => scoria_bases::layout::Markers::all()
                .iter()
                .map(|m| ComboOption::plain(m.label()))
                .collect(),
            "imageFit" => vec![ComboOption::plain("Cover"), ComboOption::plain("Contain")],
            "image" => self.image_options(base, view).1,
            name => self
                .lists_options(name, combo.ix, base, view)
                .or_else(|| self.properties_options(name))
                .or_else(|| self.filter_options(name, combo.ix, base, view))
                .unwrap_or_default(),
        }
    }
}
