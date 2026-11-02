//! Colors and fonts for the shell, chosen from `appearance.json` the way
//! Obsidian's base themes are: `obsidian` is dark, `moonstone` is light,
//! `system` follows the desktop (dark until GPUI reports the preference).
//!
//! Every colour comes out of the token registry ([`registry::Tokens`]): a
//! table keyed by the CSS custom properties Obsidian paints the same
//! surfaces from, with the default values the app's own stylesheet was read
//! to carry on 2026-11-02 (`defaults.rs`, written by
//! `tools/obsidian-probe/theme-defaults.py` from a run of
//! `theme-registry.mjs`). The chains are resolved by [`value`], so
//! `--nav-item-color` is `var(--text-muted)` is `var(--color-base-70)` is
//! `#b3b3b3` here exactly as it is there, and a theme that moves a base
//! colour moves everything downstream.
//!
//! The sizes below are the values Obsidian 1.14.4 computes for its own
//! shell on Linux, read from a running copy with the probe in
//! `tools/obsidian-probe/frame-typography.mjs` on 2026-10-20 and listed in
//! `docs/scoria/Specification.md` ("Shell typography and chrome"). Hovered
//! and active rows are a 6.7 percent wash of white (dark) or black (light)
//! over whatever is underneath rather than a flat colour, so the same token
//! reads right over both backgrounds.
//!
//! The short field names (`text_muted`, `bg_alt`, …) predate the registry
//! and are derived from it; new paint sites should read `tokens` by the
//! surface's own token, which is what the probe record names.

pub mod defaults;
pub mod registry;
pub mod value;

use std::sync::OnceLock;

use gpui::{rgb, Global, Hsla, SharedString};
use scoria_formats::config::AppearanceConfig;

pub use registry::Tokens;
use value::{parse, Expr, Table};

/// Obsidian's interface sizes: the small UI text of tabs, rows and headers,
/// the smaller text of the status bar, the height of every header row, the
/// ribbon's width, the icon box, and the sidebar width a new vault gets.
pub const FONT_UI_SMALLER: f32 = 12.;
pub const FONT_UI_SMALL: f32 = 13.;
pub const FONT_UI_MEDIUM: f32 = 15.;
pub const HEADER_HEIGHT: f32 = 40.;
/// The radius of a tab's top corners and of the active tab's feet, where it curves into the view.
pub const TAB_CURVE: f32 = 10.;
pub const RIBBON_WIDTH: f32 = 44.;
pub const ICON_SIZE: f32 = 16.;
pub const SIDEBAR_WIDTH: f32 = 300.;

/// The monospace family a vault with no `monospaceFontFamily` renders code
/// in: the first entry of Obsidian's `--font-monospace` stack (see the
/// Specification's Fonts note) that ships with the platform. gpui names a
/// concrete family, not a generic one, and a family the platform lacks
/// falls back to its sans, so each platform gets a face it has.
pub const DEFAULT_MONO_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "DejaVu Sans Mono"
};

/// The body font size's multiplier for each heading level, and the weight
/// and line height Obsidian gives it (`--h1-size`, `--h1-weight`,
/// `--h1-line-height`; the fourth level down share the third's line height).
pub fn heading_style(level: u8) -> (f32, f32, f32) {
    match level {
        1 => (1.618, 700., 1.2),
        2 => (1.462, 680., 1.2),
        3 => (1.318, 660., 1.3),
        4 => (1.188, 640., 1.3),
        5 => (1.076, 620., 1.3),
        _ => (1.0, 600., 1.3),
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub dark: bool,
    /// `--background-primary`.
    pub bg: Hsla,
    /// `--background-secondary`.
    pub bg_secondary: Hsla,
    /// Hovered rows and buttons: a translucent wash (`--background-modifier-hover`).
    pub bg_alt: Hsla,
    /// The active row, dock tab and explorer file: the same wash.
    pub bg_active: Hsla,
    /// `--background-modifier-border`.
    pub border: Hsla,
    /// `--tab-divider-color`: the hairline between two inactive root tabs,
    /// read as a computed property on 2026-10-22 (#3f3f3f dark, #dadada
    /// light), a step brighter than the strip's own `--background-modifier-border`.
    pub tab_divider: Hsla,
    /// `--background-modifier-border-focus`: the 2 px ring on the explorer
    /// row the keyboard is on.
    pub border_focus: Hsla,
    /// `--text-normal`, `--text-muted`, `--text-faint`.
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_faint: Hsla,
    /// `--interactive-accent`.
    pub accent: Hsla,
    /// `--link-color` and `--tag-color`: both `--text-accent`.
    pub link: Hsla,
    pub tag: Hsla,
    /// `--highlight-background`: `--color-yellow` at `--highlight-opacity`.
    pub highlight_bg: Hsla,
    pub ui_font: SharedString,
    pub text_font: SharedString,
    pub mono_font: SharedString,
    pub base_font_size: f32,
    /// The canvas: card background and the border of a card with no colour
    /// of its own, the dot grid, the six preset colours a card or edge can
    /// carry (`"1"` red through `"6"` purple), the stroke of an edge with no
    /// colour, the label text above a card and inside a group, and the wash
    /// of a group's body and a marquee.
    pub canvas: CanvasColors,
    /// The registry: every token by Obsidian's name.
    pub tokens: Tokens,
}

/// The canvas view's palette. Obsidian paints the view over its primary
/// background, cards over the secondary one, and its preset colours are the
/// `--color-red` .. `--color-purple` tokens the callouts use too, so the
/// values below are the ones [`Theme::callout_color`] draws for the same
/// hues plus `--color-yellow`.
#[derive(Debug, Clone)]
pub struct CanvasColors {
    /// `--canvas-background`: the view's ground and every card's fill.
    pub background: Hsla,
    /// `--canvas-color`: the border and edge colour of an uncoloured card.
    pub card_border: Hsla,
    /// `--canvas-dot-pattern`.
    pub dot: Hsla,
    /// `--canvas-card-label-color`: the file name above a file card.
    pub label: Hsla,
    /// The alpha a coloured card's border and a group's frame paint their
    /// colour at.
    pub border_alpha: f32,
    /// The alpha a coloured card's or group's body is tinted at.
    pub tint_alpha: f32,
    /// The alpha a plain group's label pill paints the default colour at;
    /// also the marquee's wash.
    pub wash_alpha: f32,
    /// The text on a coloured group's solid label pill.
    pub on_color: Hsla,
    /// Control stack: a group's fill and border, a button's fill and glyph.
    pub control_group: Hsla,
    pub control_border: Hsla,
    pub control_bg: Hsla,
    pub control_fg: Hsla,
    /// The card menu's fill.
    pub card_menu_bg: Hsla,
    /// `"1"` red, `"2"` orange, `"3"` yellow, `"4"` green, `"5"` cyan, `"6"` purple.
    pub presets: [Hsla; 6],
}

impl CanvasColors {
    /// Obsidian 1.14.4's canvas, read from the running app: the view's
    /// tokens and the six `--color-*` presets, plus the control stack's
    /// colours, which are the shell's own (`--interactive-normal`, the
    /// border, the primary background, muted text, the secondary
    /// background).
    fn from_tokens(dark: bool, t: &Tokens) -> CanvasColors {
        CanvasColors {
            background: t.canvas_background,
            card_border: t.canvas_color,
            dot: t.canvas_dot_pattern,
            label: t.canvas_card_label_color,
            border_alpha: 0.7,
            tint_alpha: 0.07,
            wash_alpha: 0.1,
            on_color: rgb(if dark { 0x000000 } else { 0xffffff }).into(),
            control_group: t.interactive_normal,
            control_border: t.background_modifier_border,
            control_bg: t.background_primary,
            control_fg: t.text_muted,
            card_menu_bg: t.background_secondary,
            presets: [
                t.color_red,
                t.color_orange,
                t.color_yellow,
                t.color_green,
                t.color_cyan,
                t.color_purple,
            ],
        }
    }

    /// The colour of a preset, `"1"` to `"6"`.
    pub fn preset(&self, n: u8) -> Option<Hsla> {
        (1..=6)
            .contains(&n)
            .then(|| self.presets[usize::from(n) - 1])
    }
}

impl Global for Theme {}

/// The default declarations of one base theme, parsed once.
fn base_table(dark: bool) -> &'static Table {
    static DARK: OnceLock<Table> = OnceLock::new();
    static LIGHT: OnceLock<Table> = OnceLock::new();
    let cell = if dark { &DARK } else { &LIGHT };
    cell.get_or_init(|| {
        let mut t = Table::new();
        for (name, value) in defaults::COMMON {
            t.set_str(name, value);
        }
        let layer = if dark {
            defaults::DARK
        } else {
            defaults::LIGHT
        };
        for (name, value) in layer {
            t.set_str(name, value);
        }
        t
    })
}

/// The default tokens of one base theme, resolved once. Every token must
/// resolve from the defaults alone; `defaults.rs` is generated against the
/// registry, so a failure here is a registry entry the record never had.
fn base_tokens(dark: bool) -> &'static Tokens {
    static DARK: OnceLock<Tokens> = OnceLock::new();
    static LIGHT: OnceLock<Tokens> = OnceLock::new();
    let cell = if dark { &DARK } else { &LIGHT };
    cell.get_or_init(|| {
        Tokens::from_resolved(&base_table(dark).resolve_all(), None)
            .unwrap_or_else(|e| panic!("theme defaults do not resolve: {e}"))
    })
}

/// The base theme's tokens with `overrides` declared over them: what a
/// theme file, a snippet or the accent setting contributes. A declaration
/// that resolves to the wrong shape for its token keeps the default.
pub fn tokens_with(dark: bool, overrides: &[(String, Expr)]) -> Tokens {
    let fallback = base_tokens(dark);
    if overrides.is_empty() {
        return fallback.clone();
    }
    let mut table = base_table(dark).clone();
    for (name, expr) in overrides {
        table.set(name, expr.clone());
    }
    Tokens::from_resolved(&table.resolve_all(), Some(fallback)).unwrap_or_else(|_| fallback.clone())
}

impl Theme {
    pub fn dark() -> Theme {
        Theme::from_tokens(true, base_tokens(true).clone())
    }

    /// The app reaches the light theme through `from_appearance`; the
    /// tests reach it here.
    #[cfg(test)]
    pub fn light() -> Theme {
        Theme::from_tokens(false, base_tokens(false).clone())
    }

    /// A theme over a token set: the short fields are the tokens the probe
    /// saw the corresponding surfaces read.
    pub fn from_tokens(dark: bool, tokens: Tokens) -> Theme {
        Theme {
            dark,
            bg: tokens.background_primary,
            bg_secondary: tokens.background_secondary,
            bg_alt: tokens.background_modifier_hover,
            bg_active: tokens.background_modifier_hover,
            border: tokens.background_modifier_border,
            tab_divider: tokens.tab_divider_color,
            border_focus: tokens.background_modifier_border_focus,
            text: tokens.text_normal,
            text_muted: tokens.text_muted,
            text_faint: tokens.text_faint,
            accent: tokens.interactive_accent,
            link: tokens.link_color,
            tag: tokens.tag_color,
            highlight_bg: tokens.highlight_background,
            ui_font: crate::assets::FONT_FAMILY.into(),
            text_font: crate::assets::FONT_FAMILY.into(),
            mono_font: DEFAULT_MONO_FONT.into(),
            base_font_size: 16.0,
            canvas: CanvasColors::from_tokens(dark, &tokens),
            tokens,
        }
    }

    /// The theme `appearance.json` asks for.
    ///
    /// `system` is Obsidian's default and means the desktop's own scheme,
    /// which only a window can answer; `system_dark` is that answer, and
    /// the caller passes what it knows. A vault with no `theme` key gets
    /// the same treatment, since that is what Obsidian reads it as.
    pub fn from_appearance(config: &AppearanceConfig, system_dark: bool) -> Theme {
        let dark = match config.theme.as_deref() {
            Some("moonstone") => false,
            Some("obsidian") => true,
            // `system` and anything else Obsidian does not know.
            _ => system_dark,
        };
        let mut overrides = Vec::new();
        if let Some(hsl) = config.accent_color.as_deref().and_then(accent_hsl) {
            // Obsidian writes the three channels onto `body` as inline
            // custom properties; everything accent-coloured follows.
            overrides.push(("--accent-h".to_string(), parse(&format!("{}", hsl.0))));
            overrides.push(("--accent-s".to_string(), parse(&format!("{}%", hsl.1))));
            overrides.push(("--accent-l".to_string(), parse(&format!("{}%", hsl.2))));
        }
        let mut theme = Theme::from_tokens(dark, tokens_with(dark, &overrides));
        if let Some(size) = config.base_font_size {
            // Obsidian's own slider offers 10 to 30, but it neither clamps
            // nor rewrites a file that holds something else — 99 was left
            // alone across a whole session — so the file's value is
            // honoured. The floor only keeps a zero out of the layout.
            theme.base_font_size = size.max(1) as f32;
        }
        if let Some(f) = first_font(config.text_font_family.as_deref()) {
            theme.text_font = f;
        }
        if let Some(f) = first_font(config.interface_font_family.as_deref()) {
            theme.ui_font = f;
        }
        if let Some(f) = first_font(config.monospace_font_family.as_deref()) {
            theme.mono_font = f;
        }
        theme
    }

    /// The wash behind a `==🔴word==` highlight: Obsidian paints the
    /// theme's `--color-<name>` at 0.3 alpha — red was read back as
    /// `oklch(0.659737 0.216875 23.8105 / 0.3)` on the dark theme, which is
    /// its `#fb464c`; the other five are the same tokens the canvas presets
    /// use, at the same alpha.
    pub fn highlight_color(&self, color: scoria_markdown::HighlightColor) -> Hsla {
        use scoria_markdown::HighlightColor as C;
        let t = &self.tokens;
        let mut c = match color {
            C::Red => t.color_red,
            C::Orange => t.color_orange,
            C::Yellow => t.color_yellow,
            C::Green => t.color_green,
            C::Blue => t.color_blue,
            C::Purple => t.color_purple,
        };
        c.a = 0.3;
        c
    }

    /// Obsidian's default callout colors by canonical type. `--callout-color`
    /// is a `--color-<name>` token, the same six the canvas presets and the
    /// highlight washes draw from, so the hue is picked here and the theme's
    /// own value comes from the registry — which is what makes the dark
    /// theme's callouts the dark theme's colours. The table of observed
    /// values is in `docs/scoria/Specification.md` under "The lucide icon
    /// and colour of every built-in callout type"; it was read in the
    /// default dark theme, and the light column is the same tokens' light
    /// values.
    pub fn callout_color(&self, kind: &str) -> Hsla {
        let t = &self.tokens;
        match kind {
            "abstract" | "tip" => t.color_cyan,
            "success" => t.color_green,
            "question" | "warning" => t.color_orange,
            "failure" | "danger" | "bug" => t.color_red,
            "example" => t.color_purple,
            // Obsidian's grey is a flat value in both themes.
            "quote" => rgb(0x9e9e9e).into(),
            // `note`, `info`, `todo` and anything Obsidian does not know.
            _ => t.color_blue,
        }
    }
}

/// `appearance.json`'s `accentColor` (`#rrggbb`) as the `(h, s%, l%)`
/// Obsidian derives from it; an empty or malformed value is the default.
fn accent_hsl(hex: &str) -> Option<(f32, f32, f32)> {
    let hex = hex.trim().trim_start_matches('#');
    if !matches!(hex.len(), 6 | 3) {
        return None;
    }
    let c = match parse(&format!("#{hex}")) {
        Expr::Color(c) => c,
        _ => return None,
    };
    let round = |v: f32| (v * 100.).round() / 100.;
    Some((round(c.h * 360.), round(c.s * 100.), round(c.l * 100.)))
}

/// The first family of a CSS-style `"Font A", Font B` list.
fn first_font(list: Option<&str>) -> Option<SharedString> {
    let first = list?
        .split(',')
        .next()?
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    if first.is_empty() {
        None
    } else {
        Some(first.to_string().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use value::{hex, Value};

    /// Every token resolves from the defaults, in both themes.
    #[test]
    fn the_defaults_resolve() {
        let _ = Theme::dark();
        let _ = Theme::light();
        assert_eq!(registry::NAMES.len(), defaults::OBSERVED_DARK.len());
    }

    /// The resolver is held to what Obsidian computed for every token: the
    /// probe read each property's computed value off `body`, and whatever
    /// syntax that came back in (`#b3b3b3`, `rgba(10, 10, 10, 0.4)`,
    /// `hsl(calc(258 - 3), …)`, `color-mix(in oklch, white 6.7%, transparent)`,
    /// `calc(400 + 200)`, `0.875em`) is itself a value the resolver reads,
    /// so the two resolutions are compared value for value.
    fn observed_match(dark: bool, observed: &[(&str, &str)]) {
        let table = base_table(dark);
        let resolved = table.resolve_all();
        let empty = Table::new();
        let mut failures = Vec::new();
        for (name, computed) in observed {
            if computed.is_empty() {
                // `inherit` computes to nothing: an inherited token.
                continue;
            }
            let want = empty.resolve(&parse(computed));
            let got = resolved.get(name).cloned().unwrap_or(Value::Invalid);
            let same = match (&want, &got) {
                (Value::Color(a), Value::Color(b)) => {
                    let (ra, rb) = (a.to_rgb(), b.to_rgb());
                    (ra.r - rb.r).abs() < 1.5 / 255.
                        && (ra.g - rb.g).abs() < 1.5 / 255.
                        && (ra.b - rb.b).abs() < 1.5 / 255.
                        && (ra.a - rb.a).abs() < 0.005
                }
                (Value::Shadow(a), Value::Shadow(b)) => a.len() == b.len(),
                (a, b) => a == b,
            };
            if !same {
                failures.push(format!("{name}: want {computed:?} ({want:?}), got {got:?}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} tokens differ from Obsidian:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_dark_defaults_compute_what_obsidian_computed() {
        observed_match(true, defaults::OBSERVED_DARK);
    }

    #[test]
    fn the_light_defaults_compute_what_obsidian_computed() {
        observed_match(false, defaults::OBSERVED_LIGHT);
    }

    /// The short fields the app has painted from since 2026-10-20, as the
    /// registry derives them. These are the values `theme.rs` carried as
    /// literals before the registry existed, except where Obsidian differs:
    /// the light theme's accent is `--color-accent-1`, a shade lighter
    /// than the dark theme's (`hsl(257, 88.9%, 71%)`, not `hsl(258, 88%,
    /// 66%)`), and the highlight wash is `--color-yellow` at 30 percent
    /// rather than a flat gold.
    #[test]
    fn the_short_fields_are_the_probed_shell_colours() {
        let d = Theme::dark();
        assert_eq!(hex(d.bg), "#1c1c1c");
        assert_eq!(hex(d.bg_secondary), "#282828");
        assert_eq!(hex(d.border), "#333333");
        assert_eq!(hex(d.tab_divider), "#3f3f3f");
        assert_eq!(hex(d.border_focus), "#555555");
        assert_eq!(hex(d.text), "#dadada");
        assert_eq!(hex(d.text_muted), "#b3b3b3");
        assert_eq!(hex(d.text_faint), "#666666");
        assert!((d.bg_alt.a - 0.067).abs() < 1e-3 && d.bg_alt.l > 0.99);
        assert!((d.accent.h * 360. - 258.).abs() < 0.01 && (d.accent.l - 0.66).abs() < 1e-3);
        assert!((d.link.h * 360. - 255.).abs() < 0.01 && (d.link.l - 0.759).abs() < 1e-3);
        assert_eq!(d.tag, d.link);
        assert_eq!(hex(d.highlight_bg), "#e0de714d");

        let l = Theme::light();
        assert_eq!(hex(l.bg), "#ffffff");
        assert_eq!(hex(l.bg_secondary), "#f6f6f6");
        assert_eq!(hex(l.border), "#e4e4e4");
        assert_eq!(hex(l.tab_divider), "#dadada");
        assert_eq!(hex(l.border_focus), "#bdbdbd");
        assert_eq!(hex(l.text), "#222222");
        assert_eq!(hex(l.text_muted), "#5c5c5c");
        assert_eq!(hex(l.text_faint), "#ababab");
        assert!((l.bg_alt.a - 0.067).abs() < 1e-3 && l.bg_alt.l < 0.01);
        assert!(
            (l.accent.h * 360. - 257.).abs() < 0.01 && (l.accent.l - 0.7095).abs() < 1e-3,
            "{:?}",
            l.accent
        );
        assert!((l.link.h * 360. - 258.).abs() < 0.01 && (l.link.l - 0.66).abs() < 1e-3);
        assert_eq!(hex(l.highlight_bg), "#e0ac004d");
    }

    /// `accentColor` moves every accent-derived token the way Obsidian's
    /// inline `--accent-h/s/l` do: the brand orange lands on the links,
    /// the tags and the interactive accent.
    #[test]
    fn the_accent_setting_recolours_the_chain() {
        let config = AppearanceConfig {
            accent_color: Some("#e8590c".into()),
            ..Default::default()
        };
        let t = Theme::from_appearance(&config, true);
        assert!(
            (t.tokens.accent_h - 21.).abs() < 1.,
            "{}",
            t.tokens.accent_h
        );
        assert!((t.accent.h * 360. - 21.).abs() < 1., "{:?}", t.accent);
        // `--color-accent-1` on the dark theme: hue 3 less, saturation times
        // 1.02, lightness times 1.15.
        assert!((t.link.h * 360. - 18.).abs() < 1., "{:?}", t.link);
        assert!((t.link.l / t.tokens.color_accent.l - 1.15).abs() < 0.01);
        assert!(
            (t.tokens.tag_background.h - t.accent.h).abs() < 0.01
                && (t.tokens.tag_background.a - 0.1).abs() < 1e-3
        );
        // An empty value is the default.
        let plain = Theme::from_appearance(
            &AppearanceConfig {
                accent_color: Some(String::new()),
                ..Default::default()
            },
            true,
        );
        assert_eq!(plain.accent, Theme::dark().accent);
    }

    /// A theme's override that is not a colour keeps the default rather
    /// than painting nothing.
    #[test]
    fn a_malformed_override_keeps_the_default() {
        let tokens = tokens_with(true, &[("--text-muted".into(), parse("1px solid red"))]);
        assert_eq!(tokens.text_muted, Theme::dark().tokens.text_muted);
        let tokens = tokens_with(true, &[("--color-base-70".into(), parse("#ff0000"))]);
        assert_eq!(hex(tokens.text_muted), "#ff0000");
        assert_eq!(hex(tokens.nav_item_color), "#ff0000");
        assert_eq!(hex(tokens.icon_color), "#ff0000");
    }

    /// The whole of the probe's `--callout-color` table, aliases and all, read
    /// in Obsidian 1.14.5's default dark theme. Before this test
    /// [`Theme::callout_color`] ignored `self.dark` and answered the light
    /// theme's values in both themes, so every callout but `quote` was the
    /// wrong colour on the theme Obsidian ships.
    #[test]
    fn every_callout_type_takes_obsidians_dark_color() {
        let theme = Theme::dark();
        let table = [
            ("note", 0x027aff),
            ("abstract", 0x53dfdd),
            ("summary", 0x53dfdd),
            ("tldr", 0x53dfdd),
            ("info", 0x027aff),
            ("todo", 0x027aff),
            ("tip", 0x53dfdd),
            ("hint", 0x53dfdd),
            ("important", 0x53dfdd),
            ("success", 0x44cf6e),
            ("check", 0x44cf6e),
            ("done", 0x44cf6e),
            ("question", 0xe9973f),
            ("help", 0xe9973f),
            ("faq", 0xe9973f),
            ("warning", 0xe9973f),
            ("caution", 0xe9973f),
            ("attention", 0xe9973f),
            ("failure", 0xfb464c),
            ("fail", 0xfb464c),
            ("missing", 0xfb464c),
            ("danger", 0xfb464c),
            ("error", 0xfb464c),
            ("bug", 0xfb464c),
            ("example", 0xa882ff),
            ("quote", 0x9e9e9e),
            ("cite", 0x9e9e9e),
            // An unknown type falls back to note's pair, which it reaches as
            // `note` by way of the canonical type.
            ("nosuchtype", 0x027aff),
        ];
        for (written, want) in table {
            let kind = scoria_markdown::canonical_callout_type(written);
            assert_eq!(
                theme.callout_color(kind),
                rgb(want).into(),
                "[!{written}] on the dark theme"
            );
        }
    }

    /// The light theme's callouts are the same tokens' light values. These
    /// were never probed, so the assertion is the set the app already drew,
    /// unchanged by routing the lookup through the presets -- except
    /// `--color-blue`, which was `0x086dff` here and `0x086ddd` in
    /// [`Theme::highlight_color`] for the same token. The spec records
    /// `#086ddd`, so the odd one out was a typo.
    #[test]
    fn the_light_theme_keeps_the_light_tokens() {
        let theme = Theme::light();
        for (kind, want) in [
            ("abstract", 0x00bfbc),
            ("tip", 0x00bfbc),
            ("success", 0x08b94e),
            ("question", 0xec7500),
            ("warning", 0xec7500),
            ("failure", 0xe93147),
            ("danger", 0xe93147),
            ("bug", 0xe93147),
            ("example", 0x7852ee),
            ("quote", 0x9e9e9e),
            ("note", 0x086ddd),
        ] {
            assert_eq!(theme.callout_color(kind), rgb(want).into(), "[!{kind}]");
        }
    }

    /// The two themes may not answer the same colour for a hue that has one,
    /// which is the bug this pair of tests was written for.
    #[test]
    fn the_themes_disagree_where_obsidian_does() {
        for kind in [
            "note", "abstract", "success", "warning", "failure", "example",
        ] {
            assert_ne!(
                Theme::dark().callout_color(kind),
                Theme::light().callout_color(kind),
                "[!{kind}] is the same in both themes"
            );
        }
        // Grey is the exception: one value for both.
        assert_eq!(
            Theme::dark().callout_color("quote"),
            Theme::light().callout_color("quote")
        );
    }

    /// The callout hues and the highlight washes read the same tokens, so the
    /// two lookups may not drift apart again: the wash is the callout colour
    /// at 0.3 alpha, blue included.
    #[test]
    fn a_highlight_wash_is_its_callout_color_at_a_third_alpha() {
        use scoria_markdown::HighlightColor as C;
        for theme in [Theme::dark(), Theme::light()] {
            for (color, kind) in [
                (C::Red, "failure"),
                (C::Orange, "warning"),
                (C::Green, "success"),
                (C::Purple, "example"),
                (C::Blue, "note"),
            ] {
                let mut want = theme.callout_color(kind);
                want.a = 0.3;
                assert_eq!(theme.highlight_color(color), want, "{color:?}");
            }
        }
    }
}
