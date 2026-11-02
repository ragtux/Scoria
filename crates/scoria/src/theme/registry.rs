//! The token registry: every value the shell paints from, keyed by the CSS
//! custom property Obsidian paints the same surface from.
//!
//! The names are Obsidian's (`docs/scoria/Theming.md`, "The registry: use
//! Obsidian's variable names"): a community theme's `--nav-item-color` lands
//! on [`Tokens::nav_item_color`] by a dictionary lookup, and the default
//! values in `defaults.rs` are the declarations the app's own stylesheet was
//! observed to carry, chains included, so a theme that overrides
//! `--color-base-70` moves `--text-muted` and everything downstream the way
//! it does in Obsidian.
//!
//! Which surface reads which token was probed, not guessed:
//! `tools/obsidian-probe/theme-registry.mjs` records, per surface element,
//! the cascade-winning declaration of each painted property
//! (`results/2026-11-02-theme-registry.json`), and the table in the
//! Specification ("Observed theme tokens") is the condensed record.

use gpui::Hsla;

use super::value::{Length, Resolved, ShadowLayer, Value};

// What a token's value has to be is the kind before its name in the list
// below. The kind decides the field's type and how a resolved CSS value is
// read into it; a value of the wrong shape keeps the default theme's, so a
// broken declaration in a theme cannot paint nonsense.
//
//   Color      an opaque or translucent colour
//   ColorOpt   a colour that may be `inherit`: the surface takes its parent's
//   Number     a unitless number or a percentage
//   NumberOpt  a number that may be `inherit`
//   Length     a `px` or `em` length
//   Shadow     box-shadow layers; `none` is no layers

pub trait FromValue: Sized {
    fn from_value(v: &Value) -> Option<Self>;
}

impl FromValue for Hsla {
    fn from_value(v: &Value) -> Option<Hsla> {
        match v {
            Value::Color(c) => Some(*c),
            _ => None,
        }
    }
}

/// `Some` for a colour, `None` for `inherit` and its cousins.
pub struct OptColor(pub Option<Hsla>);

impl FromValue for OptColor {
    fn from_value(v: &Value) -> Option<OptColor> {
        match v {
            Value::Color(c) => Some(OptColor(Some(*c))),
            Value::Keyword(k)
                if matches!(
                    k.as_str(),
                    "inherit" | "initial" | "unset" | "none" | "currentcolor"
                ) =>
            {
                Some(OptColor(None))
            }
            _ => None,
        }
    }
}

impl FromValue for f32 {
    fn from_value(v: &Value) -> Option<f32> {
        match v {
            Value::Number(n) | Value::Percent(n) => Some(*n),
            _ => None,
        }
    }
}

pub struct OptNumber(pub Option<f32>);

impl FromValue for OptNumber {
    fn from_value(v: &Value) -> Option<OptNumber> {
        match v {
            Value::Number(n) | Value::Percent(n) => Some(OptNumber(Some(*n))),
            Value::Keyword(k)
                if matches!(k.as_str(), "inherit" | "initial" | "unset" | "normal") =>
            {
                Some(OptNumber(None))
            }
            _ => None,
        }
    }
}

impl FromValue for Length {
    fn from_value(v: &Value) -> Option<Length> {
        match v {
            Value::Length(l) => Some(*l),
            // A bare `0` is a length of any unit.
            Value::Number(n) if *n == 0. => Some(Length::Px(0.)),
            // A shorthand (`10px 10px 0 0`, a radius per corner): its first.
            Value::Seq(items) => items.first().and_then(Length::from_value),
            _ => None,
        }
    }
}

impl FromValue for Vec<ShadowLayer> {
    fn from_value(v: &Value) -> Option<Vec<ShadowLayer>> {
        match v {
            Value::Shadow(l) => Some(l.clone()),
            Value::Keyword(k) if k == "none" => Some(Vec::new()),
            _ => None,
        }
    }
}

macro_rules! kind_ty {
    (Color) => { Hsla };
    (ColorOpt) => { Option<Hsla> };
    (Number) => { f32 };
    (NumberOpt) => { Option<f32> };
    (Length) => { Length };
    (Shadow) => { Vec<ShadowLayer> };
}

macro_rules! kind_read {
    (Color, $v:expr) => {
        <Hsla as FromValue>::from_value($v)
    };
    (ColorOpt, $v:expr) => {
        <OptColor as FromValue>::from_value($v).map(|o| o.0)
    };
    (Number, $v:expr) => {
        <f32 as FromValue>::from_value($v)
    };
    (NumberOpt, $v:expr) => {
        <OptNumber as FromValue>::from_value($v).map(|o| o.0)
    };
    (Length, $v:expr) => {
        <Length as FromValue>::from_value($v)
    };
    (Shadow, $v:expr) => {
        <Vec<ShadowLayer> as FromValue>::from_value($v)
    };
}

macro_rules! tokens {
    ( $( $(#[$meta:meta])* $kind:ident $field:ident = $css:literal; )* ) => {
        /// The registry's values, one typed field per token.
        #[derive(Debug, Clone, PartialEq)]
        pub struct Tokens {
            $( $(#[$meta])* pub $field: kind_ty!($kind), )*
        }

        /// Every token's CSS name, in declaration order.
        #[cfg(test)]
        pub const NAMES: &[&str] = &[ $( $css, )* ];

        impl Tokens {
            /// Read every token out of a resolved table. A token the table
            /// has not got, or has in the wrong shape, takes `fallback`'s
            /// value; `None` for a fallback means every token must resolve,
            /// and the first that does not is the error.
            pub fn from_resolved(table: &Resolved, fallback: Option<&Tokens>) -> Result<Tokens, String> {
                Ok(Tokens {
                    $(
                        $field: match table.get($css).and_then(|v| kind_read!($kind, v)) {
                            Some(v) => v,
                            None => match fallback {
                                Some(f) => f.$field.clone(),
                                None => return Err(format!("{}: {:?}", $css, table.get($css))),
                            },
                        },
                    )*
                })
            }
        }
    };
}

// Grouped as Obsidian's CSS variable reference groups them; a comment names
// the surfaces the probe saw read the token where that is not obvious from
// the name.
tokens! {
    // ---- Colors: base palette --------------------------------------------
    Color color_base_00 = "--color-base-00";
    Color color_base_05 = "--color-base-05";
    Color color_base_10 = "--color-base-10";
    Color color_base_20 = "--color-base-20";
    Color color_base_25 = "--color-base-25";
    Color color_base_30 = "--color-base-30";
    Color color_base_35 = "--color-base-35";
    Color color_base_40 = "--color-base-40";
    Color color_base_50 = "--color-base-50";
    Color color_base_60 = "--color-base-60";
    Color color_base_70 = "--color-base-70";
    Color color_base_100 = "--color-base-100";
    // ---- Colors: accent ----------------------------------------------------
    /// Hue in degrees; `appearance.json`'s `accentColor` sets the three.
    Number accent_h = "--accent-h";
    Number accent_s = "--accent-s";
    Number accent_l = "--accent-l";
    Color color_accent = "--color-accent";
    Color color_accent_1 = "--color-accent-1";
    Color color_accent_2 = "--color-accent-2";
    // ---- Colors: extended --------------------------------------------------
    Color color_red = "--color-red";
    Color color_orange = "--color-orange";
    Color color_yellow = "--color-yellow";
    Color color_green = "--color-green";
    Color color_cyan = "--color-cyan";
    Color color_blue = "--color-blue";
    Color color_purple = "--color-purple";
    Color color_pink = "--color-pink";
    /// Black on the light theme, white on the dark: the wash colours.
    Color mono_0 = "--mono-0";
    Color mono_100 = "--mono-100";
    // ---- Colors: surfaces --------------------------------------------------
    Color background_primary = "--background-primary";
    Color background_primary_alt = "--background-primary-alt";
    Color background_secondary = "--background-secondary";
    Color background_secondary_alt = "--background-secondary-alt";
    /// The hover wash: 6.7 percent of `--mono-100` over whatever is beneath.
    Color background_modifier_hover = "--background-modifier-hover";
    Color background_modifier_active_hover = "--background-modifier-active-hover";
    Color background_modifier_border = "--background-modifier-border";
    Color background_modifier_border_hover = "--background-modifier-border-hover";
    Color background_modifier_border_focus = "--background-modifier-border-focus";
    Color background_modifier_error = "--background-modifier-error";
    Color background_modifier_error_hover = "--background-modifier-error-hover";
    Color background_modifier_success = "--background-modifier-success";
    /// Notices and tooltips.
    Color background_modifier_message = "--background-modifier-message";
    Color background_modifier_form_field = "--background-modifier-form-field";
    /// The modal backdrop.
    Color background_modifier_cover = "--background-modifier-cover";
    // ---- Colors: interactive ----------------------------------------------
    Color interactive_normal = "--interactive-normal";
    Color interactive_hover = "--interactive-hover";
    Color interactive_accent = "--interactive-accent";
    Color interactive_accent_hover = "--interactive-accent-hover";
    // ---- Colors: text ------------------------------------------------------
    Color text_normal = "--text-normal";
    Color text_muted = "--text-muted";
    Color text_faint = "--text-faint";
    Color text_on_accent = "--text-on-accent";
    Color text_on_accent_inverted = "--text-on-accent-inverted";
    Color text_success = "--text-success";
    Color text_warning = "--text-warning";
    Color text_error = "--text-error";
    Color text_accent = "--text-accent";
    Color text_accent_hover = "--text-accent-hover";
    Color text_selection = "--text-selection";
    Color caret_color = "--caret-color";
    // ---- Typography --------------------------------------------------------
    Length font_ui_smaller = "--font-ui-smaller";
    Length font_ui_small = "--font-ui-small";
    Length font_ui_medium = "--font-ui-medium";
    Length font_ui_large = "--font-ui-large";
    Length font_text_size = "--font-text-size";
    Length font_smaller = "--font-smaller";
    Length font_smallest = "--font-smallest";
    Number font_weight = "--font-weight";
    Number font_normal = "--font-normal";
    Number font_medium = "--font-medium";
    Number font_semibold = "--font-semibold";
    Number font_bold = "--font-bold";
    Number bold_modifier = "--bold-modifier";
    Number bold_weight = "--bold-weight";
    Number line_height_normal = "--line-height-normal";
    Number line_height_tight = "--line-height-tight";
    // ---- Headings ----------------------------------------------------------
    ColorOpt h1_color = "--h1-color";
    ColorOpt h2_color = "--h2-color";
    ColorOpt h3_color = "--h3-color";
    ColorOpt h4_color = "--h4-color";
    ColorOpt h5_color = "--h5-color";
    ColorOpt h6_color = "--h6-color";
    Length h1_size = "--h1-size";
    Length h2_size = "--h2-size";
    Length h3_size = "--h3-size";
    Length h4_size = "--h4-size";
    Length h5_size = "--h5-size";
    Length h6_size = "--h6-size";
    Number h1_weight = "--h1-weight";
    Number h2_weight = "--h2-weight";
    Number h3_weight = "--h3-weight";
    Number h4_weight = "--h4-weight";
    Number h5_weight = "--h5-weight";
    Number h6_weight = "--h6-weight";
    Number h1_line_height = "--h1-line-height";
    Number h2_line_height = "--h2-line-height";
    Number h3_line_height = "--h3-line-height";
    Number h4_line_height = "--h4-line-height";
    Number h5_line_height = "--h5-line-height";
    Number h6_line_height = "--h6-line-height";
    ColorOpt inline_title_color = "--inline-title-color";
    Length inline_title_size = "--inline-title-size";
    Number inline_title_weight = "--inline-title-weight";
    Number inline_title_line_height = "--inline-title-line-height";
    // ---- Icons -------------------------------------------------------------
    Color icon_color = "--icon-color";
    Color icon_color_hover = "--icon-color-hover";
    Color icon_color_active = "--icon-color-active";
    Color icon_color_focused = "--icon-color-focused";
    Number icon_opacity = "--icon-opacity";
    Number icon_opacity_hover = "--icon-opacity-hover";
    Number icon_opacity_active = "--icon-opacity-active";
    Length icon_size = "--icon-size";
    // ---- Borders and dividers ---------------------------------------------
    Length border_width = "--border-width";
    Color divider_color = "--divider-color";
    Color divider_color_hover = "--divider-color-hover";
    Length divider_width = "--divider-width";
    Length divider_width_hover = "--divider-width-hover";
    Color hr_color = "--hr-color";
    Length hr_thickness = "--hr-thickness";
    // ---- Navigation (the Files, Tags, Outline, Backlinks rows) -------------
    Color nav_item_color = "--nav-item-color";
    Color nav_item_color_hover = "--nav-item-color-hover";
    Color nav_item_color_active = "--nav-item-color-active";
    Color nav_item_color_selected = "--nav-item-color-selected";
    Color nav_item_color_highlighted = "--nav-item-color-highlighted";
    Color nav_item_background_hover = "--nav-item-background-hover";
    Color nav_item_background_active = "--nav-item-background-active";
    Color nav_item_background_selected = "--nav-item-background-selected";
    Length nav_item_size = "--nav-item-size";
    NumberOpt nav_item_weight = "--nav-item-weight";
    NumberOpt nav_item_weight_hover = "--nav-item-weight-hover";
    NumberOpt nav_item_weight_active = "--nav-item-weight-active";
    Color nav_heading_color = "--nav-heading-color";
    Number nav_heading_weight = "--nav-heading-weight";
    Color nav_collapse_icon_color = "--nav-collapse-icon-color";
    Color nav_collapse_icon_color_collapsed = "--nav-collapse-icon-color-collapsed";
    Color nav_indentation_guide_color = "--nav-indentation-guide-color";
    /// The extension tag beside a non-markdown file's name.
    Color nav_tag_color = "--nav-tag-color";
    Color nav_tag_background = "--nav-tag-background";
    Number nav_tag_weight = "--nav-tag-weight";
    Color collapse_icon_color = "--collapse-icon-color";
    Color collapse_icon_color_collapsed = "--collapse-icon-color-collapsed";
    // ---- Tabs --------------------------------------------------------------
    Color tab_background_active = "--tab-background-active";
    Color tab_background_hover = "--tab-background-hover";
    Color tab_text_color = "--tab-text-color";
    Color tab_text_color_active = "--tab-text-color-active";
    Color tab_text_color_focused = "--tab-text-color-focused";
    Color tab_text_color_focused_active = "--tab-text-color-focused-active";
    Color tab_text_color_focused_highlighted = "--tab-text-color-focused-highlighted";
    /// The active tab of the active group: the one title in `--text-normal`.
    Color tab_text_color_focused_active_current = "--tab-text-color-focused-active-current";
    Length tab_font_size = "--tab-font-size";
    NumberOpt tab_font_weight = "--tab-font-weight";
    Color tab_container_background = "--tab-container-background";
    Color tab_divider_color = "--tab-divider-color";
    Color tab_outline_color = "--tab-outline-color";
    Color tab_outline_color_hover = "--tab-outline-color-hover";
    Length tab_outline_width = "--tab-outline-width";
    Length tab_curve = "--tab-curve";
    Length tab_radius = "--tab-radius";
    Length tab_radius_active = "--tab-radius-active";
    Color tab_inner_background_hover = "--tab-inner-background-hover";
    Color tab_inner_background_active = "--tab-inner-background-active";
    Color tab_inner_border_color = "--tab-inner-border-color";
    Color sidebar_tab_container_background = "--sidebar-tab-container-background";
    // ---- Window frame, ribbon, status bar, vault profile -------------------
    Color titlebar_background = "--titlebar-background";
    Color titlebar_background_focused = "--titlebar-background-focused";
    Color titlebar_border_color = "--titlebar-border-color";
    Length titlebar_border_width = "--titlebar-border-width";
    Color titlebar_text_color = "--titlebar-text-color";
    Color titlebar_text_color_focused = "--titlebar-text-color-focused";
    Length header_height = "--header-height";
    Color ribbon_background = "--ribbon-background";
    Length ribbon_width = "--ribbon-width";
    Color status_bar_background = "--status-bar-background";
    Color status_bar_border_color = "--status-bar-border-color";
    Length status_bar_border_width = "--status-bar-border-width";
    Color status_bar_text_color = "--status-bar-text-color";
    Length status_bar_font_size = "--status-bar-font-size";
    Color vault_profile_color = "--vault-profile-color";
    Color vault_profile_color_hover = "--vault-profile-color-hover";
    Length vault_profile_font_size = "--vault-profile-font-size";
    Number vault_profile_font_weight = "--vault-profile-font-weight";
    Color file_header_background = "--file-header-background";
    Color file_header_background_focused = "--file-header-background-focused";
    Length file_header_font_size = "--file-header-font-size";
    Number file_header_font_weight = "--file-header-font-weight";
    // ---- Menus, prompts, modals, popovers ----------------------------------
    Color menu_background = "--menu-background";
    Color menu_border_color = "--menu-border-color";
    Length menu_border_width = "--menu-border-width";
    Shadow menu_shadow = "--menu-shadow";
    Color prompt_background = "--prompt-background";
    Color prompt_border_color = "--prompt-border-color";
    Length prompt_border_width = "--prompt-border-width";
    Color suggestion_background = "--suggestion-background";
    Color modal_background = "--modal-background";
    Color modal_border_color = "--modal-border-color";
    Length modal_border_width = "--modal-border-width";
    Color modal_sidebar_background = "--modal-sidebar-background";
    Shadow shadow_s = "--shadow-s";
    Shadow shadow_l = "--shadow-l";
    Color notice_color = "--notice-color";
    Color notice_background = "--notice-background";
    Length notice_font_size = "--notice-font-size";
    Color tooltip_color = "--tooltip-color";
    Color tooltip_background = "--tooltip-background";
    Shadow tooltip_shadow = "--tooltip-shadow";
    Length tooltip_font_size = "--tooltip-font-size";
    Color hotkey_background = "--hotkey-background";
    Length hotkey_font_size = "--hotkey-font-size";
    // ---- Form controls -----------------------------------------------------
    Color dropdown_background = "--dropdown-background";
    Shadow input_shadow = "--input-shadow";
    Length input_border_width = "--input-border-width";
    Color slider_track_background = "--slider-track-background";
    Color slider_fill_background = "--slider-fill-background";
    Color slider_thumb_border_color = "--slider-thumb-border-color";
    Color checkbox_color = "--checkbox-color";
    Color checkbox_border_color = "--checkbox-border-color";
    Color checkbox_marker_color = "--checkbox-marker-color";
    Color checklist_done_color = "--checklist-done-color";
    Color toggle_thumb_color = "--toggle-thumb-color";
    // ---- Settings ----------------------------------------------------------
    Color setting_item_name_color = "--setting-item-name-color";
    Length setting_item_name_size = "--setting-item-name-size";
    Number setting_item_name_weight = "--setting-item-name-weight";
    Color setting_group_heading_color = "--setting-group-heading-color";
    Length setting_group_heading_size = "--setting-group-heading-size";
    Number setting_group_heading_weight = "--setting-group-heading-weight";
    Color setting_items_border_color = "--setting-items-border-color";
    Length setting_items_border_width = "--setting-items-border-width";
    // ---- Search ------------------------------------------------------------
    Color search_result_background = "--search-result-background";
    Color search_clear_button_color = "--search-clear-button-color";
    // ---- Scrollbars and guides --------------------------------------------
    Color scrollbar_thumb_bg = "--scrollbar-thumb-bg";
    Color scrollbar_active_thumb_bg = "--scrollbar-active-thumb-bg";
    Color scrollbar_bg = "--scrollbar-bg";
    Color indentation_guide_color = "--indentation-guide-color";
    Color indentation_guide_color_active = "--indentation-guide-color-active";
    // ---- Editor: code ------------------------------------------------------
    Color code_background = "--code-background";
    Color code_normal = "--code-normal";
    Color code_comment = "--code-comment";
    Color code_keyword = "--code-keyword";
    Color code_string = "--code-string";
    Color code_function = "--code-function";
    Color code_value = "--code-value";
    Color code_operator = "--code-operator";
    Color code_punctuation = "--code-punctuation";
    Color code_property = "--code-property";
    Color code_tag = "--code-tag";
    Color code_important = "--code-important";
    Length code_size = "--code-size";
    Color code_border_color = "--code-border-color";
    Length code_border_width = "--code-border-width";
    // ---- Editor: links and tags -------------------------------------------
    Color link_color = "--link-color";
    Color link_color_hover = "--link-color-hover";
    Color link_external_color = "--link-external-color";
    Color link_external_color_hover = "--link-external-color-hover";
    Color link_unresolved_color = "--link-unresolved-color";
    Number link_unresolved_opacity = "--link-unresolved-opacity";
    Color link_unresolved_decoration_color = "--link-unresolved-decoration-color";
    Number link_weight = "--link-weight";
    Color tag_color = "--tag-color";
    Color tag_color_hover = "--tag-color-hover";
    Color tag_background = "--tag-background";
    Color tag_background_hover = "--tag-background-hover";
    Color tag_border_color = "--tag-border-color";
    Length tag_border_width = "--tag-border-width";
    Length tag_size = "--tag-size";
    NumberOpt tag_weight = "--tag-weight";
    // ---- Editor: blocks ----------------------------------------------------
    ColorOpt blockquote_color = "--blockquote-color";
    Color blockquote_background_color = "--blockquote-background-color";
    Color blockquote_border_color = "--blockquote-border-color";
    Length blockquote_border_thickness = "--blockquote-border-thickness";
    Number callout_border_opacity = "--callout-border-opacity";
    ColorOpt callout_title_color = "--callout-title-color";
    Color callout_content_background = "--callout-content-background";
    ColorOpt bold_color = "--bold-color";
    ColorOpt italic_color = "--italic-color";
    Color highlight_background = "--highlight-background";
    Number highlight_opacity = "--highlight-opacity";
    Color list_marker_color = "--list-marker-color";
    Color table_border_color = "--table-border-color";
    Length table_border_width = "--table-border-width";
    Color table_header_color = "--table-header-color";
    Number table_header_weight = "--table-header-weight";
    Color table_header_background = "--table-header-background";
    Color table_header_border_color = "--table-header-border-color";
    ColorOpt table_text_color = "--table-text-color";
    Color table_background = "--table-background";
    Color table_row_alt_background = "--table-row-alt-background";
    Color table_row_background_hover = "--table-row-background-hover";
    Color table_selection = "--table-selection";
    ColorOpt embed_background = "--embed-background";
    Color embed_action_color = "--embed-action-color";
    Color metadata_background = "--metadata-background";
    Color metadata_border_color = "--metadata-border-color";
    Color metadata_label_text_color = "--metadata-label-text-color";
    Color metadata_property_background_hover = "--metadata-property-background-hover";
    Color metadata_divider_color = "--metadata-divider-color";
    Color pill_color = "--pill-color";
    Color pill_color_hover = "--pill-color-hover";
    Color pill_color_remove = "--pill-color-remove";
    Color pill_background = "--pill-background";
    Color pill_background_hover = "--pill-background-hover";
    Color pill_border_color = "--pill-border-color";
    Color pill_border_color_hover = "--pill-border-color-hover";
    Length pill_border_width = "--pill-border-width";
    // ---- Plugins: canvas and graph ----------------------------------------
    Color canvas_background = "--canvas-background";
    Color canvas_color = "--canvas-color";
    Color canvas_dot_pattern = "--canvas-dot-pattern";
    Color canvas_card_label_color = "--canvas-card-label-color";
    Color graph_line = "--graph-line";
    Color graph_node = "--graph-node";
    Color graph_node_unresolved = "--graph-node-unresolved";
    Color graph_node_focused = "--graph-node-focused";
    Color graph_node_tag = "--graph-node-tag";
    Color graph_node_attachment = "--graph-node-attachment";
    Color graph_text = "--graph-text";
}
