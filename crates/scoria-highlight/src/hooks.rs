//! The tokenize hooks a language registers with Prism, ported.
//!
//! Most of a Prism language is its grammar, but a few finish the job in a
//! hook that runs round `tokenize`, and Obsidian runs them: what a
//! highlighted block shows is the grammar's tokens *after* the hooks. Three
//! of them change what a block paints, and all three are here:
//!
//! - **PHP** (`prism-php.js` over `prism-markup-templating.js`): code that
//!   holds a `<?` is markup with PHP islands, so each `<?…?>` is swapped
//!   for a placeholder, the rest is tokenized as markup, and the islands go
//!   back in as `php` tokens.
//!
//! - **JSX** (`prism-jsx.js`, for `jsx` and `tsx`): text between a tag's
//!   open and close is prose, not code, so whatever the JavaScript grammar
//!   matched in it — a keyword, a string — is dropped back to one
//!   `plain-text` token. Without it, `<p>if "x"</p>` paints `if` as a
//!   keyword.
//! - **Markdown** (`prism-markdown.js`): a fenced block's body is
//!   highlighted with the fence's own language. Prism does this in two
//!   steps — `after-tokenize` gives the `code-block` token a
//!   `language-xxxx` class and the `wrap` hook highlights its content when
//!   it is stringified — and here both happen at once on the tokens, which
//!   is what lets a painter see the nested runs.
//!
//! Two are not ported. GraphQL's only adds aliases no theme rule styles, so
//! it changes no colour. `js-templates`'s does: it highlights the body of a
//! tagged template (`` html`…` ``, `` css`…` ``, `` sql`…` ``) in the
//! language its tag names, and without it that body is one uncoloured
//! `embedded-code` token between its backticks — a gap, filed in
//! [[UI parity backlog]].

use crate::engine::{tokenize, Node, Token};
use crate::languages;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// How deep a Markdown block may hold another highlighted block. A fence
/// cannot hold a fence, so one level is all a real note reaches; the limit
/// only guards the recursion.
const MAX_NESTING: usize = 4;

/// `Prism.highlight` without the stringify: `code` tokenized as the
/// language `name` resolves to, with that language's hooks run. `None`
/// when no grammar is ported for the name.
pub fn tokenize_language(code: &str, name: &str) -> Option<Vec<Node>> {
    tokenize_nested(code, name, 0)
}

fn tokenize_nested(code: &str, name: &str, depth: usize) -> Option<Vec<Node>> {
    let (id, grammar) = languages::resolve(name)?;
    if id == "php" && code.contains("<?") {
        return Some(php_in_markup(code));
    }
    let mut nodes = tokenize(code, grammar);
    match id {
        "jsx" | "tsx" => walk_jsx(code, &mut nodes),
        "markdown" if depth < MAX_NESTING => walk_markdown(code, &mut nodes, depth),
        _ => {}
    }
    Some(nodes)
}

// ---- JSX ------------------------------------------------------------------

/// What Prism's `content` is when it is a string: a token made without an
/// `inside`, whose content is one run of text.
fn content_str<'t>(text: &'t str, node: Option<&Node>) -> Option<&'t str> {
    match node? {
        Node::Token(t) => match t.content.as_slice() {
            [Node::Text { start, end }] => Some(&text[*start..*end]),
            _ => None,
        },
        Node::Text { .. } => None,
    }
}

/// Prism's `stringifyToken`: the source a node covers.
fn source<'t>(text: &'t str, node: Option<&Node>) -> &'t str {
    node.map_or("", |n| &text[n.start()..n.end()])
}

fn is_text_or_plain(node: &Node) -> bool {
    match node {
        Node::Text { .. } => true,
        Node::Token(t) => t.kind == "plain-text",
    }
}

struct OpenTag<'t> {
    name: &'t str,
    braces: usize,
}

/// `walkTokens` from `prism-jsx.js`.
fn walk_jsx(text: &str, tokens: &mut Vec<Node>) {
    let mut opened: Vec<OpenTag> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut not_tag_nor_brace = false;
        if let Node::Token(t) = &tokens[i] {
            let inner = match t.content.first() {
                Some(Node::Token(inner)) if t.kind == "tag" && inner.kind == "tag" => Some(inner),
                _ => None,
            };
            if let Some(inner) = inner {
                let name = source(text, inner.content.get(1));
                if content_str(text, inner.content.first()) == Some("</") {
                    if opened.last().is_some_and(|o| o.name == name) {
                        opened.pop();
                    }
                } else if content_str(text, t.content.last()) == Some("/>") {
                    // Closes itself: nothing opens.
                } else {
                    opened.push(OpenTag { name, braces: 0 });
                }
            } else if t.kind == "punctuation"
                && !opened.is_empty()
                && content_str(text, Some(&tokens[i])) == Some("{")
            {
                if let Some(top) = opened.last_mut() {
                    top.braces += 1;
                }
            } else if t.kind == "punctuation"
                && opened.last().is_some_and(|o| o.braces > 0)
                && content_str(text, Some(&tokens[i])) == Some("}")
            {
                if let Some(top) = opened.last_mut() {
                    top.braces -= 1;
                }
            } else {
                not_tag_nor_brace = true;
            }
        }
        let is_text = matches!(tokens[i], Node::Text { .. });
        if (not_tag_nor_brace || is_text) && opened.last().is_some_and(|o| o.braces == 0) {
            // Inside a tag and outside any `{…}`: prose. Whatever was
            // matched here goes, and the run joins the text either side.
            let (mut start, mut end) = (tokens[i].start(), tokens[i].end());
            if i + 1 < tokens.len() && is_text_or_plain(&tokens[i + 1]) {
                end = tokens[i + 1].end();
                tokens.remove(i + 1);
            }
            if i > 0 && is_text_or_plain(&tokens[i - 1]) {
                start = tokens[i - 1].start();
                tokens.remove(i - 1);
                i -= 1;
            }
            tokens[i] = Node::Token(Token {
                kind: "plain-text",
                alias: &[],
                start,
                end,
                content: vec![Node::Text { start, end }],
            });
        } else if let Node::Token(t) = &mut tokens[i] {
            walk_jsx(text, &mut t.content);
        }
        i += 1;
    }
}

// ---- PHP in markup ----------------------------------------------------------

/// `prism-php.js`'s `before-tokenize` pattern: one `<?…?>` block, or one
/// left open to the end, with strings and comments that may hold a `?>`
/// stepped over whole.
const PHP_BLOCK: &str = concat!(
    r#"<\?(?:[^"'/#]|/(?![*/])|("|')(?:\\[\s\S]|(?!\1)[^\\])*\1|"#,
    r"(?://|#(?!\[))(?:[^?\n\r]|\?(?!>))*(?=$|\?>|[\r\n])|#\[|/\*(?:[^*]|\*(?!/))*(?:\*/|$))*?(?:\?>|$)",
);

/// One `<?…?>` block: where its placeholder is in the text markup was
/// given, and where the block is in the source.
struct Block {
    placeholder: (usize, usize),
    source: (usize, usize),
}

/// PHP that holds a `<?`, as `prism-markup-templating.js` highlights it:
/// each `<?…?>` block is swapped for a placeholder, the text is tokenized
/// as markup, and each placeholder is swapped back for a `php` token
/// holding the block tokenized as PHP. Without a `<?` the hook does
/// nothing and PHP is tokenized as PHP.
///
/// The engine's nodes are ranges of the text it was given, and markup was
/// given the text with placeholders in it, so the markup's ranges are
/// mapped back onto the source afterwards.
fn php_in_markup(code: &str) -> Vec<Node> {
    static RE: OnceLock<Option<fancy_regex::Regex>> = OnceLock::new();
    let Some(re) = RE.get_or_init(|| fancy_regex::Regex::new(PHP_BLOCK).ok()) else {
        return tokenize(code, &languages::php::PHP);
    };
    let mut blocks: Vec<Block> = Vec::new();
    let mut swapped = String::with_capacity(code.len());
    let mut last = 0;
    let mut index = 0;
    for m in re.find_iter(code) {
        let Ok(m) = m else { break };
        // The first index whose placeholder the source does not already
        // spell out, so a placeholder is never confused with real text.
        let placeholder = loop {
            let p = format!("___PHP{index}___");
            index += 1;
            if !code.contains(&p) {
                break p;
            }
        };
        swapped.push_str(&code[last..m.start()]);
        let at = swapped.len();
        swapped.push_str(&placeholder);
        blocks.push(Block {
            placeholder: (at, swapped.len()),
            source: (m.start(), m.end()),
        });
        last = m.end();
    }
    swapped.push_str(&code[last..]);

    let mut nodes = tokenize(&swapped, &languages::markup::MARKUP);
    let mut next = 0;
    place_php(&mut nodes, &blocks, &mut next);
    remap(code, &mut nodes, &blocks);
    nodes
}

/// `tokenizePlaceholders`'s walk: the next placeholder is looked for in
/// each run of text in turn, and where it is found the run is split round
/// a `php` token. The token's content is filled once the ranges are back
/// in the source's terms, by [`remap`].
fn place_php(tokens: &mut Vec<Node>, blocks: &[Block], next: &mut usize) {
    let mut i = 0;
    while i < tokens.len() {
        if *next >= blocks.len() {
            return;
        }
        let (p_start, p_end) = blocks[*next].placeholder;
        let run = match &tokens[i] {
            Node::Text { start, end } => Some((*start, *end)),
            Node::Token(t) => match t.content.as_slice() {
                [Node::Text { start, end }] => Some((*start, *end)),
                _ => None,
            },
        };
        match run {
            Some((start, end)) if start <= p_start && p_end <= end => {
                *next += 1;
                let mut replacement = Vec::new();
                if start < p_start {
                    let mut before = vec![Node::Text {
                        start,
                        end: p_start,
                    }];
                    place_php(&mut before, blocks, next);
                    replacement.extend(before);
                }
                replacement.push(Node::Token(Token {
                    kind: "php",
                    alias: &["language-php"],
                    start: p_start,
                    end: p_end,
                    content: Vec::new(),
                }));
                if p_end < end {
                    let mut after = vec![Node::Text { start: p_end, end }];
                    place_php(&mut after, blocks, next);
                    replacement.extend(after);
                }
                match &mut tokens[i] {
                    Node::Token(t) => {
                        t.content = replacement;
                        i += 1;
                    }
                    Node::Text { .. } => {
                        let n = replacement.len();
                        tokens.splice(i..i + 1, replacement);
                        i += n;
                    }
                }
            }
            Some(_) => i += 1,
            None => {
                if let Node::Token(t) = &mut tokens[i] {
                    place_php(&mut t.content, blocks, next);
                }
                i += 1;
            }
        }
    }
}

/// Every range taken from the swapped text back to the source's, and each
/// `php` token given its block tokenized as PHP.
fn remap(code: &str, nodes: &mut [Node], blocks: &[Block]) {
    let map = |pos: usize| -> usize {
        let mut shift: isize = 0;
        for b in blocks {
            let ((p_start, p_end), (o_start, o_end)) = (b.placeholder, b.source);
            if pos >= p_end {
                shift += (o_end - o_start) as isize - (p_end - p_start) as isize;
            } else if pos > p_start {
                // Inside a placeholder the markup split, which Prism then
                // leaves in place: the block's start is the nearest the
                // source has.
                return o_start;
            } else {
                break;
            }
        }
        (pos as isize + shift) as usize
    };
    for node in nodes {
        match node {
            Node::Text { start, end } => {
                *start = map(*start);
                *end = map(*end);
            }
            Node::Token(t) => {
                let is_php = t.kind == "php" && t.content.is_empty();
                t.start = map(t.start);
                t.end = map(t.end);
                if is_php {
                    let mut inner = tokenize(&code[t.start..t.end], &languages::php::PHP);
                    for n in &mut inner {
                        n.shift(t.start);
                    }
                    t.content = inner;
                } else {
                    remap(code, &mut t.content, blocks);
                }
            }
        }
    }
}

// ---- Markdown -------------------------------------------------------------

fn walk_markdown(text: &str, tokens: &mut [Node], depth: usize) {
    for node in tokens {
        let Node::Token(t) = node else { continue };
        if t.kind != "code" {
            walk_markdown(text, &mut t.content, depth);
            continue;
        }
        // `token.content` is ``` ` ```, the language, a newline, the block,
        // a newline and ``` ` ```; the grammar admits no other shape with
        // both of the two that matter.
        let lang = match (t.content.get(1), t.content.get(3)) {
            (Some(Node::Token(l)), Some(Node::Token(b)))
                if l.kind == "code-language" && b.kind == "code-block" =>
            {
                content_str(text, t.content.get(1)).map(fence_language)
            }
            _ => None,
        };
        let Some(lang) = lang else { continue };
        let Some(Node::Token(block)) = t.content.get_mut(3) else {
            continue;
        };
        if let Some(alias) = language_class(&lang) {
            block.alias = alias;
        }
        let body = &text[block.start..block.end];
        if let Some(mut inner) = tokenize_nested(body, &lang, depth + 1) {
            for n in &mut inner {
                n.shift(block.start);
            }
            block.content = inner;
        }
    }
}

/// The language a Markdown fence names, as `prism-markdown.js` reads it:
/// `c#` and `f#` become `csharp` and `fsharp`, `c++` becomes `cpp`, and
/// the first word left is lowercased.
fn fence_language(info: &str) -> String {
    let mut s = String::with_capacity(info.len());
    let mut prev_word = false;
    let mut chars = info.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' && prev_word {
            s.push_str("sharp");
            prev_word = false;
            continue;
        }
        if c == '+' && prev_word && chars.peek() == Some(&'+') {
            chars.next();
            s.push_str("pp");
            prev_word = false;
            continue;
        }
        prev_word = c.is_ascii_alphanumeric() || c == '_';
        s.push(c);
    }
    let Some(from) = s.find(|c: char| c.is_ascii_alphabetic()) else {
        return String::new();
    };
    s[from..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// The `language-xxxx` class as a `'static` alias, which is what a token
/// carries. Interned, and capped: the names come from what a note writes,
/// and past the cap a block keeps its colours and only loses the class.
fn language_class(lang: &str) -> Option<&'static [&'static str]> {
    const CAP: usize = 256;
    static TABLE: OnceLock<Mutex<HashMap<String, &'static [&'static str]>>> = OnceLock::new();
    let mut table = TABLE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(found) = table.get(lang) {
        return Some(found);
    }
    if table.len() >= CAP {
        return None;
    }
    let class: &'static str = Box::leak(format!("language-{lang}").into_boxed_str());
    let alias: &'static [&'static str] = Box::leak(Box::new([class]));
    table.insert(lang.to_owned(), alias);
    Some(alias)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fence_language_reads_as_prism_does() {
        assert_eq!(fence_language("js"), "js");
        assert_eq!(fence_language("C#"), "csharp");
        assert_eq!(fence_language("f#"), "fsharp");
        assert_eq!(fence_language("c++"), "cpp");
        assert_eq!(fence_language("  Python title"), "python");
        assert_eq!(fence_language("123"), "");
    }

    #[test]
    fn jsx_text_is_plain() {
        let code = "<p>if \"x\" return</p>";
        let nodes = tokenize_language(code, "jsx").unwrap();
        let html = crate::to_html(code, &nodes);
        assert!(html.contains(r#"<span class="token plain-text">if "x" return</span>"#));
    }
}
