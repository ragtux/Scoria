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
//! Every paint site reads `tokens` by the surface's own token, which is
//! what the probe record names.

pub mod css;
pub mod defaults;
pub mod fonts;
pub mod registry;
pub mod value;

use std::path::Path;
use std::sync::OnceLock;

use gpui::{rgb, Global, Hsla, SharedString};
use scoria_formats::config::AppearanceConfig;

use css::Cascade;
pub use registry::Tokens;
use value::{parse, Expr};

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
    pub ui_font: SharedString,
    pub text_font: SharedString,
    pub mono_font: SharedString,
    pub base_font_size: f32,
    /// `baseFontSizeAction`, "Quick font size adjustment": Ctrl+wheel over
    /// a note steps [`Theme::base_font_size`] for the session.
    pub quick_font_size: bool,
    /// The canvas: card background and the border of a card with no colour
    /// of its own, the dot grid, the six preset colours a card or edge can
    /// carry (`"1"` red through `"6"` purple), the stroke of an edge with no
    /// colour, the label text above a card and inside a group, and the wash
    /// of a group's body and a marquee.
    pub canvas: CanvasColors,
    /// The registry: every token by Obsidian's name.
    pub tokens: Tokens,
}

/// The canvas view's palette. Obsidian paints the view over
/// `--canvas-background` and a card over the primary background, and its
/// preset colours are `--canvas-color-1` .. `-6`, which are the
/// `--color-red` .. `--color-purple` tokens the callouts use too.
#[derive(Debug, Clone)]
pub struct CanvasColors {
    /// `--canvas-background`: the view's ground.
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
    /// Control stack: a group's fill and border, a button's fill and glyph,
    /// and a button's fill and glyph under the pointer.
    pub control_group: Hsla,
    pub control_border: Hsla,
    pub control_bg: Hsla,
    pub control_fg: Hsla,
    pub control_bg_hover: Hsla,
    pub control_fg_hover: Hsla,
    /// The card menu's fill: the secondary background on the dark theme,
    /// the primary on the light (`.theme-dark .canvas-card-menu`).
    pub card_menu_bg: Hsla,
    /// `"1"` red, `"2"` orange, `"3"` yellow, `"4"` green, `"5"` cyan, `"6"` purple.
    pub presets: [Hsla; 6],
}

impl CanvasColors {
    /// Obsidian 1.14.5's canvas, read from the running app (the probe of
    /// 2026-11-04, `theme-plugins.mjs`): the view's tokens and the six
    /// `--canvas-color-*` presets, plus the control stack's colours, which
    /// are the shell's own (`.canvas-control-group` is `--interactive-normal`
    /// in a `--background-modifier-border` frame; an item is `--text-muted`
    /// on the primary background, `--text-normal` on `--interactive-hover`
    /// under the pointer).
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
            control_bg_hover: t.interactive_hover,
            control_fg_hover: t.text_normal,
            card_menu_bg: if dark {
                t.background_secondary
            } else {
                t.background_primary
            },
            presets: [
                t.canvas_color_1,
                t.canvas_color_2,
                t.canvas_color_3,
                t.canvas_color_4,
                t.canvas_color_5,
                t.canvas_color_6,
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

/// Obsidian's own declarations, parsed once: the cascade every theme's
/// sheets are read into after them.
fn defaults_cascade() -> &'static Cascade {
    static CASCADE: OnceLock<Cascade> = OnceLock::new();
    CASCADE.get_or_init(|| {
        let mut c = Cascade::new();
        for (selector, name, value) in defaults::DEFAULTS {
            c.declare(selector, name, value);
        }
        c
    })
}

/// Scoria's own accent, `(h, s%, l%)`: a brick between the two inks of the
/// mark's palette that sit either side of it, `#a6462c` and `#d55227`
/// (`docs/scoria/Scoria.md`, "Brand colors"). It replaces Obsidian's purple
/// (`258, 88%, 66%`) as the accent a vault gets when `appearance.json` sets
/// none, and it is the one place the app's default theme departs from
/// Obsidian's sheet on purpose. Its hue keeps it apart from the error red
/// (`--color-red`, hue 358): brick is orange-leaning, the error pink-leaning.
/// Through Obsidian's own derivations it comes to `#be4f2d`, 4.9:1 under
/// white button text on the dark theme, 4.2:1 as a link on `#1c1c1c`, and
/// 4.9:1 as a link on white.
pub const ACCENT: (f32, f32, f32) = (14., 62., 46.);

/// The default sheet with Scoria's accent declared after Obsidian's, on
/// `body` like the original: a theme's own `--accent-*` still wins over it,
/// as does the inline `accentColor`, exactly as they win over the purple.
fn scoria_cascade() -> &'static Cascade {
    static CASCADE: OnceLock<Cascade> = OnceLock::new();
    CASCADE.get_or_init(|| {
        let mut c = defaults_cascade().clone();
        let (h, s, l) = ACCENT;
        c.declare("body", "--accent-h", &format!("{h}"));
        c.declare("body", "--accent-s", &format!("{s}%"));
        c.declare("body", "--accent-l", &format!("{l}%"));
        c
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
        Tokens::from_resolved(&scoria_cascade().table(dark).resolve_all(), None)
            .unwrap_or_else(|e| panic!("theme defaults do not resolve: {e}"))
    })
}

/// The base theme's tokens with `sheets` read over Obsidian's own, in
/// order (the theme's `theme.css`, then each enabled snippet), and the
/// accent channels set inline the way Obsidian's `accentColor` sets them.
/// A declaration that resolves to the wrong shape for its token keeps the
/// default.
pub fn tokens_from(dark: bool, sheets: &[String], accent: Option<(f32, f32, f32)>) -> Tokens {
    let fallback = base_tokens(dark);
    if sheets.is_empty() && accent.is_none() {
        return fallback.clone();
    }
    let mut cascade = scoria_cascade().clone();
    for sheet in sheets {
        cascade.add_css(sheet);
    }
    if let Some((h, s, l)) = accent {
        cascade.declare_inline("--accent-h", &format!("{h}"));
        cascade.declare_inline("--accent-s", &format!("{s}%"));
        cascade.declare_inline("--accent-l", &format!("{l}%"));
    }
    Tokens::from_resolved(&cascade.table(dark).resolve_all(), Some(fallback))
        .unwrap_or_else(|_| fallback.clone())
}

/// The sheets `appearance.json` names, read from the vault: the theme's
/// `theme.css` under `themes/<cssTheme>/`, then the enabled snippets under
/// `snippets/`. A file that is not there is skipped, as Obsidian skips it.
///
/// The snippets come in the **snippets folder's** order, sorted by name,
/// not in `enabledCssSnippets`' order. Obsidian rebuilds the whole set of
/// `<style>` elements whenever one is toggled, walking its sorted list of
/// snippet files and appending the enabled ones, so the alphabetically
/// later snippet always wins a conflict however the two were enabled —
/// `zz-last` enabled first still beats `aa-first` enabled second (probed
/// 2026-11-02, `appearance-tab.mjs` step s14).
pub fn sheets_of(root: &Path, config_dir: &str, config: &AppearanceConfig) -> Vec<String> {
    let dir = root.join(config_dir);
    let mut sheets = Vec::new();
    if let Some(theme) = config.css_theme.as_deref().filter(|t| !t.is_empty()) {
        if let Ok(css) = std::fs::read_to_string(dir.join("themes").join(theme).join("theme.css")) {
            sheets.push(css);
        }
    }
    let enabled = config.enabled_css_snippets.as_deref().unwrap_or(&[]);
    if !enabled.is_empty() {
        let mut names: Vec<String> = std::fs::read_dir(dir.join("snippets"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                path.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("css"))
                    .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .flatten()
            })
            .collect();
        names.sort();
        for name in names {
            if !enabled.contains(&name) {
                continue;
            }
            if let Ok(css) =
                std::fs::read_to_string(dir.join("snippets").join(format!("{name}.css")))
            {
                sheets.push(css);
            }
        }
    }
    sheets
}

impl Theme {
    pub fn dark() -> Theme {
        Theme::from_tokens(true, base_tokens(true).clone())
    }

    /// Obsidian's light theme with Scoria's accent. The app's windows reach
    /// the light theme through `from_appearance`; Export to PDF prints in
    /// this one, as Obsidian prints in its light theme whatever is on
    /// screen, and the tests reach it here.
    pub fn light() -> Theme {
        Theme::from_tokens(false, base_tokens(false).clone())
    }

    /// A theme over a token set.
    pub fn from_tokens(dark: bool, tokens: Tokens) -> Theme {
        Theme {
            dark,
            ui_font: crate::assets::FONT_FAMILY.into(),
            text_font: crate::assets::FONT_FAMILY.into(),
            mono_font: DEFAULT_MONO_FONT.into(),
            base_font_size: 16.0,
            quick_font_size: false,
            canvas: CanvasColors::from_tokens(dark, &tokens),
            tokens,
        }
    }

    /// The theme `appearance.json` asks for, with its sheets read from the
    /// vault's config folder.
    pub fn load(
        root: &Path,
        config_dir: &str,
        config: &AppearanceConfig,
        system_dark: bool,
    ) -> Theme {
        let sheets = sheets_of(root, config_dir, config);
        Theme::from_sources(config, system_dark, &sheets)
    }

    /// The theme `appearance.json` asks for, with no sheet of its own; the
    /// app reaches a theme through `load`, the tests through this.
    ///
    #[cfg(test)]
    /// `system` is Obsidian's default and means the desktop's own scheme,
    /// which only a window can answer; `system_dark` is that answer, and
    /// the caller passes what it knows. A vault with no `theme` key gets
    /// the same treatment, since that is what Obsidian reads it as.
    pub fn from_appearance(config: &AppearanceConfig, system_dark: bool) -> Theme {
        Theme::from_sources(config, system_dark, &[])
    }

    /// The theme `appearance.json` asks for, over `sheets`: the CSS of the
    /// theme it names and of its enabled snippets, in that order.
    pub fn from_sources(config: &AppearanceConfig, system_dark: bool, sheets: &[String]) -> Theme {
        let dark = match config.theme.as_deref() {
            Some("moonstone") => false,
            Some("obsidian") => true,
            // `system` and anything else Obsidian does not know.
            _ => system_dark,
        };
        // Obsidian writes the three accent channels onto `body` as inline
        // custom properties; everything accent-coloured follows.
        let accent = config.accent_color.as_deref().and_then(accent_hsl);
        let mut theme = Theme::from_tokens(dark, tokens_from(dark, sheets, accent));
        if let Some(size) = config.base_font_size {
            // Obsidian's own slider offers 10 to 30, but it neither clamps
            // nor rewrites a file that holds something else — 99 was left
            // alone across a whole session — so the file's value is
            // honoured. The floor only keeps a zero out of the layout.
            theme.base_font_size = size.max(1) as f32;
        }
        theme.quick_font_size = config.base_font_size_action.unwrap_or(false);
        // Each field is a list, and the first family of it the machine has
        // is the one applied; none of them leaves the default.
        if let Some(f) = fonts::first_installed(config.text_font_family.as_deref()) {
            theme.text_font = f.into();
        }
        if let Some(f) = fonts::first_installed(config.interface_font_family.as_deref()) {
            theme.ui_font = f.into();
        }
        if let Some(f) = fonts::first_installed(config.monospace_font_family.as_deref()) {
            theme.mono_font = f.into();
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

/// `accentColor` as the colour to paint its swatch with, for the
/// Appearance tab's row. `None` for the empty or malformed value that
/// means "no accent set", whose swatch is the theme's own accent.
pub fn accent_swatch(hex: &str) -> Option<Hsla> {
    let (h, s, l) = accent_hsl(hex)?;
    Some(gpui::hsla(h / 360., s / 100., l / 100., 1.))
}

/// The colour of a shadow token's first layer. Obsidian draws the frame
/// of a Bases card, a kanban card and an edited cell as a ring,
/// `0 0 0 1px <colour>`, and that ring is what Scoria paints as the border;
/// a shadow of `none` is no border at all.
pub fn ring(layers: &[value::ShadowLayer]) -> Hsla {
    layers
        .first()
        .map_or(gpui::transparent_black(), |l| l.color)
}

/// A shadow token as gpui draws it: one [`gpui::BoxShadow`] per outer
/// layer. gpui has no inset shadow, so an inset layer is left out.
pub fn box_shadows(layers: &[value::ShadowLayer]) -> Vec<gpui::BoxShadow> {
    layers
        .iter()
        .filter(|l| !l.inset)
        .map(|l| gpui::BoxShadow {
            color: l.color,
            offset: gpui::point(gpui::px(l.x), gpui::px(l.y)),
            blur_radius: gpui::px(l.blur),
            spread_radius: gpui::px(l.spread),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use value::{hex, Table, Value};

    /// Every token resolves from the defaults, in both themes.
    #[test]
    fn the_defaults_resolve() {
        let _ = Theme::dark();
        let _ = Theme::light();
        assert_eq!(registry::NAMES.len(), defaults::OBSERVED_DARK.len());
        assert!(defaults_cascade().len() > registry::NAMES.len());
    }

    /// The resolver is held to what Obsidian computed for every token: the
    /// probe read each property's computed value off `body`, and whatever
    /// syntax that came back in (`#b3b3b3`, `rgba(10, 10, 10, 0.4)`,
    /// `hsl(calc(258 - 3), …)`, `color-mix(in oklch, white 6.7%, transparent)`,
    /// `calc(400 + 200)`, `0.875em`) is itself a value the resolver reads,
    /// so the two resolutions are compared value for value.
    fn observed_match(dark: bool, observed: &[(&str, &str)]) {
        let resolved = defaults_cascade().table(dark).resolve_all();
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

    /// The shell colours the app has painted since 2026-10-20, as the
    /// registry resolves them. These are the values `theme.rs` carried as
    /// literals before the registry existed, except where Obsidian differs:
    /// the light theme's accent is `--color-accent-1`, a shade lighter
    /// than the dark theme's, and the highlight wash is `--color-yellow`
    /// at 30 percent rather than a flat gold - and except for the accent
    /// itself, which since 2026-11-02 is Scoria's brick ([`ACCENT`]) run
    /// through Obsidian's derivations rather than Obsidian's purple.
    #[test]
    fn the_shell_tokens_are_the_probed_colours() {
        let d = Theme::dark().tokens;
        assert_eq!(hex(d.background_primary), "#1c1c1c");
        assert_eq!(hex(d.background_secondary), "#282828");
        assert_eq!(hex(d.code_background), "#232323");
        assert_eq!(hex(d.background_modifier_border), "#333333");
        assert_eq!(hex(d.tab_divider_color), "#3f3f3f");
        assert_eq!(hex(d.background_modifier_border_focus), "#555555");
        assert_eq!(hex(d.text_normal), "#dadada");
        assert_eq!(hex(d.text_muted), "#b3b3b3");
        assert_eq!(hex(d.text_faint), "#666666");
        let hover = d.background_modifier_hover;
        assert!((hover.a - 0.067).abs() < 1e-3 && hover.l > 0.99);
        // `--interactive-accent` is `--color-accent` on the dark theme and
        // the links are `--color-accent-1`: hue 3 back, lightness x1.15.
        let accent = d.interactive_accent;
        assert!((accent.h * 360. - 14.).abs() < 0.01 && (accent.l - 0.46).abs() < 1e-3);
        assert_eq!(hex(accent), "#be4f2d");
        assert!(
            (d.link_color.h * 360. - 11.).abs() < 0.01 && (d.link_color.l - 0.529).abs() < 1e-3
        );
        assert_eq!(d.tag_color, d.link_color);
        assert_eq!(hex(d.highlight_background), "#e0de714d");

        let l = Theme::light().tokens;
        assert_eq!(hex(l.background_primary), "#ffffff");
        assert_eq!(hex(l.background_secondary), "#f6f6f6");
        assert_eq!(hex(l.code_background), "#fafafa");
        assert_eq!(hex(l.background_modifier_border), "#e4e4e4");
        assert_eq!(hex(l.tab_divider_color), "#dadada");
        assert_eq!(hex(l.background_modifier_border_focus), "#bdbdbd");
        assert_eq!(hex(l.text_normal), "#222222");
        assert_eq!(hex(l.text_muted), "#5c5c5c");
        assert_eq!(hex(l.text_faint), "#ababab");
        let hover = l.background_modifier_hover;
        assert!((hover.a - 0.067).abs() < 1e-3 && hover.l < 0.01);
        // The light theme's buttons are `--color-accent-1` (hue 1 back,
        // lightness x1.075) and its links `--color-accent` itself.
        let accent = l.interactive_accent;
        assert!(
            (accent.h * 360. - 13.).abs() < 0.01 && (accent.l - 0.4945).abs() < 1e-3,
            "{accent:?}"
        );
        assert!((l.link_color.h * 360. - 14.).abs() < 0.01 && (l.link_color.l - 0.46).abs() < 1e-3);
        assert_eq!(hex(l.highlight_background), "#e0ac004d");
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
        let t = Theme::from_appearance(&config, true).tokens;
        assert!((t.accent_h - 21.).abs() < 1., "{}", t.accent_h);
        assert!(
            (t.interactive_accent.h * 360. - 21.).abs() < 1.,
            "{:?}",
            t.interactive_accent
        );
        // `--color-accent-1` on the dark theme: hue 3 less, saturation times
        // 1.02, lightness times 1.15.
        assert!(
            (t.link_color.h * 360. - 18.).abs() < 1.,
            "{:?}",
            t.link_color
        );
        assert!((t.link_color.l / t.color_accent.l - 1.15).abs() < 0.01);
        assert!(
            (t.tag_background.h - t.interactive_accent.h).abs() < 0.01
                && (t.tag_background.a - 0.1).abs() < 1e-3
        );
        // An empty value is the default.
        let plain = Theme::from_appearance(
            &AppearanceConfig {
                accent_color: Some(String::new()),
                ..Default::default()
            },
            true,
        );
        assert_eq!(
            plain.tokens.interactive_accent,
            Theme::dark().tokens.interactive_accent
        );
    }

    /// A theme the way a community theme is written — a palette under the
    /// theme classes, tokens on `body`, its own variables, a snippet after
    /// it — read from the files `appearance.json` names, in that order.
    #[test]
    fn the_loader_reads_the_theme_and_the_enabled_snippets_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cfg = root.join(".obsidian");
        std::fs::create_dir_all(cfg.join("themes/Ember")).unwrap();
        std::fs::create_dir_all(cfg.join("snippets")).unwrap();
        std::fs::write(
            cfg.join("themes/Ember/theme.css"),
            r#"/* Ember */
            .theme-dark { --ember: #e8590c; --color-base-00: #101010; --color-base-70: var(--ember); }
            .theme-light { --color-base-00: #fff8f0; }
            body { --tab-text-color-focused-active-current: var(--ember); --h1-color: var(--ember); }
            .nav-file-title { color: red; --nav-item-color: red; }
            "#,
        )
        .unwrap();
        std::fs::write(
            cfg.join("snippets/wider.css"),
            "body { --tab-text-color-focused-active-current: #00ff00 }",
        )
        .unwrap();
        std::fs::write(cfg.join("snippets/off.css"), "body { --h1-color: #0000ff }").unwrap();
        let config = AppearanceConfig {
            css_theme: Some("Ember".into()),
            enabled_css_snippets: Some(vec!["wider".into(), "missing".into()]),
            ..Default::default()
        };
        let dark = Theme::load(root, ".obsidian", &config, true).tokens;
        assert_eq!(hex(dark.background_primary), "#101010");
        // The palette moved the chain: muted text, nav rows and icons.
        assert_eq!(hex(dark.text_muted), "#e8590c");
        assert_eq!(hex(dark.nav_item_color), "#e8590c");
        assert_eq!(hex(dark.icon_color), "#e8590c");
        // The snippet comes after the theme and wins on `body`.
        assert_eq!(hex(dark.tab_text_color_focused_active_current), "#00ff00");
        // An inherited heading colour becomes a colour of its own.
        assert_eq!(dark.h1_color.map(hex), Some("#e8590c".into()));
        // A rule on a component is not read, and a snippet not enabled is
        // not read either.
        assert_eq!(hex(dark.nav_item_color), "#e8590c");
        assert_ne!(dark.h1_color.map(hex), Some("#0000ff".into()));
        // The light theme takes its own palette entry and the same body rules.
        let light = Theme::load(root, ".obsidian", &config, false).tokens;
        assert_eq!(hex(light.background_primary), "#fff8f0");
        assert_eq!(light.text_muted, Theme::light().tokens.text_muted);
        assert_eq!(hex(light.tab_text_color_focused_active_current), "#00ff00");
        // No theme named: the defaults.
        let plain = Theme::load(root, ".obsidian", &AppearanceConfig::default(), true).tokens;
        assert_eq!(plain, Theme::dark().tokens);
    }

    /// Two snippets setting the same property settle in the snippets
    /// folder's order, not in the order `enabledCssSnippets` lists them:
    /// Obsidian walks its sorted list of files and appends the enabled
    /// ones, so the later name wins however they were switched on.
    #[test]
    fn enabled_snippets_cascade_in_the_folder_s_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cfg = root.join(".obsidian");
        std::fs::create_dir_all(cfg.join("snippets")).unwrap();
        std::fs::write(
            cfg.join("snippets/aa-first.css"),
            "body { --h1-color: #ff0000 }",
        )
        .unwrap();
        std::fs::write(
            cfg.join("snippets/zz-last.css"),
            "body { --h1-color: #00ff00 }",
        )
        .unwrap();
        // Enabled last-first, as clicking zz-last then aa-first writes it.
        let config = AppearanceConfig {
            enabled_css_snippets: Some(vec!["zz-last".into(), "aa-first".into()]),
            ..Default::default()
        };
        let tokens = Theme::load(root, ".obsidian", &config, true).tokens;
        assert_eq!(tokens.h1_color.map(hex), Some("#00ff00".into()));
        // And the other way round, which agrees because the order is the
        // folder's either way.
        let config = AppearanceConfig {
            enabled_css_snippets: Some(vec!["aa-first".into(), "zz-last".into()]),
            ..Default::default()
        };
        let tokens = Theme::load(root, ".obsidian", &config, true).tokens;
        assert_eq!(tokens.h1_color.map(hex), Some("#00ff00".into()));
        // One of the two enabled is that one, whichever end it sits at.
        let config = AppearanceConfig {
            enabled_css_snippets: Some(vec!["aa-first".into()]),
            ..Default::default()
        };
        let tokens = Theme::load(root, ".obsidian", &config, true).tokens;
        assert_eq!(tokens.h1_color.map(hex), Some("#ff0000".into()));
    }

    /// The accent setting outranks a theme's own accent channels, as an
    /// inline style outranks a sheet.
    #[test]
    fn the_accent_setting_outranks_the_theme_s_channels() {
        let sheets =
            vec![".theme-dark { --accent-h: 120; --accent-s: 50%; --accent-l: 50% }".to_string()];
        let themed = tokens_from(true, &sheets, None);
        assert!((themed.accent_h - 120.).abs() < 0.01);
        let config = AppearanceConfig {
            accent_color: Some("#e8590c".into()),
            ..Default::default()
        };
        let set = Theme::from_sources(&config, true, &sheets).tokens;
        assert!((set.accent_h - 21.).abs() < 1., "{}", set.accent_h);
    }

    /// A theme's override that is not a colour keeps the default rather
    /// than painting nothing.
    #[test]
    fn a_malformed_override_keeps_the_default() {
        let tokens = tokens_from(true, &["body { --text-muted: 1px solid red }".into()], None);
        assert_eq!(tokens.text_muted, Theme::dark().tokens.text_muted);
        let tokens = tokens_from(
            true,
            &[".theme-dark { --color-base-70: #ff0000 }".into()],
            None,
        );
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
    /// The Bases, canvas, graph and PDF chrome, by the tokens the probe of
    /// 2026-11-04 (`theme-plugins.mjs`) saw each surface read. The values
    /// are Obsidian's own; the colours Scoria had painted under the old
    /// short fields' names are in the comments where they differed.
    #[test]
    fn the_plugin_tokens_are_the_probed_colours() {
        let d = Theme::dark();
        let t = &d.tokens;
        // A Bases card sat on the secondary background (#282828); it is
        // `--bases-cards-background`, the primary.
        assert_eq!(hex(t.bases_cards_background), "#1c1c1c");
        assert_eq!(hex(ring(&t.bases_cards_shadow)), "#333333");
        assert_eq!(hex(ring(&t.bases_cards_shadow_hover)), "#3f3f3f");
        // A kanban column was the secondary background too; it is the
        // primary-alt one.
        assert_eq!(hex(t.bases_kanban_column_background), "#232323");
        assert_eq!(hex(t.bases_kanban_card_background), "#1c1c1c");
        // The kanban card's shadow is a ring and a drop.
        let drop = box_shadows(&t.bases_kanban_card_shadow);
        assert_eq!(drop.len(), 2);
        assert_eq!(
            (drop[1].offset.y, drop[1].blur_radius),
            (gpui::px(1.), gpui::px(2.))
        );
        // A table row has no hover wash, and the header has its own.
        assert_eq!(t.bases_table_row_background_hover.a, 0.);
        assert_eq!(
            t.bases_table_header_background_hover,
            t.background_modifier_hover
        );
        assert_eq!(hex(t.bases_table_group_background), "#232323");
        assert_eq!(hex(t.bases_table_header_color), "#b3b3b3");
        assert_eq!(ring(&t.bases_table_cell_shadow_focus), t.interactive_accent);
        // The toolbar's panels are `.menu`s: the secondary background in a
        // `--background-modifier-border-hover` frame, which is what
        // `--tab-divider-color` happened to be.
        assert_eq!(t.menu_background, t.background_secondary);
        assert_eq!(hex(t.menu_border_color), "#3f3f3f");
        assert_eq!(hex(t.bases_filter_input_background), "#2e2e2e");
        // The graph: a tag was painted in the accent and an attachment and
        // a link in the faint grey; they are green, yellow and `--graph-line`.
        assert_eq!(t.graph_node_tag, t.color_green);
        assert_eq!(t.graph_node_attachment, t.color_yellow);
        assert_eq!(hex(t.graph_line), "#3f3f3f");
        assert_eq!(t.graph_node, t.text_muted);
        assert_eq!(t.graph_node_unresolved, t.text_faint);
        assert_eq!(t.graph_node_focused, t.text_accent);
        assert_eq!(hex(t.toggle_thumb_color), "#ffffff");
        assert_eq!(hex(t.slider_thumb_background), "#ffffff");
        // The canvas presets are `--canvas-color-N`, still the callout hues.
        assert_eq!(d.canvas.preset(1), Some(t.color_red));
        assert_eq!(d.canvas.preset(6), Some(t.color_purple));
        assert_eq!(d.canvas.card_menu_bg, t.background_secondary);
        // The PDF view's ground and its page frame.
        assert_eq!(hex(t.pdf_background), "#1c1c1c");
        assert_eq!(hex(t.pdf_sidebar_background), "#1c1c1c");
        assert_eq!(box_shadows(&t.pdf_shadow).len(), 1);
        assert_eq!(hex(ring(&t.pdf_shadow)), "#333333");

        let l = Theme::light();
        let t = &l.tokens;
        assert_eq!(hex(t.bases_cards_background), "#ffffff");
        assert_eq!(hex(t.bases_kanban_column_background), "#fafafa");
        assert_eq!(hex(t.menu_border_color), "#dadada");
        assert_eq!(t.graph_node_tag, t.color_green);
        // `.theme-dark .canvas-card-menu` is the only rule that puts the card
        // menu on the secondary background; the light theme's is primary.
        assert_eq!(l.canvas.card_menu_bg, t.background_primary);
        // The light theme's page frame is a faint ring and a soft drop.
        let frame = box_shadows(&t.pdf_shadow);
        assert_eq!(frame.len(), 2);
        assert_eq!(hex(frame[0].color), "#0000000d");
        assert_eq!(
            (frame[1].offset.y, frame[1].blur_radius),
            (gpui::px(2.), gpui::px(8.))
        );
    }

    /// A theme that styles the plugin chrome by Obsidian's names reaches it
    /// now: the kanban column, the graph's tag colour and the PDF's ground
    /// were read under other names before.
    #[test]
    fn a_theme_reaches_the_plugin_chrome() {
        let sheet = concat!(
            "body { --bases-kanban-column-background: #102030; --graph-node-tag: #ff00ff;",
            " --pdf-background: #445566; --canvas-color-1: #00ff00; }"
        )
        .to_string();
        let t = tokens_from(true, &[sheet], None);
        assert_eq!(hex(t.bases_kanban_column_background), "#102030");
        assert_eq!(hex(t.graph_node_tag), "#ff00ff");
        assert_eq!(hex(t.pdf_background), "#445566");
        assert_eq!(hex(t.canvas_color_1), "#00ff00");
        // Untouched neighbours keep Obsidian's values.
        assert_eq!(hex(t.bases_kanban_card_background), "#1c1c1c");
    }
}
