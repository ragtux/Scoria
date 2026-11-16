//! What the editor draws beside its text: the line-number gutter
//! (`showLineNumber`), the indentation guides (`showIndentGuide`), and the
//! side both go on (`rightToLeft`). The decisions that need no window —
//! the gutter's digits, a line's indent units, the guide the caret lights,
//! a line's direction — are `scoria_text::gutter` and `scoria_text::bidi`;
//! this module turns them into geometry and paint.
//!
//! Measured in Obsidian 1.15.0 (`tools/obsidian-probe/gutter-guides-rtl.mjs`,
//! 2026-11-15): the gutter sits at the start edge of the text column (the
//! 700 px readable column, or the whole pane) and the text moves over by
//! its width plus a 24 px margin, so the column keeps its width and the
//! text loses that much; the inline title and the properties block stay
//! where they were. The numbers are the interface font at 12 px in
//! `--text-faint`, right-aligned with 5 px of padding before them and 3
//! after, in a column at least 20 px wide, one number per line block: a
//! wrapped line's rows share one, a widget (table, embed, math block)
//! shows its first line's, the properties block none, and a fold hides
//! the numbers of the lines it hides. The caret's line is not drawn
//! differently. Under `rightToLeft` the gutter moves to the column's right
//! edge.

use gpui::{
    fill, point, px, size, App, Bounds, Font, FontFeatures, Hsla, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, TextRun, Window,
};
use scoria_formats::config::AppConfig;
use std::sync::Arc;

/// Space between the gutter and the text (`margin-inline-end`).
pub const MARGIN: f32 = 24.0;
/// The numbers' size, `--font-ui-smaller`.
pub const NUMBER_SIZE: f32 = 12.0;
/// Padding before and after a number, and the column's least width.
pub const PAD_START: f32 = 5.0;
pub const PAD_END: f32 = 3.0;
pub const MIN_WIDTH: f32 = 20.0;
/// Where a guide stands inside its indent unit, in ems of the line's
/// text: under the parent's marker. Obsidian's is
/// `--indentation-guide-source-indent` (a quarter em) in Source mode and
/// `--indentation-guide-editing-indent` (0.85 em) in Live Preview, where
/// its bullet stands that far into the unit; Scoria's Live Preview bullet
/// stands at the start of its marker, as the dash does in Source mode, so
/// both modes take the quarter em. A code block in Live Preview puts the
/// guide on the unit's edge.
pub const GUIDE_SOURCE_EM: f32 = 0.25;
/// `--indentation-guide-width`, active or not.
pub const GUIDE_WIDTH: f32 = 1.0;

/// The three Editor-tab toggles this module draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chrome {
    /// `showLineNumber`, default off.
    pub line_numbers: bool,
    /// `showIndentGuide`, default on.
    pub indent_guides: bool,
    /// `rightToLeft`, default off.
    pub right_to_left: bool,
}

impl Default for Chrome {
    fn default() -> Chrome {
        Chrome {
            line_numbers: false,
            indent_guides: true,
            right_to_left: false,
        }
    }
}

impl Chrome {
    /// The toggles as `app.json` has them, Obsidian's defaults for a key
    /// it lacks.
    pub fn from_app(app: &AppConfig) -> Chrome {
        let d = Chrome::default();
        Chrome {
            line_numbers: app.show_line_number.unwrap_or(d.line_numbers),
            indent_guides: app.show_indent_guide.unwrap_or(d.indent_guides),
            right_to_left: app.right_to_left.unwrap_or(d.right_to_left),
        }
    }
}

/// The text column split between the gutter and the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Columns {
    /// The text: where it starts and how wide it wraps.
    pub text_x: Pixels,
    pub text_width: Pixels,
    /// The gutter's left edge and width, when line numbers are on.
    pub gutter: Option<(Pixels, Pixels)>,
}

/// Splits the column at `column_x`, `column_width` wide, for a gutter
/// `gutter_width` wide (none when line numbers are off): at the start
/// edge, the left one or under `rightToLeft` the right one, with the
/// margin between it and the text.
pub fn columns(
    column_x: Pixels,
    column_width: Pixels,
    gutter_width: Option<Pixels>,
    rtl: bool,
) -> Columns {
    let Some(g) = gutter_width else {
        return Columns {
            text_x: column_x,
            text_width: column_width,
            gutter: None,
        };
    };
    let taken = g + px(MARGIN);
    let text_width = (column_width - taken).max(px(50.));
    if rtl {
        Columns {
            text_x: column_x,
            text_width,
            gutter: Some((column_x + column_width - g, g)),
        }
    } else {
        Columns {
            text_x: column_x + taken,
            text_width,
            gutter: Some((column_x, g)),
        }
    }
}

/// The interface font with tabular figures, so every number of a column
/// is as wide as every other with as many digits.
fn number_font(family: &SharedString) -> Font {
    let mut font = gpui::font(family.clone());
    font.features = FontFeatures(Arc::new(vec![("tnum".into(), 1)]));
    font
}

/// The width of the gutter for a note of `line_count` lines: its run of
/// nines measured in the number font, with the padding, at least
/// [`MIN_WIDTH`].
pub fn gutter_width(window: &mut Window, family: &SharedString, line_count: usize) -> Pixels {
    let digits = scoria_text::gutter::number_digits(line_count);
    let nines = shape(
        window,
        family,
        gpui::transparent_black(),
        "9".repeat(digits),
    );
    px(scoria_text::gutter::gutter_width(
        digits,
        f32::from(nines.width) / digits as f32,
        PAD_START + PAD_END,
        MIN_WIDTH,
    ))
}

fn shape(window: &mut Window, family: &SharedString, color: Hsla, text: String) -> ShapedLine {
    let run = TextRun {
        len: text.len(),
        font: number_font(family),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.into(), px(NUMBER_SIZE), &[run], None)
}

/// One line's number, laid out for paint.
pub struct Number {
    origin: Point<Pixels>,
    height: Pixels,
    line: ShapedLine,
}

/// Lays out the number of zero-based `line` in the gutter at `gutter`
/// (left edge and width), right-aligned before the end padding and
/// centred on a first row `row_height` tall whose top is `top`.
pub fn number(
    window: &mut Window,
    family: &SharedString,
    color: Hsla,
    gutter: (Pixels, Pixels),
    line: usize,
    top: Pixels,
    row_height: Pixels,
) -> Number {
    let shaped = shape(window, family, color, (line + 1).to_string());
    let (x, width) = gutter;
    Number {
        origin: point(x + width - px(PAD_END) - shaped.width, top),
        height: row_height,
        line: shaped,
    }
}

/// Paints the numbers laid out this frame.
pub fn paint_numbers(numbers: &[Number], window: &mut Window, cx: &mut App) {
    for n in numbers {
        let _ = n
            .line
            .paint(n.origin, n.height, gpui::TextAlign::Left, None, window, cx);
    }
}

/// The guide of an indent unit whose start edge is at `unit_x`, on a line
/// `height` tall from `top`: `offset_em` ems of `em` in from that edge,
/// to the right on a left-to-right line and to the left on a
/// right-to-left one.
pub fn guide(
    unit_x: Pixels,
    top: Pixels,
    height: Pixels,
    em: Pixels,
    offset_em: f32,
    rtl_line: bool,
    color: Hsla,
) -> PaintQuad {
    let offset = em * offset_em;
    let x = if rtl_line {
        unit_x - offset - px(GUIDE_WIDTH)
    } else {
        unit_x + offset
    };
    fill(
        Bounds::new(point(x, top), size(px(GUIDE_WIDTH), height)),
        color,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gutter_takes_its_width_and_the_margin_from_the_text() {
        // Obsidian's readable column: 516..1216; with a two-digit gutter
        // (21.7) the text starts at 561.7 and is 654.3 wide.
        let c = columns(px(516.), px(700.), Some(px(21.7)), false);
        assert_eq!(c.gutter, Some((px(516.), px(21.7))));
        assert!((f32::from(c.text_x) - 561.7).abs() < 0.01);
        assert!((f32::from(c.text_width) - 654.3).abs() < 0.01);
        let off = columns(px(516.), px(700.), None, false);
        assert_eq!(
            (off.text_x, off.text_width, off.gutter),
            (px(516.), px(700.), None)
        );
    }

    #[test]
    fn right_to_left_puts_the_gutter_on_the_right() {
        let c = columns(px(516.), px(700.), Some(px(21.7)), true);
        assert_eq!(c.text_x, px(516.));
        assert!((f32::from(c.text_width) - 654.3).abs() < 0.01);
        let (gx, gw) = c.gutter.unwrap();
        assert!((f32::from(gx + gw) - 1216.).abs() < 0.01);
    }

    #[test]
    fn a_guide_stands_its_offset_in_from_the_units_start_edge() {
        let ltr = guide(
            px(516.),
            px(0.),
            px(24.),
            px(16.),
            0.85,
            false,
            gpui::black(),
        );
        assert!((f32::from(ltr.bounds.origin.x) - 529.6).abs() < 0.01);
        let src = guide(
            px(516.),
            px(0.),
            px(24.),
            px(16.),
            GUIDE_SOURCE_EM,
            false,
            gpui::black(),
        );
        assert_eq!(src.bounds.origin.x, px(520.));
        let rtl = guide(
            px(600.),
            px(0.),
            px(24.),
            px(16.),
            GUIDE_SOURCE_EM,
            true,
            gpui::black(),
        );
        assert_eq!(rtl.bounds.origin.x, px(595.));
    }

    #[test]
    fn the_defaults_are_obsidians() {
        let c = Chrome::from_app(&AppConfig::default());
        assert!(!c.line_numbers && c.indent_guides && !c.right_to_left);
    }
}
