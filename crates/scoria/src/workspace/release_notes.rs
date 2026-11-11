//! "Show release notes" (`app:show-release-notes`): a tab of view type
//! `release-notes` at the end of the strip, as Obsidian 1.15.0 opens one
//! ([[Specification#Show release notes]]): state `{"currentVersion": …}`,
//! icon `lucide-book-up`, titled `Release Notes <version>`.
//!
//! The version is Scoria's and so are the notes. Obsidian fetches its
//! changelog from obsidian.md each time the view opens; Scoria fetches
//! nothing. Its notes are the ones on the site's releases page
//! (`site/sites/scoria/content/releases.html`), compiled in, so the view
//! says what scoria.cc says about the build that shows it and works
//! offline. The page's section for the version is turned into Markdown
//! here — headings and paragraphs, the inline `code`, links and emphasis —
//! and drawn by the reading view through the one parser.
//!
//! A tab is a release-notes tab while it holds the leaf and nothing else:
//! no file and no history. Opening a note in it makes it a note's tab, as
//! it does in Obsidian, and the leaf written for it is then the note's.

use super::*;

/// The leaf's view type.
pub(super) const VIEW_TYPE: &str = "release-notes";

/// The leaf's icon (observed).
const ICON: &str = "lucide-book-up";

/// The version this build reports, the workspace manifest's.
pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The site's releases page, which the notes are cut from.
const RELEASES_PAGE: &str = include_str!("../../../../site/sites/scoria/content/releases.html");

/// Where the releases page is published.
const RELEASES_URL: &str = "https://scoria.cc/releases/";

/// The tab's title and the leaf's: `Release Notes 1.15.0` in Obsidian.
pub(super) fn title(version: &str) -> String {
    format!("Release Notes {version}")
}

/// A new release-notes leaf in Obsidian's shape.
pub(super) fn leaf(version: &str) -> Value {
    json!({
        "id": leaf_id(),
        "type": "leaf",
        "state": {
            "type": VIEW_TYPE,
            "state": {"currentVersion": version},
            "icon": ICON,
            "title": title(version),
        },
    })
}

/// The version a tab shows release notes for, or `None` when it is not a
/// release-notes tab. A leaf without `currentVersion` shows this build's.
pub(super) fn version_of(tab: &Tab) -> Option<String> {
    if tab.path.is_some() || !tab.history.is_empty() || tab.mode == ViewMode::Graph {
        return None;
    }
    let state = tab.leaf.as_ref()?.get("state")?;
    if state.get("type").and_then(Value::as_str) != Some(VIEW_TYPE) {
        return None;
    }
    Some(
        state
            .get("state")
            .and_then(|s| s.get("currentVersion"))
            .and_then(Value::as_str)
            .unwrap_or(VERSION)
            .to_string(),
    )
}

/// The notes for `version` as Markdown, from the compiled-in page.
pub(super) fn notes(version: &str) -> String {
    notes_from(RELEASES_PAGE, version)
}

/// The notes for `version` cut from `page`: the `<section>` whose `<h2>`
/// names the version, or else its row in the table of earlier releases,
/// or else a line saying the page has none. A rule and a link to the
/// published page close every one.
pub(super) fn notes_from(page: &str, version: &str) -> String {
    let mut out = section_notes(page, version)
        .or_else(|| row_notes(page, version))
        .unwrap_or_else(|| {
            format!(
                "# {}\n\nThe releases page has no notes for {version}.\n",
                title(version)
            )
        });
    out.push_str(&format!(
        "\n---\n\nEvery release is listed on [the releases page]({RELEASES_URL}).\n"
    ));
    out
}

/// The section whose heading starts with the version: its heading, then
/// each paragraph and card heading in order.
fn section_notes(page: &str, version: &str) -> Option<String> {
    for section in elements(page, "section") {
        let Some(h2) = elements(section, "h2").next() else {
            continue;
        };
        let heading = inline_markdown(h2);
        let names_it = heading == version
            || heading
                .strip_prefix(version)
                .is_some_and(|rest| rest.starts_with(' '));
        if !names_it {
            continue;
        }
        let mut out = format!("# {heading}\n");
        for (tag, inner) in blocks(section) {
            let text = inline_markdown(inner);
            if text.is_empty() {
                continue;
            }
            match tag {
                "h3" => out.push_str(&format!("\n## {text}\n")),
                _ => out.push_str(&format!("\n{text}\n")),
            }
        }
        return Some(out);
    }
    None
}

/// The version's row of a releases table: version, date, text, link.
fn row_notes(page: &str, version: &str) -> Option<String> {
    for row in elements(page, "tr") {
        let cells: Vec<&str> = elements(row, "td").collect();
        let [name, date, text, link, ..] = cells.as_slice() else {
            continue;
        };
        if inline_text(name) != version {
            continue;
        }
        return Some(format!(
            "# {version}\n\nReleased {}. {}\n\n{}\n",
            inline_markdown(date),
            inline_markdown(text),
            inline_markdown(link)
        ));
    }
    None
}

/// The insides of every `<tag …>…</tag>` in `html`, in order. The page's
/// elements of one name do not nest.
fn elements<'a>(html: &'a str, tag: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    elements_at(html, tag).map(|(_, inner)| inner)
}

/// [`elements`] with the byte offset each one's insides start at.
fn elements_at<'a>(html: &'a str, tag: &'a str) -> impl Iterator<Item = (usize, &'a str)> + 'a {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut from = 0;
    std::iter::from_fn(move || loop {
        let at = from + html[from..].find(&open)? + open.len();
        // `<p` must not match `<pre`.
        if !html[at..].starts_with(['>', ' ', '\n', '\t']) {
            from = at;
            continue;
        }
        let body = at + html[at..].find('>')? + 1;
        let end = body + html[body..].find(&close)?;
        from = end + close.len();
        return Some((body, &html[body..end]));
    })
}

/// A section's paragraphs and card headings, in the order they come.
fn blocks(section: &str) -> Vec<(&'static str, &str)> {
    let mut found: Vec<(usize, &'static str, &str)> = ["p", "h3"]
        .into_iter()
        .flat_map(|tag| elements_at(section, tag).map(move |(at, inner)| (at, tag, inner)))
        .collect();
    found.sort_by_key(|(at, _, _)| *at);
    found
        .into_iter()
        .map(|(_, tag, inner)| (tag, inner))
        .collect()
}

/// The text of an HTML fragment with its tags dropped and its white
/// space collapsed, entities left as written.
fn inline_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        rest = rest[at..].find('>').map_or("", |end| &rest[at + end + 1..]);
    }
    out.push_str(rest);
    collapse(&out)
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An HTML fragment as one line of Markdown: `<code>` becomes a code
/// span, `<a href>` a link, `<strong>`/`<b>` and `<em>`/`<i>` emphasis;
/// other tags go and their text stays. Text outside code has Markdown's
/// and Obsidian's markers escaped, so the page's prose cannot turn into
/// a wikilink, math or a highlight.
fn inline_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut hrefs: Vec<Option<String>> = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(at) = rest.find('<') else {
            out.push_str(&escape(rest));
            break;
        };
        out.push_str(&escape(&rest[..at]));
        let Some(end) = rest[at..].find('>') else {
            out.push_str(&escape(&rest[at..]));
            break;
        };
        let tag = &rest[at + 1..at + end];
        rest = &rest[at + end + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match (name.as_str(), closing) {
            ("code", false) => {
                let end = rest.find("</code>").unwrap_or(rest.len());
                let code = decode_entities(&inline_text(&rest[..end]));
                out.push_str(&code_span(&code));
                rest = rest.get(end + "</code>".len()..).unwrap_or("");
            }
            ("a", false) => {
                hrefs.push(attribute(tag, "href"));
                out.push('[');
            }
            ("a", true) => match hrefs.pop().flatten() {
                Some(href) => out.push_str(&format!("]({})", link_target(&href))),
                None => out.push(']'),
            },
            ("strong" | "b", _) => out.push_str("**"),
            ("em" | "i", _) => out.push('*'),
            _ => {}
        }
    }
    collapse(&out)
}

/// A code span that holds `code`, fenced with more backticks than it has
/// in a row.
fn code_span(code: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in code.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest + 1);
    if longest > 0 {
        format!("{fence} {code} {fence}")
    } else {
        format!("{fence}{code}{fence}")
    }
}

/// A link's destination: the site's own paths made absolute, spaces kept
/// out of the bare form.
fn link_target(href: &str) -> String {
    let href = decode_entities(href);
    let href = match href.strip_prefix('/') {
        Some(path) => format!("https://scoria.cc/{path}"),
        None => href,
    };
    if href.contains([' ', '(', ')']) {
        format!("<{href}>")
    } else {
        href
    }
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[at..].find('"')?;
    Some(tag[at..at + end].to_string())
}

fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Backslash before every character that would start Markdown or one of
/// Obsidian's own constructs mid-line.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '$' | '~' | '=' | '%' | '#' | '^' | '|'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl Workspace {
    /// `app:show-release-notes`: a new tab at the end of the strip, active,
    /// showing this build's notes.
    pub(super) fn show_release_notes(&mut self, cx: &mut Context<Self>) {
        self.flush_save(cx);
        self.stash_active_tab();
        let mut tab = Tab::empty(self.default_mode());
        tab.leaf = Some(leaf(VERSION));
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.load_tab(cx);
    }

    /// The version the active tab shows release notes for, if it is a
    /// release-notes tab. Its slot holds the leaf; the live fields hold
    /// the rest.
    pub(super) fn active_release_notes(&self) -> Option<String> {
        let mut tab = self.tabs.get(self.active_tab)?.clone();
        tab.path = self.current.clone();
        tab.mode = self.mode;
        tab.history = self.history.clone();
        version_of(&tab)
    }

    /// The reading view that draws the notes, built for `version` if it
    /// holds another's. One view serves every group: the notes are the
    /// same text wherever they are shown.
    pub(super) fn release_notes_view(
        &self,
        version: &str,
        cx: &mut Context<Self>,
    ) -> Entity<ReadingView> {
        let view = self.release_notes.clone();
        if self.release_notes_built.borrow().as_deref() != Some(version) {
            if let Some(state) = self.state.clone() {
                let text = notes(version);
                let model = reading::build_with("", &text, state.as_ref(), state.build_options());
                view.update(cx, |r, cx| r.set_note(None, model, cx));
                *self.release_notes_built.borrow_mut() = Some(version.to_string());
            }
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The leaf `app:show-release-notes` adds, in the shape Obsidian 1.15.0
    /// wrote (`results/2026-11-09-app-window-commands.json`, `s11`).
    #[test]
    fn a_new_leaf_is_obsidians_shape_with_scorias_version() {
        let leaf = leaf("0.5.0");
        assert_eq!(leaf["type"], json!("leaf"));
        assert!(leaf["id"].as_str().is_some_and(|id| id.len() == 16));
        assert_eq!(
            leaf["state"],
            json!({
                "type": "release-notes",
                "state": {"currentVersion": "0.5.0"},
                "icon": "lucide-book-up",
                "title": "Release Notes 0.5.0",
            })
        );
        let keys: Vec<&String> = leaf["state"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["type", "state", "icon", "title"]);
    }

    #[test]
    fn a_tab_is_release_notes_only_while_it_holds_nothing_else() {
        let mut tab = Tab::empty(ViewMode::LivePreview);
        assert_eq!(version_of(&tab), None);
        tab.leaf = Some(leaf("0.5.0"));
        assert_eq!(version_of(&tab).as_deref(), Some("0.5.0"));
        assert_eq!(tab.title(), "Release Notes 0.5.0");
        // A note opened in it makes it the note's tab.
        let mut opened = tab.clone();
        opened.path = Some("Note.md".into());
        opened.history = vec![HistoryEntry::new("Note.md".into(), ViewMode::LivePreview)];
        assert_eq!(version_of(&opened), None);
        assert_eq!(opened.title(), "Note");
        // A leaf Obsidian wrote names Obsidian's version, and the tab says so.
        let mut theirs = tab.clone();
        theirs.leaf = Some(json!({
            "id": "0123456789abcdef",
            "type": "leaf",
            "state": {"type": "release-notes", "state": {"currentVersion": "1.15.0"}},
        }));
        assert_eq!(theirs.title(), "Release Notes 1.15.0");
    }

    /// A restored release-notes leaf is a tab, and is written back as it
    /// was read.
    #[test]
    fn a_release_notes_leaf_round_trips_through_workspace_json() {
        let leaf = json!({
            "id": "4d2f1c0b9a8e7d6c",
            "type": "leaf",
            "state": {
                "type": "release-notes",
                "state": {"currentVersion": "0.5.0"},
                "icon": "lucide-book-up",
                "title": "Release Notes 0.5.0",
            },
        });
        let (id, tab) = tab_from_leaf(&leaf, ViewMode::LivePreview, &|_| true).unwrap();
        assert_eq!(id, "4d2f1c0b9a8e7d6c");
        assert_eq!(version_of(&tab).as_deref(), Some("0.5.0"));
        assert_eq!(leaf_json(&tab), leaf);
        // And a new one, written and read again.
        let mut fresh = Tab::empty(ViewMode::Reading);
        fresh.leaf = Some(super::leaf(VERSION));
        let written = leaf_json(&fresh);
        assert_eq!(written["state"]["type"], json!("release-notes"));
        let (_, again) = tab_from_leaf(&written, ViewMode::Reading, &|_| true).unwrap();
        assert_eq!(leaf_json(&again), written);
    }

    /// The compiled-in page has a section for this very build, so the
    /// view never falls back on a tagged release.
    #[test]
    fn the_releases_page_has_notes_for_this_build() {
        let notes = notes(VERSION);
        let first = notes.lines().next().unwrap();
        assert!(
            first.starts_with(&format!("# {VERSION} ")),
            "the releases page has a section for {VERSION}: {first}"
        );
        assert!(!notes.contains('<'), "no tag left: {notes}");
        assert!(notes.ends_with("[the releases page](https://scoria.cc/releases/).\n"));
    }

    #[test]
    fn a_section_becomes_headings_and_paragraphs() {
        let page = r#"<section class="hero"><h1>Releases</h1><p>Intro</p></section>
<section class="band">
  <h2>1.2.0 — a test</h2>
  <p class="lede">
    Released 1 May. Uses <code>a_b</code> and
    <a href="/docs/">the manual</a>, <strong>bold</strong>.
  </p>
  <div class="grid"><div class="card"><span class="k">K</span>
    <h3>Card <code>x</code></h3>
    <p>Body with [[brackets]] and $5 and ==mark==.</p>
  </div></div>
  <pre>not a paragraph</pre>
</section>"#;
        assert_eq!(
            notes_from(page, "1.2.0"),
            "# 1.2.0 — a test\n\
             \n\
             Released 1 May. Uses `a_b` and [the manual](https://scoria.cc/docs/), **bold**.\n\
             \n\
             ## Card `x`\n\
             \n\
             Body with \\[\\[brackets\\]\\] and \\$5 and \\=\\=mark\\=\\=.\n\
             \n\
             ---\n\
             \n\
             Every release is listed on [the releases page](https://scoria.cc/releases/).\n"
        );
        // `1.2` does not name `1.2.0`'s section.
        assert!(notes_from(page, "1.2").starts_with("# Release Notes 1.2\n"));
    }

    #[test]
    fn a_version_in_the_table_only_is_its_row() {
        let page = r#"<table><tbody>
<tr><td><strong>0.4.0</strong></td><td>5 Nov 2026</td><td>Bases and
<code>PDFs</code>.</td><td><a href="https://example.com/v0.4.0">v0.4.0</a></td></tr>
</tbody></table>"#;
        assert_eq!(
            notes_from(page, "0.4.0"),
            "# 0.4.0\n\nReleased 5 Nov 2026. Bases and `PDFs`.\n\n\
             [v0.4.0](https://example.com/v0.4.0)\n\
             \n---\n\nEvery release is listed on [the releases page](https://scoria.cc/releases/).\n"
        );
    }

    #[test]
    fn an_unknown_version_says_the_page_has_no_notes() {
        let notes = notes_from("<section><h2>0.1.0</h2></section>", "9.9.9");
        assert!(notes
            .starts_with("# Release Notes 9.9.9\n\nThe releases page has no notes for 9.9.9.\n"));
    }

    #[test]
    fn code_keeps_its_characters_and_decodes_entities() {
        assert_eq!(
            inline_markdown("<code>ragtux &lt;r@x&gt;</code>"),
            "`ragtux <r@x>`"
        );
        assert_eq!(inline_markdown("<code>a`b</code>"), "`` a`b ``");
    }
}
