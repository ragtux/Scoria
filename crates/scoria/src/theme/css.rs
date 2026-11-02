//! The loader's half of a stylesheet: custom-property declarations under
//! `:root`, `html`, `body`, `.theme-dark` and `.theme-light`, and the
//! cascade that picks one value per property for the body of a page in
//! one base theme.
//!
//! This is a property scanner, not a CSS engine (`docs/scoria/Theming.md`,
//! "The loader"). Every rule whose selector is anything else — a component,
//! a descendant, a pseudo-element — is skipped without being understood,
//! and so is every declaration whose property does not begin `--`. What is
//! kept is exactly what a theme can change in Scoria: the value of a
//! token. Refusing to grow a cascade beyond these five selectors is what
//! keeps it small, and that refusal is deliberate.
//!
//! The cascade is the one CSS applies to `<body class="theme-dark">`: a
//! declaration on the body element (`body`, `.theme-dark`, `body.theme-dark`)
//! beats one it would inherit from `html` or `:root`; among declarations on
//! the same element `!important` wins, then specificity, then order. That
//! is why a theme declares `.theme-dark { --color-base-00: … }` rather than
//! `body { … }`: the app's own `.theme-dark` rule would otherwise outrank
//! it. Obsidian's defaults enter the same cascade first, so a theme's
//! rules stand in the order they do there.

use std::collections::HashMap;

use super::value::{parse, Expr, Table};

/// Which element a rule declares on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `:root` or `html`: inherited by the body.
    Root,
    /// `body`, `.theme-dark`, `.theme-light` and their compounds.
    Body,
}

/// Which base theme a rule applies in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Themes {
    Both,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: Expr,
    pub scope: Scope,
    pub themes: Themes,
    /// `(ids, classes and pseudo-classes, elements)`.
    pub specificity: (u8, u8, u8),
    pub important: bool,
    /// Position in the sheets read so far: later wins among equals.
    pub order: usize,
}

impl Declaration {
    fn applies(&self, dark: bool) -> bool {
        match self.themes {
            Themes::Both => true,
            Themes::Dark => dark,
            Themes::Light => !dark,
        }
    }

    /// The cascade's ordering on one element.
    fn rank(&self) -> (bool, bool, (u8, u8, u8), usize) {
        (
            self.important,
            self.scope == Scope::Body,
            self.specificity,
            self.order,
        )
    }
}

/// Classify a selector list: the scope, theme and specificity of its most
/// specific part that the scanner understands, or `None` when no part is a
/// compound of `html`, `body`, `:root`, `.theme-dark` and `.theme-light`.
pub fn classify(selector: &str) -> Option<(Scope, Themes, (u8, u8, u8))> {
    let mut best: Option<(Scope, Themes, (u8, u8, u8))> = None;
    for part in selector.split(',') {
        let Some(c) = classify_compound(part.trim()) else {
            continue;
        };
        if best.is_none_or(|b| c.2 > b.2) {
            best = Some(c);
        }
    }
    best
}

fn classify_compound(part: &str) -> Option<(Scope, Themes, (u8, u8, u8))> {
    if part.is_empty()
        || part.contains(char::is_whitespace)
        || part.contains(['>', '+', '~', '[', '(', '#'])
    {
        return None;
    }
    let mut body = false;
    let mut root = false;
    let mut dark = false;
    let mut light = false;
    let mut classes = 0u8;
    let mut elements = 0u8;
    let mut rest = part;
    while !rest.is_empty() {
        let end = rest[1..]
            .find(['.', ':'])
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let token = &rest[..end];
        rest = &rest[end..];
        match token {
            "body" => {
                body = true;
                elements += 1;
            }
            "html" => {
                root = true;
                elements += 1;
            }
            ":root" => {
                root = true;
                classes += 1;
            }
            ".theme-dark" => {
                dark = true;
                classes += 1;
            }
            ".theme-light" => {
                light = true;
                classes += 1;
            }
            _ => return None,
        }
    }
    if dark && light {
        return None;
    }
    let scope = if body || dark || light {
        if root {
            return None;
        }
        Scope::Body
    } else if root {
        Scope::Root
    } else {
        return None;
    };
    let themes = if dark {
        Themes::Dark
    } else if light {
        Themes::Light
    } else {
        Themes::Both
    };
    Some((scope, themes, (0, classes, elements)))
}

/// Strip `/* … */` comments, leaving strings alone.
fn strip_comments(css: &str) -> String {
    let b = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c as char);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1] as char);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == b'"' || c == b'\'' {
            quote = Some(c);
            out.push(c as char);
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            match css[i + 2..].find("*/") {
                Some(end) => i += 2 + end + 2,
                None => break,
            }
            out.push(' ');
            continue;
        }
        // Keep multi-byte characters whole.
        let ch = css[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// What a block on the stack means for the declarations inside it.
#[derive(Clone, Copy)]
enum Block {
    /// A rule the scanner understands: its declarations are read.
    Rule(Scope, Themes, (u8, u8, u8)),
    /// A rule it does not: declarations skipped, nested rules too.
    Skip,
    /// `@media`, `@supports`, `@layer`, `@container`: the rules inside
    /// stand on their own.
    Transparent,
}

/// Every custom-property declaration under a selector the loader reads,
/// numbered from `first_order`.
pub fn scan(css: &str, first_order: usize) -> Vec<Declaration> {
    let css = strip_comments(css);
    let mut out = Vec::new();
    let mut stack: Vec<Block> = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    let mut parens = 0usize;
    let mut order = first_order;
    let flush_declaration =
        |buf: &mut String, stack: &[Block], out: &mut Vec<Declaration>, order: &mut usize| {
            let text = buf.trim();
            if let Some(Block::Rule(scope, themes, specificity)) = stack.last().copied() {
                if let Some((name, value)) = text.split_once(':') {
                    let name = name.trim();
                    if name.starts_with("--") && name.len() > 2 {
                        let mut value = value.trim();
                        let mut important = false;
                        if let Some(v) = value.strip_suffix("!important") {
                            value = v.trim();
                            important = true;
                        } else if let Some(v) = value.strip_suffix("! important") {
                            value = v.trim();
                            important = true;
                        }
                        if !value.is_empty() {
                            out.push(Declaration {
                                name: name.to_string(),
                                value: parse(value),
                                scope,
                                themes,
                                specificity,
                                important,
                                order: *order,
                            });
                            *order += 1;
                        }
                    }
                }
            }
            buf.clear();
        };
    for ch in css.chars() {
        if let Some(q) = quote {
            buf.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                buf.push(ch);
            }
            '(' => {
                parens += 1;
                buf.push(ch);
            }
            ')' => {
                parens = parens.saturating_sub(1);
                buf.push(ch);
            }
            '{' if parens == 0 => {
                let prelude = buf.trim().to_string();
                buf.clear();
                let block = if let Some(at) = prelude.strip_prefix('@') {
                    let keyword = at
                        .split(|c: char| c.is_whitespace() || c == '(')
                        .next()
                        .unwrap_or("");
                    match keyword {
                        "media" | "supports" | "layer" | "container" | "scope" => {
                            Block::Transparent
                        }
                        _ => Block::Skip,
                    }
                } else if matches!(stack.last(), Some(Block::Skip)) {
                    Block::Skip
                } else {
                    match classify(&prelude) {
                        Some((scope, themes, spec)) => Block::Rule(scope, themes, spec),
                        None => Block::Skip,
                    }
                };
                stack.push(block);
            }
            '}' if parens == 0 => {
                // A last declaration may have no `;`.
                flush_declaration(&mut buf, &stack, &mut out, &mut order);
                stack.pop();
            }
            ';' if parens == 0 => {
                if stack.is_empty() {
                    // `@import …;` and friends at the top level.
                    buf.clear();
                } else {
                    flush_declaration(&mut buf, &stack, &mut out, &mut order);
                }
            }
            _ => buf.push(ch),
        }
    }
    out
}

/// The declarations of every sheet read so far, in order, with the cascade
/// that answers one value per property for a base theme.
#[derive(Debug, Clone, Default)]
pub struct Cascade {
    decls: Vec<Declaration>,
}

impl Cascade {
    pub fn new() -> Cascade {
        Cascade::default()
    }

    /// How many declarations have been read.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.decls.len()
    }

    /// A declaration as the app's own sheet carries it: `selector` is one
    /// of the five the scanner reads, `value` its text.
    pub fn declare(&mut self, selector: &str, name: &str, value: &str) {
        if let Some((scope, themes, specificity)) = classify(selector) {
            let order = self.decls.len();
            self.decls.push(Declaration {
                name: name.to_string(),
                value: parse(value),
                scope,
                themes,
                specificity,
                important: false,
                order,
            });
        }
    }

    /// Read a sheet's declarations after everything read so far.
    pub fn add_css(&mut self, css: &str) -> usize {
        let decls = scan(css, self.decls.len());
        let n = decls.len();
        self.decls.extend(decls);
        n
    }

    /// A value set on the body's own `style`, as Obsidian sets the accent
    /// channels: it outranks every sheet short of `!important`.
    pub fn declare_inline(&mut self, name: &str, value: &str) {
        let order = self.decls.len();
        self.decls.push(Declaration {
            name: name.to_string(),
            value: parse(value),
            scope: Scope::Body,
            themes: Themes::Both,
            specificity: (1, 0, 0),
            important: false,
            order,
        });
    }

    /// One winning declaration per property for the body in `dark` or
    /// light, as a table the resolver reads.
    pub fn table(&self, dark: bool) -> Table {
        let mut best: HashMap<&str, &Declaration> = HashMap::new();
        for d in &self.decls {
            if !d.applies(dark) {
                continue;
            }
            match best.get(d.name.as_str()) {
                Some(cur) if cur.rank() >= d.rank() => {}
                _ => {
                    best.insert(d.name.as_str(), d);
                }
            }
        }
        let mut t = Table::new();
        for (name, d) in best {
            t.set(name, d.value.clone());
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::value::{hex, Value};

    fn color(t: &Table, name: &str) -> String {
        match t.resolve_all().get(name) {
            Some(Value::Color(c)) => hex(*c),
            other => panic!("{name}: {other:?}"),
        }
    }

    #[test]
    fn the_five_selectors_classify_and_nothing_else_does() {
        assert_eq!(
            classify(":root"),
            Some((Scope::Root, Themes::Both, (0, 1, 0)))
        );
        assert_eq!(
            classify("html"),
            Some((Scope::Root, Themes::Both, (0, 0, 1)))
        );
        assert_eq!(
            classify("body"),
            Some((Scope::Body, Themes::Both, (0, 0, 1)))
        );
        assert_eq!(
            classify(".theme-dark"),
            Some((Scope::Body, Themes::Dark, (0, 1, 0)))
        );
        assert_eq!(
            classify("body.theme-light"),
            Some((Scope::Body, Themes::Light, (0, 1, 1)))
        );
        assert_eq!(
            classify(".theme-dark.theme-dark"),
            Some((Scope::Body, Themes::Dark, (0, 2, 0)))
        );
        // A list takes its most specific readable part.
        assert_eq!(
            classify(".nav-file-title, body.theme-dark"),
            Some((Scope::Body, Themes::Dark, (0, 1, 1)))
        );
        for bad in [
            ".nav-file-title",
            "body .markdown-preview-view",
            "body > div",
            ".theme-dark.minimal",
            ".theme-dark.theme-light",
            "html body",
            "body:not(.is-mobile)",
            "",
        ] {
            assert_eq!(classify(bad), None, "{bad:?}");
        }
    }

    /// The shape a community theme has: a palette per theme class, tokens
    /// on `body`, component rules everywhere else, comments, a nested
    /// `@media`, a `url()` with a semicolon, a last declaration without
    /// its semicolon and an `!important`.
    #[test]
    fn a_sheet_yields_its_custom_properties_and_nothing_else() {
        let css = r#"
            /* A theme { with braces; in a comment } */
            @charset "utf-8";
            @import url("fonts.css");
            :root { --accent-h: 20; --not-custom: 1px }
            body {
              --font-text-size: 17px;
              font-size: 17px;
              --bg-image: url("data:image/svg+xml;charset=utf8,%3Csvg%3E");
              --with-string: "a; b { c }";
            }
            .theme-dark { --color-base-00: #101010; --color-base-100: #f0f0f0 !important }
            .theme-light { --color-base-00: #fefefe }
            .nav-file-title { --nav-item-color: red; color: var(--nav-item-color); }
            @media (max-width: 600px) { body { --font-text-size: 15px; } .foo { --x: 1 } }
            @font-face { font-family: X; src: url(x.woff); }
            @keyframes spin { from { --a: 1 } to { --a: 2 } }
            body.theme-dark .markdown-preview-view { --h1-color: blue }
            .theme-dark.theme-dark { --color-base-00: #000000 }
        "#;
        let decls = scan(css, 0);
        #[allow(clippy::type_complexity)]
        let names: Vec<(&str, Scope, Themes, (u8, u8, u8), bool)> = decls
            .iter()
            .map(|d| {
                (
                    d.name.as_str(),
                    d.scope,
                    d.themes,
                    d.specificity,
                    d.important,
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                ("--accent-h", Scope::Root, Themes::Both, (0, 1, 0), false),
                ("--not-custom", Scope::Root, Themes::Both, (0, 1, 0), false),
                (
                    "--font-text-size",
                    Scope::Body,
                    Themes::Both,
                    (0, 0, 1),
                    false
                ),
                ("--bg-image", Scope::Body, Themes::Both, (0, 0, 1), false),
                ("--with-string", Scope::Body, Themes::Both, (0, 0, 1), false),
                (
                    "--color-base-00",
                    Scope::Body,
                    Themes::Dark,
                    (0, 1, 0),
                    false
                ),
                (
                    "--color-base-100",
                    Scope::Body,
                    Themes::Dark,
                    (0, 1, 0),
                    true
                ),
                (
                    "--color-base-00",
                    Scope::Body,
                    Themes::Light,
                    (0, 1, 0),
                    false
                ),
                (
                    "--font-text-size",
                    Scope::Body,
                    Themes::Both,
                    (0, 0, 1),
                    false
                ),
                (
                    "--color-base-00",
                    Scope::Body,
                    Themes::Dark,
                    (0, 2, 0),
                    false
                ),
            ]
        );
        let orders: Vec<usize> = decls.iter().map(|d| d.order).collect();
        assert_eq!(orders, (0..10).collect::<Vec<_>>());
    }

    /// The cascade on the body: a body-level declaration beats an inherited
    /// `:root` one whatever their order; among body-level ones specificity
    /// decides, then order; `!important` trumps all. This is the rule that
    /// makes the app's `.theme-dark` outrank a theme's `body`.
    #[test]
    fn the_body_cascade_is_css_s() {
        let mut c = Cascade::new();
        c.declare(".theme-dark", "--a", "#111111");
        c.declare("body", "--a", "#222222");
        c.declare(":root", "--a", "#333333");
        assert_eq!(color(&c.table(true), "--a"), "#111111");
        // The light theme has no `.theme-dark`: body wins there.
        assert_eq!(color(&c.table(false), "--a"), "#222222");
        // Only `:root`: it is the value.
        let mut r = Cascade::new();
        r.declare(":root", "--b", "#444444");
        r.declare("html", "--b", "#555555");
        assert_eq!(color(&r.table(true), "--b"), "#444444");
        // Same selector: later wins.
        let mut l = Cascade::new();
        l.declare(":root", "--c", "#666666");
        l.declare(":root", "--c", "#777777");
        assert_eq!(color(&l.table(true), "--c"), "#777777");
        // Important wins from anywhere.
        let mut i = Cascade::new();
        i.declare(".theme-dark", "--d", "#888888");
        i.add_css(":root { --d: #999999 !important }");
        assert_eq!(color(&i.table(true), "--d"), "#999999");
        // Inline (the accent) beats a theme's `.theme-dark`.
        let mut n = Cascade::new();
        n.declare(".theme-dark", "--e", "#aaaaaa");
        n.declare_inline("--e", "#bbbbbb");
        assert_eq!(color(&n.table(true), "--e"), "#bbbbbb");
    }

    /// A theme over the app's own chain: moving a base colour moves what
    /// is derived from it, and the theme's private variables resolve too.
    #[test]
    fn a_theme_s_palette_propagates_through_the_app_s_chain() {
        let mut c = Cascade::new();
        c.declare("body", "--text-muted", "var(--color-base-70)");
        c.declare("body", "--nav-item-color", "var(--text-muted)");
        c.declare(".theme-dark", "--color-base-70", "#b3b3b3");
        c.add_css(
            r#".theme-dark { --mine: #ff8800; --color-base-70: var(--mine); }
               body { --nav-item-color: var(--mine); }"#,
        );
        let t = c.table(true);
        assert_eq!(color(&t, "--text-muted"), "#ff8800");
        // The theme's `body` rule for the nav colour loses to nothing: the
        // app declares it on `body` too, and the theme's comes later.
        assert_eq!(color(&t, "--nav-item-color"), "#ff8800");
        assert_eq!(color(&t, "--mine"), "#ff8800");
    }
}
