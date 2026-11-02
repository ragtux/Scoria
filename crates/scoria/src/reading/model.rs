//! Reading-view model: an owned render tree built from the shared parser's
//! [`Document`], so the view can hold it across frames without borrowing the
//! note text. Mirrors what `scoria_markdown::render_html` emits (the same
//! tree, the same footnote numbering, the same embed rules) but as data a
//! GPUI element tree can be built from: paragraphs become runs of styled
//! text with clickable ranges, embeds are resolved and inlined recursively,
//! and footnote definitions are gathered at the end.

use scoria_formats::properties::Property;
use scoria_formats::FileKind;
use scoria_markdown::{
    canonical_callout_type, html::has_scheme, html::percent_decode, parse, plain_text, Alignment,
    Block, BlockKind, Document, Fold, HighlightColor, Inline, InlineKind, LinkKind, ListItem,
    ListKind, Subpath,
};
use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;

/// Embedded notes nest at most this deep; Obsidian stops at a similar depth.
pub const MAX_EMBED_DEPTH: usize = 4;

/// The embedded bases named while building one model.
///
/// A base is drawn by a live view of its own (`bases::embed`), so the model
/// keeps nothing of its rows; what it settles here is whether the file is
/// there — asked once per path however many times a note names it, keyed
/// case-insensitively like every other vault path — and the **ordinal** of
/// each embed among the ones naming the same base and view, so the same
/// `![[x.base]]` written twice is two views rather than one drawn twice.
#[derive(Default)]
struct BaseCache {
    exists: RefCell<Vec<(String, bool)>>,
    counts: RefCell<Vec<(EmbedKey, usize)>>,
}

/// A base and view as the ordinals are counted: lowercased, like every
/// other vault path comparison.
type EmbedKey = (String, Option<String>);

impl BaseCache {
    fn resolve(
        &self,
        path: &str,
        view: Option<&str>,
        vault: &dyn EmbedSource,
    ) -> Option<crate::bases::EmbeddedBase> {
        // The borrow ends before the ask: nothing is held across it.
        let known = self
            .exists
            .borrow()
            .iter()
            .find(|(p, _)| p.eq_ignore_ascii_case(path))
            .map(|(_, exists)| *exists);
        let exists = match known {
            Some(exists) => exists,
            None => {
                let exists = vault.has_base(path);
                self.exists.borrow_mut().push((path.to_string(), exists));
                exists
            }
        };
        if !exists {
            return None;
        }
        let key = (path.to_lowercase(), view.map(str::to_lowercase));
        let mut counts = self.counts.borrow_mut();
        let nth = match counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                counts.push((key, 0));
                0
            }
        };
        Some(crate::bases::EmbeddedBase {
            path: path.to_string(),
            view: view.map(str::to_string),
            nth,
        })
    }
}

/// What the model builder needs from the vault to resolve embeds.
pub trait EmbedSource {
    /// Resolves a link body written in `source_path` to a vault path.
    fn resolve(&self, linktext: &str, source_path: &str) -> Option<String>;
    /// Reads a markdown file's text.
    fn read(&self, path: &str) -> Option<String>;
    /// Absolute path of a vault file, for images.
    fn absolute(&self, path: &str) -> PathBuf;
    /// Whether the `.base` at `path` can be read. An embedded base is drawn
    /// by a live view that reads the file itself (`bases::embed`), so this
    /// is all the model asks; the default answers no, which is what a source
    /// with no vault behind it (a test's stub) has to answer.
    fn has_base(&self, _path: &str) -> bool {
        false
    }
}

/// Formatting that applies to a run of text; several may apply at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub highlight: bool,
    /// One of the six colours a `==🔴word==` highlight names, or none for
    /// the plain `==word==`.
    pub highlight_color: Option<HighlightColor>,
    pub code: bool,
    pub link: bool,
    pub tag: bool,
    pub math: bool,
    pub footnote: bool,
    /// Raw HTML the model cannot map onto a style: drawn as muted source.
    pub html: bool,
    /// `<u>`: underlined in the text colour, where a link is underlined in
    /// the link colour.
    pub underline: bool,
    /// `<kbd>`: drawn the way inline code is.
    pub kbd: bool,
    /// `<sup>` and `<sub>`. A gpui text run cannot shift its baseline or
    /// change its size, so for now these carry the markup's meaning without
    /// a look of their own.
    pub sup: bool,
    pub sub: bool,
}

impl InlineStyle {
    pub fn is_plain(&self) -> bool {
        *self == InlineStyle::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// A wikilink body or a vault-relative markdown link destination,
    /// resolved by the workspace relative to the note it appears in.
    Internal(String),
    External(String),
    Tag(String),
    /// Jump to footnote number `n` at the bottom of the note.
    Footnote(usize),
}

/// One line-wrapped run of inline content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RichText {
    /// Unique within one model, for GPUI element ids.
    pub id: usize,
    pub text: String,
    /// Non-overlapping styled byte ranges in order; gaps are plain.
    pub runs: Vec<(Range<usize>, InlineStyle)>,
    pub links: Vec<(Range<usize>, LinkTarget)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Text(RichText),
    Embed(Embed),
    /// `$$x$$` written inside a paragraph: display math, drawn as its own
    /// centred block the way MathJax's `display="true"` container is.
    Math(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Embed {
    pub kind: EmbedKind,
    /// The link body as written, for opening the target.
    pub link: String,
    pub alt: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmbedKind {
    Image(PathBuf),
    Note {
        path: String,
        /// `Note`, `Note > Heading`, or `Note > ^id`.
        title: String,
        blocks: Vec<RBlock>,
    },
    /// A `.base`: the path, the view its subpath names and its ordinal in
    /// the note. The rows are the live view's (`bases::embed`).
    Base(crate::bases::EmbeddedBase),
    /// A `.pdf`: the path and the subpath, nothing read. The pages come
    /// from the document cache when the embed is drawn (`pdf::embed`).
    Pdf(crate::pdf::embed::PdfEmbed),
    /// The target file does not exist.
    Missing,
    /// The target exists but Scoria cannot render it inline yet.
    Unsupported(FileKind),
    TooDeep,
}

/// Calls `f` on every embed in `blocks`, in document order, descending into
/// embedded notes, quotes, callouts, lists and footnotes: the walk the
/// control socket's dumps and the canvas's dependency lists share.
pub fn each_embed<'a>(blocks: &'a [RBlock], f: &mut dyn FnMut(&'a Embed)) {
    for block in blocks {
        match block {
            RBlock::Paragraph(segments) => {
                for segment in segments {
                    if let Segment::Embed(embed) = segment {
                        f(embed);
                        if let EmbedKind::Note { blocks, .. } = &embed.kind {
                            each_embed(blocks, f);
                        }
                    }
                }
            }
            RBlock::Quote(children) | RBlock::Callout { children, .. } => each_embed(children, f),
            RBlock::List { items, .. } => {
                for item in items {
                    each_embed(&item.children, f);
                }
            }
            RBlock::Footnotes(notes) => {
                for (_, body) in notes {
                    each_embed(body, f);
                }
            }
            _ => {}
        }
    }
}

/// Where a task's `[ ]` marker sits in the note: a zero-based line and the
/// byte column inside it. The editor's buffer drops a BOM and rewrites
/// `\r\n`, so a byte offset into the file is not one of its offsets, but
/// every line of the file is still a line of the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskMarker {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RItem {
    pub task: Option<char>,
    /// Where the three bytes `[`, status, `]` are, for a task of the note
    /// itself. An item that came out of an embed has none: its bytes
    /// belong to another file, so clicking it must not write this one.
    pub marker: Option<TaskMarker>,
    pub number: Option<u64>,
    pub children: Vec<RBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RBlock {
    /// The properties block. The entries are the parsed frontmatter with
    /// their byte spans; the renderer resolves each key's type against the
    /// vault's `types.json` and builds the rows, because the same block is
    /// drawn read-only in the reading view and editable in Live Preview.
    Properties(Vec<Property>),
    Paragraph(Vec<Segment>),
    Heading {
        level: u8,
        text: RichText,
        plain: String,
    },
    ThematicBreak,
    Code {
        /// Unique within one model, for GPUI element ids.
        id: usize,
        language: Option<String>,
        text: String,
        /// What the copy button writes: the body with `\n` endings and
        /// exactly one trailing newline removed, as Obsidian's copies.
        copy: String,
        /// A raw `<pre>` wraps its lines (observed: `white-space: pre-wrap`,
        /// no sideways scroll), where a fence keeps its columns.
        wrap: bool,
        /// Whether the block carries a copy button: a fence always, a raw
        /// `<pre>` only with a `<code>` child (observed).
        copyable: bool,
    },
    /// A raw `<details>`: its `<summary>` as the row that folds the rest,
    /// open when the tag says `open`.
    Details {
        /// The summary's own text id, which is also the fold's.
        id: usize,
        open: bool,
        summary: RichText,
        body: Vec<RBlock>,
    },
    Html(String),
    Quote(Vec<RBlock>),
    Callout {
        /// Canonical type, one of Obsidian's built-ins.
        kind: &'static str,
        fold: Option<bool>,
        title: RichText,
        children: Vec<RBlock>,
    },
    List {
        tight: bool,
        items: Vec<RItem>,
    },
    Table {
        alignments: Vec<Alignment>,
        header: Vec<RichText>,
        rows: Vec<Vec<RichText>>,
        /// The bytes each cell stands for, the header row first, so Live
        /// Preview can edit a cell in place.
        spans: Vec<Vec<Range<usize>>>,
    },
    Math(String),
    /// `(number, body)` in reference order, rendered after a rule.
    Footnotes(Vec<(usize, Vec<RBlock>)>),
}

/// A built note.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub blocks: Vec<RBlock>,
    /// Source byte offset where each top-level block starts, for mapping
    /// cache positions (outline, backlinks) to list items. One entry per
    /// block in `blocks`, built from the same list: a comment or a
    /// footnote definition is dropped from both or from neither, or every
    /// jump past one lands on the wrong block.
    pub starts: Vec<usize>,
    /// Source byte offset of footnote `n`'s definition, `n - 1` indexed, in
    /// the order the Footnotes section lists them. A footnote written
    /// inline has no definition of its own and carries the offset of the
    /// reference instead.
    pub footnotes: Vec<usize>,
}

impl Model {
    /// Index of the top-level block containing byte `offset`.
    pub fn block_at(&self, offset: usize) -> Option<usize> {
        if self.starts.is_empty() {
            return None;
        }
        Some(match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        })
    }
}

/// Rendering settings that come from `app.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// Obsidian's "Strict line breaks": a single newline inside a paragraph
    /// is a space rather than a line break.
    pub strict_line_breaks: bool,
}

/// Builds the model for `text`, the content of `source_path`, with the default settings.
#[cfg(test)]
pub fn build(source_path: &str, text: &str, vault: &dyn EmbedSource) -> Model {
    build_with(source_path, text, vault, BuildOptions::default())
}

/// Builds the model for `text`, the content of `source_path`.
pub fn build_with(
    source_path: &str,
    text: &str,
    vault: &dyn EmbedSource,
    opts: BuildOptions,
) -> Model {
    let doc = parse(text);
    let bases = BaseCache::default();
    let mut b = Builder {
        vault,
        source_path,
        text: doc.source,
        opts,
        depth: 0,
        next_id: 0,
        footnotes: FootnoteState::default(),
        line_starts: None,
        bases: &bases,
    };
    b.footnotes.definitions = collect_definitions(&doc.blocks);
    let (mut blocks, mut starts) = b.blocks_with_starts(&doc.blocks);
    let mut footnotes = Vec::new();
    if let Some((section, offsets)) = b.footnote_section(&doc) {
        // The appended section needs a start of its own, or an offset
        // inside a definition resolves past the end of the list.
        starts.push(offsets.iter().copied().min().unwrap_or(doc.source.len()));
        blocks.push(section);
        footnotes = offsets;
    }
    Model {
        blocks,
        starts,
        footnotes,
    }
}

/// Builds one part of `text`, the content of `source_path`: the section
/// under a heading path (`#A#B`, from the heading to the next of the same
/// or a higher level) or the one block carrying an id (`#^id`). `None`
/// when the note has no such heading or block. A canvas file card with a
/// `subpath` shows exactly this, so unlike an embedded section it is built
/// at depth 0: its task markers are positions in the note itself and its
/// embeds nest as deep as the note's own would.
pub(crate) fn build_section(
    source_path: &str,
    text: &str,
    subpath: &Subpath<'_>,
    vault: &dyn EmbedSource,
    opts: BuildOptions,
) -> Option<Model> {
    let doc = parse(text);
    let selected: Vec<Block<'_>> = match subpath {
        Subpath::Headings(names) => section(&doc.blocks, names)?.to_vec(),
        Subpath::Block(id) => vec![block_by_id(&doc.blocks, id)?.clone()],
    };
    let bases = BaseCache::default();
    let mut b = Builder {
        vault,
        source_path,
        text: doc.source,
        opts,
        depth: 0,
        next_id: 0,
        footnotes: FootnoteState::default(),
        line_starts: None,
        bases: &bases,
    };
    b.footnotes.definitions = collect_definitions(&doc.blocks);
    let (mut blocks, mut starts) = b.blocks_with_starts(&selected);
    let mut footnotes = Vec::new();
    if let Some((section, offsets)) = b.footnote_section(&doc) {
        starts.push(offsets.iter().copied().min().unwrap_or(doc.source.len()));
        blocks.push(section);
        footnotes = offsets;
    }
    Some(Model {
        blocks,
        starts,
        footnotes,
    })
}

/// A top-level block Live Preview renders in place of its source lines:
/// a table, a math block, or a paragraph that is one embed.
#[derive(Debug, Clone, PartialEq)]
pub struct Widget {
    /// Byte span of the block in the note (newline after it excluded).
    pub span: Range<usize>,
    pub block: RBlock,
}

/// The blocks of `doc` (the content of `source_path`) that Live Preview
/// shows as widgets, built with ids unique among them.
pub fn build_widgets(
    source_path: &str,
    doc: &Document<'_>,
    vault: &dyn EmbedSource,
    opts: BuildOptions,
) -> Vec<Widget> {
    let bases = BaseCache::default();
    let mut b = Builder {
        vault,
        source_path,
        text: doc.source,
        opts,
        depth: 0,
        next_id: 0,
        footnotes: FootnoteState::default(),
        line_starts: None,
        bases: &bases,
    };
    b.footnotes.definitions = collect_definitions(&doc.blocks);
    let mut out = Vec::new();
    for block in &doc.blocks {
        let widget = match &block.kind {
            // The properties block is a widget so Live Preview can draw it as
            // an editable table instead of raw YAML.
            BlockKind::Yaml { .. } => true,
            BlockKind::Table { .. } | BlockKind::Math { .. } => true,
            // A callout is drawn as the same tinted block the reading view
            // draws; a cursor inside it still brings its source lines back.
            BlockKind::Callout { .. } => true,
            // Only the raw HTML the renderer can actually draw earns a
            // widget. Anything else -- a comment, a script, a bare `<div>` --
            // would fold its source lines away to show the same muted text.
            BlockKind::HtmlBlock { content } => {
                html_images(content).is_some() || html_block_renders(content)
            }
            BlockKind::Paragraph { content } => match content.as_slice() {
                [one] => matches!(
                    &one.kind,
                    InlineKind::Wikilink { embed: true, .. }
                        | InlineKind::Image {
                            kind: LinkKind::Inline,
                            ..
                        }
                ),
                _ => false,
            },
            _ => false,
        };
        if !widget {
            continue;
        }
        if let Some(built) = b.block(block, false) {
            out.push(Widget {
                span: block.span.start..block.span.end,
                block: built,
            });
        }
    }
    out
}

/// Builds the model of an embedded note or section with the given nesting.
///
/// Eight arguments, because an embed is built from the whole nesting it sits
/// in: the file it came from, its blocks, how deep it is, the id counter and
/// the base cache it shares with every sibling, the vault, and the options.
/// Bundling them into a struct would only move the same list one line down.
#[allow(clippy::too_many_arguments)]
fn build_embedded(
    source_path: &str,
    doc: &Document<'_>,
    blocks: &[Block<'_>],
    depth: usize,
    next_id: &mut usize,
    bases: &BaseCache,
    vault: &dyn EmbedSource,
    opts: BuildOptions,
) -> Vec<RBlock> {
    let mut b = Builder {
        vault,
        source_path,
        text: doc.source,
        opts,
        depth,
        next_id: *next_id,
        footnotes: FootnoteState::default(),
        line_starts: None,
        bases,
    };
    b.footnotes.definitions = collect_definitions(&doc.blocks);
    let mut out = b.blocks(blocks, false);
    // An embedded section is not jumped into, so its footnote offsets are
    // of no use to anyone.
    if let Some((section, _)) = b.footnote_section(doc) {
        out.push(section);
    }
    *next_id = b.next_id;
    out
}

#[derive(Default)]
struct FootnoteState {
    /// Labels present in the document (first definition wins).
    definitions: Vec<String>,
    /// Labels in order of first reference; index + 1 is the number.
    numbers: Vec<String>,
    /// Inline footnotes keyed by generated label.
    inline: Vec<(String, RichText)>,
}

fn collect_definitions(blocks: &[Block<'_>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    scoria_markdown::walk_blocks(blocks, &mut |b| {
        if let BlockKind::FootnoteDefinition { label, .. } = &b.kind {
            if !out.iter().any(|l| l == label) {
                out.push(label.to_string());
            }
        }
    });
    out
}

struct Builder<'v> {
    vault: &'v dyn EmbedSource,
    /// Shared with every nested builder of the same model, so an embedded
    /// base is resolved once however many times the note names it.
    bases: &'v BaseCache,
    source_path: &'v str,
    /// The whole note, so the properties block can carry file byte spans.
    text: &'v str,
    opts: BuildOptions,
    depth: usize,
    next_id: usize,
    footnotes: FootnoteState,
    /// Where each line of `text` starts, built the first time a task
    /// marker asks for its position.
    line_starts: Option<Vec<usize>>,
}

impl<'v> Builder<'v> {
    /// The line and column of a byte offset in the note.
    fn marker_at(&mut self, offset: usize) -> TaskMarker {
        let text = self.text;
        let starts = self.line_starts.get_or_insert_with(|| {
            std::iter::once(0)
                .chain(text.match_indices('\n').map(|(i, _)| i + 1))
                .collect()
        });
        let line = match starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        TaskMarker {
            line,
            col: offset - starts[line],
        }
    }

    fn id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    fn blocks(&mut self, blocks: &[Block<'_>], tight: bool) -> Vec<RBlock> {
        blocks.iter().filter_map(|b| self.block(b, tight)).collect()
    }

    /// The top level, where each block kept has to keep its source offset
    /// too: `block` returns `None` for a comment and for a footnote
    /// definition, so deriving the offsets from the *parsed* list instead
    /// would leave the two lists one apart for every block dropped.
    fn blocks_with_starts(&mut self, blocks: &[Block<'_>]) -> (Vec<RBlock>, Vec<usize>) {
        let mut out = Vec::new();
        let mut starts = Vec::new();
        for block in blocks {
            if let Some(built) = self.block(block, false) {
                out.push(built);
                starts.push(block.span.start);
            }
        }
        (out, starts)
    }

    fn block(&mut self, block: &Block<'_>, _tight: bool) -> Option<RBlock> {
        Some(match &block.kind {
            BlockKind::Yaml { .. } => RBlock::Properties(
                scoria_cache::note_properties(self.text)
                    .map(|p| p.entries)
                    .unwrap_or_default(),
            ),
            BlockKind::Paragraph { content } => RBlock::Paragraph(self.segments(content)),
            BlockKind::Heading { level, content, .. } => RBlock::Heading {
                level: *level,
                text: self.rich(content),
                plain: plain_text(content).trim().to_string(),
            },
            BlockKind::ThematicBreak => RBlock::ThematicBreak,
            BlockKind::CodeBlock { info, content, .. } => {
                // Obsidian renders and copies a code block with `\n`
                // endings whatever the file holds, and its copy button
                // drops exactly one trailing newline rather than trimming.
                let body = if content.contains('\r') {
                    content.replace("\r\n", "\n")
                } else {
                    content.to_string()
                };
                RBlock::Code {
                    id: self.id(),
                    language: info
                        .as_deref()
                        .and_then(|i| i.split_whitespace().next())
                        .map(str::to_string),
                    text: body.trim_end_matches('\n').to_string(),
                    copy: body.strip_suffix('\n').unwrap_or(&body).to_string(),
                    wrap: false,
                    copyable: true,
                }
            }
            // A block of nothing but comments and scripts draws nothing at
            // all. Obsidian's sanitizer takes a `<script>` out whole --
            // element and script text, with the text after it kept
            // ([[Specification#Sanitizer, 26 single-construct cases]]) --
            // and a comment is a comment node, which no DOM paints. Showing
            // either as muted source is the one case where the fallback is
            // not merely plain but wrong. (The comment half is the DOM's
            // definition rather than a probe's reading; no case covers it.)
            BlockKind::HtmlBlock { content } if html_block_is_invisible(content) => return None,
            BlockKind::HtmlBlock { content } => match html_images(content) {
                // Obsidian resolves a raw `<img src>` like a wikilink target
                // and renders the run of them as one block, so they go
                // through the same embeds a markdown image does.
                Some(imgs) => {
                    let mut segments = Vec::with_capacity(imgs.len());
                    for img in imgs {
                        let embed = self.embed(&img.src, img.alt, img.width, img.height);
                        segments.push(Segment::Embed(embed));
                    }
                    RBlock::Paragraph(segments)
                }
                None => self.html_block(content),
            },
            BlockKind::BlockQuote { children } => RBlock::Quote(self.blocks(children, false)),
            BlockKind::Callout {
                callout_type,
                fold,
                title,
                children,
            } => {
                let title = if title.is_empty() {
                    // Shared with the reading view's HTML and with Live
                    // Preview: this was a fourth copy of the derivation, and
                    // the only one that still left `-` in place, so a
                    // `[!bad-type]` callout read "Bad-type" here.
                    RichText {
                        id: self.id(),
                        text: scoria_markdown::callout_default_title(callout_type),
                        ..RichText::default()
                    }
                } else {
                    // The title is block-parsed; this view paints one run of
                    // rich text, so flatten the tree's inlines into it. A
                    // heading or a list written as a title therefore shows its
                    // text without the block styling Obsidian gives it -- see
                    // the callout block in docs/scoria/Specification.md.
                    let flat: Vec<Inline<'_>> = scoria_markdown::inline_runs(title)
                        .into_iter()
                        .flat_map(|run| run.iter().cloned())
                        .collect();
                    self.rich(&flat)
                };
                RBlock::Callout {
                    kind: canonical_callout_type(callout_type),
                    fold: fold.map(|f| f == Fold::Collapsed),
                    title,
                    children: self.blocks(children, false),
                }
            }
            BlockKind::List { kind, tight, items } => RBlock::List {
                tight: *tight,
                items: self.items(kind, items, *tight),
            },
            BlockKind::Table {
                alignments,
                header,
                rows,
            } => RBlock::Table {
                alignments: alignments.clone(),
                header: header.iter().map(|c| self.rich(&c.content)).collect(),
                rows: rows
                    .iter()
                    .map(|r| r.iter().map(|c| self.rich(&c.content)).collect())
                    .collect(),
                spans: std::iter::once(header)
                    .chain(rows.iter())
                    .map(|r| r.iter().map(|c| c.span.start..c.span.end).collect())
                    .collect(),
            },
            BlockKind::Math { content } => RBlock::Math(content.trim_matches('\n').to_string()),
            BlockKind::Comment { .. } | BlockKind::FootnoteDefinition { .. } => return None,
        })
    }

    fn items(&mut self, kind: &ListKind, items: &[ListItem<'_>], tight: bool) -> Vec<RItem> {
        let mut number = match kind {
            ListKind::Ordered { start, .. } => Some(*start),
            ListKind::Bullet { .. } => None,
        };
        items
            .iter()
            .map(|item| {
                let n = number;
                if let Some(n) = number.as_mut() {
                    *n += 1;
                }
                let marker = item
                    .task_span
                    .filter(|_| self.depth == 0)
                    .map(|s| self.marker_at(s.start));
                RItem {
                    task: item.task,
                    marker,
                    number: n,
                    children: self.blocks(&item.children, tight),
                }
            })
            .collect()
    }

    /// A paragraph's content split into text runs and block-level embeds.
    fn segments(&mut self, content: &[Inline<'_>]) -> Vec<Segment> {
        let mut out = Vec::new();
        let mut acc = RichBuilder::default();
        let is_display_math =
            |inline: &Inline<'_>| matches!(inline.kind, InlineKind::Math { display: true, .. });
        for (ix, inline) in content.iter().enumerate() {
            match &inline.kind {
                // The space on either side of a display block inside a
                // paragraph is the block's margin, not text to set: an HTML
                // renderer collapses it at the block's edge.
                InlineKind::Text(t)
                    if ix
                        .checked_sub(1)
                        .is_some_and(|p| is_display_math(&content[p]))
                        || content.get(ix + 1).is_some_and(is_display_math) =>
                {
                    let mut t: &str = t;
                    if ix
                        .checked_sub(1)
                        .is_some_and(|p| is_display_math(&content[p]))
                    {
                        t = t.trim_start();
                    }
                    if content.get(ix + 1).is_some_and(is_display_math) {
                        t = t.trim_end();
                    }
                    acc.push(t);
                }
                InlineKind::Wikilink { body, embed: true } => {
                    self.flush(&mut acc, &mut out);
                    let embed = self.wikilink_embed(body);
                    out.push(Segment::Embed(embed));
                }
                InlineKind::Image {
                    destination,
                    kind: LinkKind::Inline,
                    children,
                    ..
                } if !has_scheme(destination) => {
                    self.flush(&mut acc, &mut out);
                    // `![alt|100](pic.png)` sizes the picture the way
                    // `![[pic.png|100]]` does: the alt is read as an alias.
                    let (alt, width, height) = alias_parts(&plain_text(children));
                    let dest = percent_decode(destination).into_owned();
                    let embed = self.embed(&dest, alt.unwrap_or_default(), width, height);
                    out.push(Segment::Embed(embed));
                }
                InlineKind::Math {
                    content,
                    display: true,
                } => {
                    self.flush(&mut acc, &mut out);
                    out.push(Segment::Math(content.to_string()));
                }
                _ => self.inline(inline, &mut acc),
            }
        }
        self.flush(&mut acc, &mut out);
        out
    }

    fn flush(&mut self, acc: &mut RichBuilder, out: &mut Vec<Segment>) {
        let text = std::mem::take(acc).finish(self.id());
        // Whitespace between two embeds would render as an empty line.
        if !text.text.trim().is_empty() {
            out.push(Segment::Text(text));
        }
    }

    fn rich(&mut self, content: &[Inline<'_>]) -> RichText {
        let mut acc = RichBuilder::default();
        for inline in content {
            self.inline(inline, &mut acc);
        }
        acc.finish(self.id())
    }

    /// Rich text from a run of raw HTML: the tags go through the same
    /// overlay inline HTML takes, the text between them has its entities
    /// decoded and its newlines read as spaces.
    fn rich_html(&mut self, html: &str) -> RichText {
        let mut acc = RichBuilder::default();
        let mut rest = html.trim();
        while !rest.is_empty() {
            match rest.find('<') {
                Some(0) => {
                    let end = rest.find('>').map(|i| i + 1).unwrap_or(rest.len());
                    acc.html(&rest[..end]);
                    rest = &rest[end..];
                }
                Some(at) => {
                    acc.push(&html_text(&rest[..at]));
                    rest = &rest[at..];
                }
                None => {
                    acc.push(&html_text(rest));
                    rest = "";
                }
            }
        }
        acc.finish(self.id())
    }

    /// A raw HTML block that is not a run of images: a `<pre>` as a wrapped
    /// code block, a `<details>` as a fold, a container (`<div>`, `<p>`,
    /// `<center>`, ...) as a paragraph of its content, and everything else
    /// as the muted source it was ([[Specification#Observed raw HTML behavior, Obsidian 1.14.4 (installer build 1.13.7)]]).
    fn html_block(&mut self, content: &str) -> RBlock {
        match classify_html_block(content) {
            HtmlBlockKind::Pre { text, code } => RBlock::Code {
                id: self.id(),
                language: None,
                copy: text.clone(),
                text,
                wrap: true,
                copyable: code,
            },
            HtmlBlockKind::Details {
                open,
                summary,
                body,
            } => {
                let summary = self.rich_html(&summary);
                let body = if body.trim().is_empty() {
                    Vec::new()
                } else {
                    let rich = self.rich_html(&body);
                    vec![RBlock::Paragraph(vec![Segment::Text(rich)])]
                };
                RBlock::Details {
                    id: summary.id,
                    open,
                    summary,
                    body,
                }
            }
            HtmlBlockKind::Container(inner) => {
                let rich = self.rich_html(&inner);
                RBlock::Paragraph(vec![Segment::Text(rich)])
            }
            HtmlBlockKind::Source => RBlock::Html(content.trim_end().to_string()),
        }
    }

    fn inline(&mut self, inline: &Inline<'_>, acc: &mut RichBuilder) {
        match &inline.kind {
            InlineKind::Text(t) => acc.push(t),
            InlineKind::SoftBreak if self.opts.strict_line_breaks => acc.push(" "),
            InlineKind::SoftBreak | InlineKind::HardBreak => acc.push("\n"),
            InlineKind::Code(c) => acc.styled(|s| s.code = true, |a| a.push(c)),
            InlineKind::Html(h) => acc.html(h),
            InlineKind::Emphasis(c) => self.wrap(acc, c, |s| s.italic = true),
            InlineKind::Strong(c) => self.wrap(acc, c, |s| s.bold = true),
            InlineKind::Strikethrough(c) => self.wrap(acc, c, |s| s.strike = true),
            InlineKind::Highlight { color, children } => {
                let saved = acc.style;
                acc.style.highlight = true;
                acc.style.highlight_color = *color;
                for c in children {
                    self.inline(c, acc);
                }
                acc.style = saved;
            }
            InlineKind::Link {
                destination,
                kind,
                children,
                ..
            } => {
                let internal = matches!(kind, LinkKind::Inline | LinkKind::Reference)
                    && scoria_markdown::html::is_vault_destination(destination);
                let target = if internal {
                    LinkTarget::Internal(percent_decode(destination).into_owned())
                } else {
                    LinkTarget::External(destination.to_string())
                };
                let start = acc.len();
                if *kind == LinkKind::Autolink {
                    let shown = plain_text(children);
                    let shown = shown.strip_prefix("mailto:").unwrap_or(&shown).to_string();
                    acc.styled(|s| s.link = true, |a| a.push(&shown));
                    acc.link(start, target);
                } else {
                    // The same rule the HTML renderer follows: Obsidian ends a
                    // link at the first child that is a link of its own, and
                    // what comes after that child is not part of it. Without
                    // this the trailing text of `[md [[Links]] link](url)`
                    // would still be clickable through to the outer link.
                    let split = children
                        .iter()
                        .position(scoria_markdown::html::renders_as_anchor);
                    let head = split.unwrap_or(children.len());
                    self.wrap(acc, &children[..head], |s| s.link = true);
                    acc.link(start, target);
                    if let Some(at) = split {
                        for child in &children[at..] {
                            self.inline(child, acc);
                        }
                    }
                }
            }
            InlineKind::Image {
                destination,
                children,
                ..
            } => {
                // External or reference-style images: shown as a link until
                // remote images load.
                let start = acc.len();
                let alt = plain_text(children);
                let shown = if alt.is_empty() {
                    destination.to_string()
                } else {
                    alt
                };
                acc.styled(|s| s.link = true, |a| a.push(&shown));
                acc.link(start, LinkTarget::External(destination.to_string()));
            }
            // An embed reaches this walk only where a block-level embed has
            // no room -- a heading, a table cell, a callout title -- since
            // `segments()` takes the paragraph case first. Obsidian draws
            // the embedded content inline there; until `RichText` can carry
            // an inline embed, the link to it is the readable, clickable
            // stand-in, never the raw `![[...]]`.
            InlineKind::Wikilink { body, embed } => {
                let link = scoria_markdown::parse_linktext(body);
                let target = scoria_markdown::link_target(body).to_string();
                let shown = match link.alias {
                    // `![[pic.png|200]]`: the size hint is not display text.
                    Some(a) if *embed => {
                        alias_parts(a).0.unwrap_or_else(|| wikilink_display(&link))
                    }
                    Some(a) => a.to_string(),
                    None => wikilink_display(&link),
                };
                let start = acc.len();
                acc.styled(|s| s.link = true, |a| a.push(&shown));
                acc.link(start, LinkTarget::Internal(target));
            }
            InlineKind::Tag(name) => {
                let start = acc.len();
                acc.styled(
                    |s| s.tag = true,
                    |a| {
                        a.push("#");
                        a.push(name);
                    },
                );
                acc.link(start, LinkTarget::Tag(name.to_string()));
            }
            InlineKind::Math { content, .. } => {
                acc.styled(|s| s.math = true, |a| a.push(content));
            }
            InlineKind::Comment(_) => {}
            InlineKind::FootnoteReference(label) => {
                if self
                    .footnotes
                    .definitions
                    .iter()
                    .any(|l| l == label.as_ref())
                {
                    let n = self.footnote_number(label);
                    self.footnote_ref(acc, n);
                } else {
                    acc.push(label);
                }
            }
            InlineKind::InlineFootnote(children) => {
                let label = format!("inline-{}", self.footnotes.inline.len() + 1);
                let body = self.rich(children);
                self.footnotes.inline.push((label.clone(), body));
                let n = self.footnote_number(&label);
                self.footnote_ref(acc, n);
            }
        }
    }

    fn wrap(&mut self, acc: &mut RichBuilder, children: &[Inline<'_>], f: fn(&mut InlineStyle)) {
        let saved = acc.style;
        f(&mut acc.style);
        for c in children {
            self.inline(c, acc);
        }
        acc.style = saved;
    }

    fn footnote_number(&mut self, label: &str) -> usize {
        if let Some(i) = self.footnotes.numbers.iter().position(|l| l == label) {
            return i + 1;
        }
        self.footnotes.numbers.push(label.to_string());
        self.footnotes.numbers.len()
    }

    fn footnote_ref(&mut self, acc: &mut RichBuilder, n: usize) {
        let start = acc.len();
        acc.styled(|s| s.footnote = true, |a| a.push(&format!("[{n}]")));
        acc.link(start, LinkTarget::Footnote(n));
    }

    /// The Footnotes section, and where in the note each definition is
    /// written. A click on a reference reveals that offset, which is the
    /// definition's line in an editing mode and the Footnotes block in
    /// reading.
    fn footnote_section(&mut self, doc: &Document<'_>) -> Option<(RBlock, Vec<usize>)> {
        if self.footnotes.numbers.is_empty() {
            return None;
        }
        let labels = self.footnotes.numbers.clone();
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        for (i, label) in labels.iter().enumerate() {
            let n = i + 1;
            let mut at = None;
            let body =
                if let Some((_, text)) = self.footnotes.inline.iter().find(|(l, _)| l == label) {
                    vec![RBlock::Paragraph(vec![Segment::Text(text.clone())])]
                } else {
                    let mut found = None;
                    scoria_markdown::walk_blocks(&doc.blocks, &mut |b| {
                        if found.is_none() {
                            if let BlockKind::FootnoteDefinition { label: l, children } = &b.kind {
                                if l == label {
                                    found = Some(children);
                                    at = Some(b.span.start);
                                }
                            }
                        }
                    });
                    match found {
                        Some(children) => self.blocks(children, false),
                        None => Vec::new(),
                    }
                };
            // An inline footnote is written where it is referenced, and a
            // label nothing defines has nowhere else to point either.
            offsets.push(at.unwrap_or(0));
            out.push((n, body));
        }
        Some((RBlock::Footnotes(out), offsets))
    }

    // ----- embeds --------------------------------------------------------------

    fn wikilink_embed(&mut self, body: &str) -> Embed {
        let link = scoria_markdown::parse_linktext(body);
        let target = scoria_markdown::link_target(body);
        let (alt, width, height) = alias_parts(link.alias.unwrap_or(""));
        let alt = alt.unwrap_or_else(|| wikilink_display(&link));
        self.embed(target, alt, width, height)
    }

    fn embed(&mut self, link: &str, alt: String, width: Option<f32>, height: Option<f32>) -> Embed {
        let parsed = scoria_markdown::parse_linktext(link);
        let resolved = self.vault.resolve(link, self.source_path);
        let kind = match resolved {
            None => EmbedKind::Missing,
            Some(path) => match FileKind::from_path(&path) {
                FileKind::Image => EmbedKind::Image(self.vault.absolute(&path)),
                FileKind::Markdown => {
                    if self.depth + 1 >= MAX_EMBED_DEPTH {
                        EmbedKind::TooDeep
                    } else {
                        self.note_embed(&path, parsed.subpath.as_ref(), &wikilink_display(&parsed))
                    }
                }
                // A base nests no further: its rows are files, and a note
                // inside one is a row rather than an embed, so the depth
                // guard above has nothing to count here — and a note that
                // embeds a base listing that same note still terminates,
                // because a row is cells rather than another model. The
                // subpath is the view to show, `![[x.base#Cards]]`, which the
                // link parser reads as a heading.
                FileKind::Base => {
                    let view = match parsed.subpath.as_ref() {
                        Some(scoria_markdown::Subpath::Headings(names)) => {
                            names.first().map(|n| n.to_string())
                        }
                        _ => None,
                    };
                    match self.bases.resolve(&path, view.as_deref(), self.vault) {
                        Some(base) => EmbedKind::Base(base),
                        None => EmbedKind::Missing,
                    }
                }
                // A PDF is resolved and nothing more: the file is parsed by
                // the document cache when the embed is drawn, off the UI
                // thread. The subpath (`#page=3&height=400`) is the link's
                // own, not a heading, so it is read from the raw link text.
                FileKind::Pdf => {
                    let id = self.next_id;
                    self.next_id += 1;
                    EmbedKind::Pdf(crate::pdf::embed::PdfEmbed::new(
                        id,
                        path.clone(),
                        self.vault.absolute(&path),
                        link,
                        height,
                    ))
                }
                other => EmbedKind::Unsupported(other),
            },
        };
        Embed {
            kind,
            link: link.to_string(),
            alt,
            width,
            height,
        }
    }

    fn note_embed(&mut self, path: &str, subpath: Option<&Subpath<'_>>, title: &str) -> EmbedKind {
        let Some(text) = self.vault.read(path) else {
            return EmbedKind::Missing;
        };
        let doc = parse(&text);
        let selected: Vec<Block<'_>> = match subpath {
            None => doc
                .blocks
                .iter()
                .filter(|b| !matches!(b.kind, BlockKind::Yaml { .. }))
                .cloned()
                .collect(),
            Some(Subpath::Headings(names)) => match section(&doc.blocks, names) {
                Some(s) => s.to_vec(),
                None => return EmbedKind::Missing,
            },
            Some(Subpath::Block(id)) => match block_by_id(&doc.blocks, id) {
                Some(b) => vec![b.clone()],
                None => return EmbedKind::Missing,
            },
        };
        let mut next_id = self.next_id;
        let blocks = build_embedded(
            path,
            &doc,
            &selected,
            self.depth + 1,
            &mut next_id,
            self.bases,
            self.vault,
            self.opts,
        );
        self.next_id = next_id;
        let title = if title.is_empty() {
            path.trim_end_matches(".md").to_string()
        } else {
            title.to_string()
        };
        EmbedKind::Note {
            path: path.to_string(),
            title,
            blocks,
        }
    }
}

/// Accumulates text with a style stack into one [`RichText`].
#[derive(Default)]
struct RichBuilder {
    text: String,
    /// The style the Markdown nesting gives the next text; `wrap` and
    /// `styled` save and restore it around their children.
    style: InlineStyle,
    /// The raw HTML tags open at this point, innermost last. Kept apart from
    /// `style` because a tag's extent is not a Markdown node's: `*a <b>b*
    /// c</b>` closes the emphasis before the bold, and the overlay keeps
    /// each from restoring over the other.
    html: Vec<HtmlOpen>,
    runs: Vec<(Range<usize>, InlineStyle)>,
    links: Vec<(Range<usize>, LinkTarget)>,
}

/// One open raw HTML tag.
struct HtmlOpen {
    name: HtmlTag,
    /// For `<a href>`: where its text started and what it points at,
    /// recorded as a link when the tag closes.
    link: Option<(usize, LinkTarget)>,
}

/// The inline HTML tags the reading model maps onto its own styles: the
/// part of the raw-HTML subset the spec records Obsidian keeping that a
/// flat `RichText` can carry. Anything else stays muted source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HtmlTag {
    Bold,
    Italic,
    Strike,
    Highlight,
    Underline,
    Kbd,
    Sup,
    Sub,
    Anchor,
    /// `<span>`, `<div>`, `<font>`, ...: dropped, content kept.
    Transparent,
}

impl HtmlTag {
    fn from_name(name: &str) -> Option<HtmlTag> {
        Some(match name.to_ascii_lowercase().as_str() {
            "b" | "strong" => HtmlTag::Bold,
            "i" | "em" => HtmlTag::Italic,
            "s" | "del" | "strike" => HtmlTag::Strike,
            "mark" => HtmlTag::Highlight,
            "u" => HtmlTag::Underline,
            "kbd" => HtmlTag::Kbd,
            "sup" => HtmlTag::Sup,
            "sub" => HtmlTag::Sub,
            "a" => HtmlTag::Anchor,
            "span" | "div" | "font" | "center" | "small" | "big" | "p" | "section" | "article"
            | "ins" | "samp" | "var" | "tt" => HtmlTag::Transparent,
            _ => return None,
        })
    }

    fn apply(self, s: &mut InlineStyle) {
        match self {
            HtmlTag::Bold => s.bold = true,
            HtmlTag::Italic => s.italic = true,
            HtmlTag::Strike => s.strike = true,
            HtmlTag::Highlight => s.highlight = true,
            HtmlTag::Underline => s.underline = true,
            HtmlTag::Kbd => s.kbd = true,
            HtmlTag::Sup => s.sup = true,
            HtmlTag::Sub => s.sub = true,
            // The link style belongs to an anchor with somewhere to go, so
            // `open_tag` sets it on the entry instead.
            HtmlTag::Anchor | HtmlTag::Transparent => {}
        }
    }
}

/// One inline HTML token as the parser hands it over: a whole tag, or a
/// comment, declaration or CDATA section.
enum HtmlToken<'a> {
    Open {
        name: &'a str,
        attrs: &'a str,
    },
    Close(&'a str),
    /// `<!-- -->`, `<!DOCTYPE>`, `<![CDATA[ ]]>`, `<?php ?>`: nothing shown.
    Comment,
}

fn html_token(raw: &str) -> Option<HtmlToken<'_>> {
    let inner = raw.strip_prefix('<')?.strip_suffix('>')?;
    if inner.starts_with(['!', '?']) {
        return Some(HtmlToken::Comment);
    }
    let (closing, rest) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name_len = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric())
        .count();
    if name_len == 0 {
        return None;
    }
    let (name, attrs) = rest.split_at(name_len);
    Some(if closing {
        HtmlToken::Close(name)
    } else {
        HtmlToken::Open {
            name,
            attrs: attrs.trim().trim_end_matches('/'),
        }
    })
}

impl RichBuilder {
    fn len(&self) -> usize {
        self.text.len()
    }

    /// The style the next text takes: the Markdown style with every open
    /// HTML tag laid over it.
    fn effective(&self) -> InlineStyle {
        let mut s = self.style;
        for open in &self.html {
            open.name.apply(&mut s);
            if open.link.is_some() {
                s.link = true;
            }
        }
        s
    }

    fn push(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(s);
        let end = self.text.len();
        let style = self.effective();
        if style.is_plain() {
            return;
        }
        if let Some((range, last)) = self.runs.last_mut() {
            if *last == style && range.end == start {
                range.end = end;
                return;
            }
        }
        self.runs.push((start..end, style));
    }

    /// One inline HTML token. The tags the model understands become style,
    /// a break or a link; a comment vanishes; a closer with nothing open to
    /// close is ignored; and a tag outside the subset keeps its source,
    /// muted, which is what every raw tag was before.
    fn html(&mut self, raw: &str) {
        match html_token(raw) {
            Some(HtmlToken::Comment) => {}
            Some(HtmlToken::Open { name, .. }) if name.eq_ignore_ascii_case("br") => {
                self.push("\n")
            }
            Some(HtmlToken::Open { name, attrs }) => match HtmlTag::from_name(name) {
                Some(tag) => self.open_tag(tag, attrs),
                None => self.styled(|s| s.html = true, |a| a.push(raw)),
            },
            Some(HtmlToken::Close(name)) => match HtmlTag::from_name(name) {
                Some(tag) => self.close_tag(tag),
                None => self.styled(|s| s.html = true, |a| a.push(raw)),
            },
            None => self.styled(|s| s.html = true, |a| a.push(raw)),
        }
    }

    fn open_tag(&mut self, name: HtmlTag, attrs: &str) {
        let mut link = None;
        if name == HtmlTag::Anchor {
            // Obsidian keeps a raw anchor's href as written -- no vault
            // resolution, so even `href="Note.md"` is external -- and drops
            // a `javascript:` one altogether, leaving plain text.
            let href = html_attrs(attrs)
                .into_iter()
                .flatten()
                .find(|(n, _)| n == "href")
                .map(|(_, v)| v)
                .filter(|v| {
                    !v.is_empty()
                        && !v
                            .trim_start()
                            .to_ascii_lowercase()
                            .starts_with("javascript:")
                });
            link = href.map(|href| (self.text.len(), LinkTarget::External(href)));
        }
        self.html.push(HtmlOpen { name, link });
    }

    /// Closes the innermost open tag of this name and whatever was opened
    /// inside it; a closer with no opener is dropped.
    fn close_tag(&mut self, name: HtmlTag) {
        let Some(at) = self.html.iter().rposition(|o| o.name == name) else {
            return;
        };
        let closed: Vec<HtmlOpen> = self.html.drain(at..).collect();
        for open in closed {
            if let Some((start, target)) = open.link {
                self.link(start, target);
            }
        }
    }

    /// The end of the text closes every tag still open, the way Obsidian
    /// closes `start <b>bold never closed` at the end of its paragraph.
    fn close_all(&mut self) {
        let open: Vec<HtmlOpen> = std::mem::take(&mut self.html);
        for open in open {
            if let Some((start, target)) = open.link {
                self.link(start, target);
            }
        }
    }

    fn styled(&mut self, f: impl FnOnce(&mut InlineStyle), body: impl FnOnce(&mut RichBuilder)) {
        let saved = self.style;
        f(&mut self.style);
        body(self);
        self.style = saved;
    }

    fn link(&mut self, start: usize, target: LinkTarget) {
        let end = self.text.len();
        if end > start {
            self.links.push((start..end, target));
        }
    }

    fn finish(mut self, id: usize) -> RichText {
        self.close_all();
        RichText {
            id,
            text: self.text,
            runs: self.runs,
            links: self.links,
        }
    }
}

/// The `|`-separated parts of an embed's alias, which is the alias of
/// `![[pic.png|alt|200x100]]` and the alt of `![alt|200x100](pic.png)`
/// alike: every part that reads as `W` or `WxH` claims the size (the last
/// one wins), and the parts that do not stay the alt, `|`-joined again.
/// Both spellings go through here so they cannot disagree.
fn alias_parts(alias: &str) -> (Option<String>, Option<f32>, Option<f32>) {
    let mut width = None;
    let mut height = None;
    let mut alt: Vec<&str> = Vec::new();
    for part in alias.split('|').map(str::trim) {
        match size_hint(part) {
            Some((w, h)) => {
                width = Some(w);
                height = h;
            }
            None if !part.is_empty() => alt.push(part),
            None => {}
        }
    }
    let alt = (!alt.is_empty()).then(|| alt.join("|"));
    (alt, width, height)
}

/// `200` or `200x100` → `(width, Some(height))`.
fn size_hint(s: &str) -> Option<(f32, Option<f32>)> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit() || b == b'x') {
        return None;
    }
    let mut parts = s.split('x');
    let w: f32 = parts.next()?.parse().ok()?;
    let h = match parts.next() {
        Some(h) => Some(h.parse().ok()?),
        None => None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((w, h))
}

/// Whether an HTML block would paint nothing: comments and `<script>`
/// elements, and the whitespace between them, and nothing else. An
/// unterminated comment or script swallows the rest of the block, as a
/// parser reading to the end of input does.
/// What a raw HTML block draws as.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HtmlBlockKind {
    /// `<pre>…</pre>`: the literal text and whether a `<code>` child earns
    /// the copy button.
    Pre { text: String, code: bool },
    /// `<details [open]>`: the summary's HTML, the rest's HTML.
    Details {
        open: bool,
        summary: String,
        body: String,
    },
    /// A container the sanitizer keeps with its content: the inner HTML.
    Container(String),
    /// Anything else, drawn as source.
    Source,
}

/// Whether a raw HTML block renders as something other than its source,
/// which is what earns it a Live Preview widget.
fn html_block_renders(content: &str) -> bool {
    classify_html_block(content) != HtmlBlockKind::Source
}

fn classify_html_block(content: &str) -> HtmlBlockKind {
    // A comment before the first tag says nothing about the block: the
    // inline overlay drops it on the way through.
    let mut trimmed = content.trim();
    while let Some(after) = trimmed.strip_prefix("<!--") {
        trimmed = match after.find("-->") {
            Some(end) => after[end + 3..].trim_start(),
            None => return HtmlBlockKind::Source,
        };
    }
    let Some(end) = trimmed.find('>') else {
        return HtmlBlockKind::Source;
    };
    let Some(HtmlToken::Open { name, attrs }) = html_token(&trimmed[..=end]) else {
        return HtmlBlockKind::Source;
    };
    let name = name.to_ascii_lowercase();
    let after = &trimmed[end + 1..];
    let inner = |closing: &str| -> &str {
        match find_ignore_ascii_case(after, closing) {
            Some(at) => &after[..at],
            None => after,
        }
    };
    match name.as_str() {
        "pre" => {
            let mut body = inner("</pre").trim_matches('\n').to_string();
            let mut code = false;
            if let Some(stripped) = strip_tag(&body, "code") {
                body = stripped;
                code = true;
            }
            HtmlBlockKind::Pre {
                text: decode_entities(&strip_tags(&body))
                    .trim_end_matches('\n')
                    .to_string(),
                code,
            }
        }
        "details" => {
            let open = attrs.split_whitespace().any(|a| {
                a.eq_ignore_ascii_case("open") || a.to_ascii_lowercase().starts_with("open=")
            });
            let inner = inner("</details").trim();
            let (summary, body) = match find_ignore_ascii_case(inner, "<summary") {
                Some(at) => {
                    let after_tag = inner[at..]
                        .find('>')
                        .map(|i| at + i + 1)
                        .unwrap_or(inner.len());
                    let rest = &inner[after_tag..];
                    match find_ignore_ascii_case(rest, "</summary") {
                        Some(close) => {
                            let tail = rest[close..]
                                .find('>')
                                .map(|i| close + i + 1)
                                .unwrap_or(rest.len());
                            (rest[..close].to_string(), rest[tail..].to_string())
                        }
                        None => (rest.to_string(), String::new()),
                    }
                }
                None => (String::from("Details"), inner.to_string()),
            };
            HtmlBlockKind::Details {
                open,
                summary,
                body,
            }
        }
        "div" | "p" | "center" | "span" | "font" | "small" | "big" | "section" | "article"
        | "ins" | "samp" | "var" | "tt" | "kbd" | "sup" | "sub" | "b" | "strong" | "i" | "em"
        | "u" | "mark" | "s" | "del" | "strike" | "a" => {
            HtmlBlockKind::Container(trimmed.to_string())
        }
        _ => HtmlBlockKind::Source,
    }
}

/// `body` with one outer `<tag …>…</tag>` pair removed, when it starts with
/// the opener.
fn strip_tag(body: &str, tag: &str) -> Option<String> {
    let trimmed = body.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with(&format!("<{tag}")) {
        return None;
    }
    let open_end = trimmed.find('>')? + 1;
    let inner = &trimmed[open_end..];
    let close = format!("</{tag}");
    let cut = match find_ignore_ascii_case(inner, &close) {
        Some(at) => &inner[..at],
        None => inner,
    };
    Some(cut.trim_start_matches('\n').to_string())
}

/// Every tag removed, its content kept: what a `<pre>` shows for the live
/// markup inside it, as text.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        match rest[at..].find('>') {
            Some(end) => rest = &rest[at + end + 1..],
            None => {
                out.push_str(&rest[at..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Text between tags: the entities a hand-written note uses decoded, and
/// the newlines HTML reads as spaces read as spaces.
fn html_text(raw: &str) -> String {
    let decoded = decode_entities(raw);
    // HTML collapses a run of whitespace, newlines included, to one space,
    // and keeps the one at a chunk's edge: `The ` before `<i>` stays `The `.
    let mut out = String::with_capacity(decoded.len());
    let mut in_space = false;
    for ch in decoded.chars() {
        if ch.is_whitespace() && ch != '\u{a0}' {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

/// The entities a hand-written note uses, decoded; everything else as it
/// was, newlines included (a `<pre>` keeps them).
fn decode_entities(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let entity = tail.find(';').map(|i| &tail[..=i]);
        let decoded = match entity {
            Some("&amp;") => Some('&'),
            Some("&lt;") => Some('<'),
            Some("&gt;") => Some('>'),
            Some("&quot;") => Some('"'),
            Some("&apos;") | Some("&#39;") => Some('\''),
            Some("&nbsp;") => Some('\u{a0}'),
            Some(e) if e.starts_with("&#") => {
                let num = &e[2..e.len() - 1];
                let code = match num.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => num.parse::<u32>().ok(),
                };
                code.and_then(char::from_u32)
            }
            _ => None,
        };
        match (decoded, entity) {
            (Some(ch), Some(e)) => {
                out.push(ch);
                rest = &tail[e.len()..];
            }
            _ => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn html_block_is_invisible(content: &str) -> bool {
    let mut rest = content.trim();
    let mut seen = false;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = match after.find("-->") {
                Some(end) => &after[end + 3..],
                // An unclosed comment runs to the end of the block.
                None => "",
            };
        } else if let Some(after) = script_open(rest) {
            rest = match find_ignore_ascii_case(after, "</script") {
                // Past the closing tag's own `>`, which may carry
                // whitespace before it (`</script >`).
                Some(end) => match after[end..].find('>') {
                    Some(gt) => &after[end + gt + 1..],
                    None => "",
                },
                None => "",
            };
        } else {
            return false;
        }
        seen = true;
        rest = rest.trim_start();
    }
    seen
}

/// The text after `<script ...>` when `rest` opens a script element.
fn script_open(rest: &str) -> Option<&str> {
    let after = rest
        .get(..7)
        .filter(|t| t.eq_ignore_ascii_case("<script"))?;
    let after = &rest[after.len()..];
    if !after.starts_with([' ', '\t', '\n', '\r', '/', '>']) {
        return None;
    }
    // Past the open tag, whose attributes may hold a `>` only inside
    // quotes; `split_tag` is the one place that rule lives.
    split_tag(after).map(|(_, tail)| tail)
}

/// `haystack.find(needle)` ignoring ASCII case, for a lower-case `needle`.
fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// A raw `<img>` of an HTML block, with the attributes Obsidian honours.
struct HtmlImage {
    src: String,
    alt: String,
    width: Option<f32>,
    height: Option<f32>,
}

/// The images of an HTML block that holds nothing but `<img>` tags -- the
/// shape Obsidian renders as one `div.el-img` rather than as source. A
/// block with any other content, or an image pointing outside the vault,
/// gives `None` and keeps its raw text.
fn html_images(content: &str) -> Option<Vec<HtmlImage>> {
    let mut out = Vec::new();
    let mut rest = content.trim();
    while !rest.is_empty() {
        let after = match rest.get(..4) {
            // Only `<img` itself, not `<image>` or some longer tag name.
            Some(tag) if tag.eq_ignore_ascii_case("<img") => &rest[4..],
            _ => return None,
        };
        if !after.starts_with([' ', '\t', '\n', '\r', '/', '>']) {
            return None;
        }
        let (attrs, tail) = split_tag(after)?;
        out.push(html_image(attrs)?);
        rest = tail.trim_start();
    }
    (!out.is_empty()).then_some(out)
}

/// Splits a tag's attributes from the rest of the input at the `>` that
/// closes it, skipping any inside a quoted value.
fn split_tag(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    let mut quote = None;
    for (i, &c) in b.iter().enumerate() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == b'"' || c == b'\'' => quote = Some(c),
            None if c == b'>' => return Some((&s[..i], &s[i + 1..])),
            None => {}
        }
    }
    None
}

/// One `<img>`, from the text between `<img` and its `>`.
fn html_image(attrs: &str) -> Option<HtmlImage> {
    let mut src = None;
    let mut alt = String::new();
    let mut width = None;
    let mut height = None;
    for (name, value) in html_attrs(attrs.trim_end().trim_end_matches('/'))? {
        match name.as_str() {
            "src" => src = Some(percent_decode(&value).into_owned()),
            "alt" => alt = value,
            // A raw dimension is CSS pixels; a `%` or a `calc()` is not
            // something the embed can carry, so the block stays source.
            "width" => width = Some(value.parse().ok()?),
            "height" => height = Some(value.parse().ok()?),
            _ => {}
        }
    }
    let src = src?;
    // A remote image is fetched by Obsidian and shown as a link here, the
    // way a markdown image with a scheme already is.
    if src.is_empty() || has_scheme(&src) {
        return None;
    }
    Some(HtmlImage {
        src,
        alt,
        width,
        height,
    })
}

/// A tag's attributes as lowercase name and raw value pairs. A malformed
/// run (an unterminated quote, a value with no name) gives `None`.
fn html_attrs(s: &str) -> Option<Vec<(String, String)>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' {
            i += 1;
        }
        if i == start {
            return None;
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if b.get(i) == Some(&b'=') {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            match b.get(i) {
                Some(&q) if q == b'"' || q == b'\'' => {
                    i += 1;
                    let from = i;
                    while i < b.len() && b[i] != q {
                        i += 1;
                    }
                    if i >= b.len() {
                        return None;
                    }
                    value = s[from..i].to_string();
                    i += 1;
                }
                _ => {
                    let from = i;
                    while i < b.len() && !b[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    value = s[from..i].to_string();
                }
            }
        }
        out.push((name, value));
    }
    Some(out)
}

/// Display text of a wikilink without an alias: `Note`, `Note > Heading`, `Note > ^id`.
fn wikilink_display(link: &scoria_markdown::LinkText<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !link.path.is_empty() {
        parts.push(link.path.to_string());
    }
    match &link.subpath {
        Some(Subpath::Headings(hs)) => parts.extend(hs.iter().map(|h| h.to_string())),
        Some(Subpath::Block(b)) => parts.push(format!("^{b}")),
        None => {}
    }
    parts.join(" > ")
}

fn heading_matches(block: &Block<'_>, name: &str) -> Option<u8> {
    if let BlockKind::Heading { level, content, .. } = &block.kind {
        if plain_text(content).trim().eq_ignore_ascii_case(name.trim()) {
            return Some(*level);
        }
    }
    None
}

/// The blocks under a heading path (`#A#B`): from the matched heading to
/// the next heading of the same or a higher level.
fn section<'b, 'a>(blocks: &'b [Block<'a>], names: &[&str]) -> Option<&'b [Block<'a>]> {
    let mut range: &[Block<'a>] = blocks;
    for name in names {
        let start = range
            .iter()
            .position(|b| heading_matches(b, name).is_some())?;
        let level = heading_matches(&range[start], name)?;
        let end = range[start + 1..]
            .iter()
            .position(|b| matches!(&b.kind, BlockKind::Heading { level: l, .. } if *l <= level))
            .map(|i| start + 1 + i)
            .unwrap_or(range.len());
        range = &range[start..end];
    }
    Some(range)
}

fn block_by_id<'b, 'a>(blocks: &'b [Block<'a>], id: &str) -> Option<&'b Block<'a>> {
    let mut found = None;
    scoria_markdown::walk_blocks(blocks, &mut |b| {
        if found.is_none() && b.id.is_some_and(|i| i.id.eq_ignore_ascii_case(id)) {
            found = Some(b);
        }
    });
    if found.is_some() {
        return found;
    }
    // A list item id lives on the item's first paragraph in the cache but
    // the block that carries it in the tree is the paragraph itself, which
    // `walk_blocks` visits; nothing more to do here.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_formats::properties::Value as PropertyValue;
    use std::collections::HashMap;

    struct Stub {
        files: HashMap<&'static str, &'static str>,
        /// How many times [`EmbedSource::base`] was asked, so a test can see
        /// that one model resolves a base once however often it is embedded.
        base_calls: std::cell::Cell<usize>,
    }

    impl EmbedSource for Stub {
        fn resolve(&self, linktext: &str, _source: &str) -> Option<String> {
            let target = scoria_markdown::parse_linktext(linktext).path;
            let target = if target.is_empty() { "Self" } else { target };
            self.files
                .keys()
                .find(|k| {
                    k.trim_end_matches(".md")
                        .eq_ignore_ascii_case(target.trim_end_matches(".md"))
                })
                .map(|k| k.to_string())
        }
        fn read(&self, path: &str) -> Option<String> {
            self.files.get(path).map(|s| s.to_string())
        }
        fn absolute(&self, path: &str) -> PathBuf {
            PathBuf::from("/vault").join(path)
        }
        /// Whether the stub has the base. `Broken.base` is a file the
        /// link resolver knows and the read still refuses, which is the
        /// other way an embed comes out `Missing`.
        fn has_base(&self, path: &str) -> bool {
            self.base_calls.set(self.base_calls.get() + 1);
            path != "Broken.base" && self.files.contains_key(path)
        }
    }

    fn stub() -> Stub {
        Stub {
            files: HashMap::from([
                ("Self.md", "root"),
                (
                    "Other.md",
                    "---\ntitle: x\n---\n# A\nfirst\n## B\nsecond ^blk\n# C\nthird",
                ),
                ("Loop.md", "![[Loop]]"),
                ("pic.png", ""),
                ("Field.pdf", ""),
                ("Main.base", ""),
                ("Broken.base", ""),
                ("Nester.md", "![[Main.base]]"),
            ]),
            base_calls: std::cell::Cell::new(0),
        }
    }

    fn first_embed(block: &RBlock) -> &Embed {
        match block {
            RBlock::Paragraph(segs) => match &segs[0] {
                Segment::Embed(embed) => embed,
                other => panic!("not an embed: {other:?}"),
            },
            other => panic!("not a paragraph: {other:?}"),
        }
    }

    fn para_text(block: &RBlock) -> &RichText {
        match block {
            RBlock::Paragraph(segs) => match &segs[0] {
                Segment::Text(t) => t,
                other => panic!("not text: {other:?}"),
            },
            other => panic!("not a paragraph: {other:?}"),
        }
    }

    #[test]
    fn runs_merge_and_links_cover_their_text() {
        let m = build(
            "Self.md",
            "a **b *c*** [[Note|n]] #tag `x` https://e.com",
            &stub(),
        );
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "a b c n #tag x https://e.com");
        let bold = InlineStyle {
            bold: true,
            ..InlineStyle::default()
        };
        let bold_italic = InlineStyle {
            bold: true,
            italic: true,
            ..InlineStyle::default()
        };
        assert_eq!(t.runs[0], (2..4, bold));
        assert_eq!(t.runs[1], (4..5, bold_italic));
        assert!(t
            .runs
            .iter()
            .any(|(r, s)| s.link && &t.text[r.clone()] == "n"));
        assert!(t
            .runs
            .iter()
            .any(|(r, s)| s.code && &t.text[r.clone()] == "x"));
        assert_eq!(
            t.links,
            vec![
                (6..7, LinkTarget::Internal("Note".into())),
                (8..12, LinkTarget::Tag("tag".into())),
                (15..28, LinkTarget::External("https://e.com".into())),
            ]
        );
    }

    #[test]
    fn soft_breaks_become_newlines_and_comments_vanish() {
        let m = build("Self.md", "one\ntwo %%hidden%% three", &stub());
        assert_eq!(para_text(&m.blocks[0]).text, "one\ntwo  three");
        let strict = BuildOptions {
            strict_line_breaks: true,
        };
        let m = build_with("Self.md", "one\ntwo  \nthree", &stub(), strict);
        assert_eq!(para_text(&m.blocks[0]).text, "one two\nthree");
    }

    #[test]
    fn embeds_split_paragraphs_and_resolve_recursively() {
        let m = build(
            "Self.md",
            "before ![[pic.png|200x100]] ![[Other#B]] ![[Other#^blk]] ![[Nope]] after",
            &stub(),
        );
        let RBlock::Paragraph(segs) = &m.blocks[0] else {
            panic!()
        };
        assert_eq!(segs.len(), 6);
        match &segs[1] {
            Segment::Embed(Embed {
                kind: EmbedKind::Image(p),
                width,
                height,
                ..
            }) => {
                assert_eq!(p, &PathBuf::from("/vault/pic.png"));
                assert_eq!((*width, *height), (Some(200.0), Some(100.0)));
            }
            other => panic!("{other:?}"),
        }
        match &segs[2] {
            Segment::Embed(Embed {
                kind: EmbedKind::Note { title, blocks, .. },
                ..
            }) => {
                assert_eq!(title, "Other > B");
                assert!(matches!(&blocks[0], RBlock::Heading { plain, .. } if plain == "B"));
                assert_eq!(para_text(&blocks[1]).text, "second");
                assert_eq!(blocks.len(), 2, "section stops before # C");
            }
            other => panic!("{other:?}"),
        }
        match &segs[3] {
            Segment::Embed(Embed {
                kind: EmbedKind::Note { blocks, .. },
                ..
            }) => assert_eq!(para_text(&blocks[0]).text, "second"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &segs[4],
            Segment::Embed(Embed {
                kind: EmbedKind::Missing,
                ..
            })
        ));
        assert_eq!(para_text(&segs[5].as_text()).text, " after");
    }

    impl Segment {
        fn as_text(&self) -> RBlock {
            match self {
                Segment::Text(t) => RBlock::Paragraph(vec![Segment::Text(t.clone())]),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn recursive_embeds_stop_at_the_depth_limit() {
        let m = build("Loop.md", "![[Loop]]", &stub());
        let mut depth = 0;
        let mut blocks = &m.blocks;
        loop {
            let RBlock::Paragraph(segs) = &blocks[0] else {
                panic!()
            };
            match &segs[0] {
                Segment::Embed(Embed {
                    kind: EmbedKind::Note { blocks: inner, .. },
                    ..
                }) => {
                    depth += 1;
                    blocks = inner;
                }
                Segment::Embed(Embed {
                    kind: EmbedKind::TooDeep,
                    ..
                }) => break,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(depth, MAX_EMBED_DEPTH - 1);
    }

    #[test]
    fn an_embedded_base_is_its_own_kind_wherever_it_is_written() {
        let stub = stub();
        // The same base twice, and once more inside an embedded note.
        let m = build(
            "Self.md",
            "![[Main.base]]\n\n![[Nester]]\n\n![[Main.base]]",
            &stub,
        );
        // The same base three times is three embeds, counted in document
        // order -- the one inside the embedded note is the second.
        for (ix, nth) in [(0, 0), (2, 2)] {
            match &first_embed(&m.blocks[ix]).kind {
                EmbedKind::Base(base) => {
                    assert_eq!(base.path, "Main.base");
                    assert_eq!(base.view, None);
                    assert_eq!(base.nth, nth);
                }
                other => panic!("{other:?}"),
            }
        }
        // A base inside an embedded note is a base there too: the note's own
        // depth bounds the nesting, and the base below it adds none.
        let EmbedKind::Note { blocks, .. } = &first_embed(&m.blocks[1]).kind else {
            panic!("the middle embed is a note")
        };
        match &first_embed(&blocks[0]).kind {
            EmbedKind::Base(base) => assert_eq!(base.nth, 1),
            other => panic!("{other:?}"),
        }
        // And whether the file is there was asked exactly once.
        assert_eq!(stub.base_calls.get(), 1);
    }

    /// `![[x.base#Cards]]` names the view to show; the ordinal counts per
    /// view, so a plain embed and a named one of the same base are each
    /// the first of their kind.
    #[test]
    fn a_base_embeds_subpath_names_its_view() {
        let stub = stub();
        let m = build(
            "Self.md",
            "![[Main.base#Cards]]\n\n![[Main.base]]\n\n![[Main.base#cards|alias]]",
            &stub,
        );
        let views: Vec<(Option<String>, usize)> = (0..3)
            .map(|ix| match &first_embed(&m.blocks[ix]).kind {
                EmbedKind::Base(base) => (base.view.clone(), base.nth),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            views,
            [
                (Some("Cards".to_string()), 0),
                (None, 0),
                (Some("cards".to_string()), 1),
            ]
        );
    }

    #[test]
    fn a_pdf_embed_carries_its_subpath_and_reads_nothing() {
        use crate::pdf::embed::DEFAULT_HEIGHT;
        let m = build(
            "Self.md",
            "![[Field.pdf]]\n\n![[Field.pdf#page=2]]\n\n![[Field.pdf#height=200|300x150]]\n\n\
             ![](Field.pdf#page=3)\n\n![[Nope.pdf]]",
            &stub(),
        );
        let pdf = |ix: usize| match &first_embed(&m.blocks[ix]).kind {
            EmbedKind::Pdf(pdf) => pdf.clone(),
            other => panic!("{other:?}"),
        };
        // The stub never read `Field.pdf` (it has no text to read): the
        // model carries the path and the subpath and nothing else.
        let plain = pdf(0);
        assert_eq!(plain.path, "Field.pdf");
        assert_eq!(plain.absolute, PathBuf::from("/vault/Field.pdf"));
        assert_eq!((plain.page, plain.height), (None, None));
        assert_eq!(plain.box_height(), DEFAULT_HEIGHT);
        // `#page=2` is the link's own subpath, not a heading: zero-based.
        assert_eq!(pdf(1).page, Some(1));
        // `#height=` wins over the alias size; the alias is still the alt.
        let sized = pdf(2);
        assert_eq!(sized.height, Some(200.0));
        assert_eq!(first_embed(&m.blocks[2]).width, Some(300.0));
        // The markdown image form reads the same subpath.
        assert_eq!(pdf(3).page, Some(2));
        // Every embed has an id of its own, so two of one file scroll apart.
        assert_ne!(pdf(0).id, pdf(1).id);
        assert!(matches!(first_embed(&m.blocks[4]).kind, EmbedKind::Missing));
    }

    #[test]
    fn each_embed_walks_into_embedded_notes() {
        let m = build("Self.md", "![[Nester]]\n\n> ![[Field.pdf]]", &stub());
        let mut seen = Vec::new();
        each_embed(&m.blocks, &mut |embed| seen.push(embed.link.clone()));
        assert_eq!(seen, vec!["Nester", "Main.base", "Field.pdf"]);
    }

    #[test]
    fn a_base_nothing_answers_for_is_missing_rather_than_empty() {
        let stub = stub();
        let m = build("Self.md", "![[Nope.base]]\n\n![[Broken.base]]", &stub);
        // A link that resolves to no file at all, and a file that exists and
        // still answers no base: both are the embed's `Missing`.
        assert!(matches!(first_embed(&m.blocks[0]).kind, EmbedKind::Missing));
        assert!(matches!(first_embed(&m.blocks[1]).kind, EmbedKind::Missing));
        // The miss is remembered too, so a second mention asks nothing again.
        let m = build("Self.md", "![[Broken.base]]\n\n![[Broken.base]]", &stub);
        assert!(matches!(first_embed(&m.blocks[1]).kind, EmbedKind::Missing));
        assert_eq!(stub.base_calls.get(), 2, "once per model, not per embed");
    }

    /// `blocks` drops a comment block; `starts` has to drop exactly the
    /// same one, or every jump past it lands a block out.
    #[test]
    fn a_block_comment_shifts_neither_list() {
        let plain = build("Self.md", "# One\n\nbody\n\n# Two\n", &stub());
        assert_eq!(plain.blocks.len(), plain.starts.len());
        let two = plain.starts[2];
        assert!(matches!(&plain.blocks[2], RBlock::Heading { plain, .. } if plain == "Two"));

        // The same note behind a block comment: one parsed block more, the
        // same built blocks, and "Two" still the third of them.
        let text = "%%\na block comment\n%%\n\n# One\n\nbody\n\n# Two\n";
        let with_comment = build("Self.md", text, &stub());
        assert_eq!(
            with_comment.blocks.len(),
            with_comment.starts.len(),
            "{:?}",
            with_comment.blocks
        );
        assert_eq!(with_comment.blocks.len(), plain.blocks.len());
        assert!(matches!(&with_comment.blocks[2], RBlock::Heading { plain, .. } if plain == "Two"));
        // The offsets moved by the comment's length, and `block_at` follows
        // them rather than the index the parsed list would have given.
        assert!(with_comment.starts[2] > two);
        assert_eq!(
            with_comment.block_at(text.find("# Two").unwrap()),
            Some(2),
            "a jump to Two has to land on Two"
        );
    }

    /// An offset inside a footnote definition belongs to the Footnotes
    /// section, which is appended after every parsed block and so needs a
    /// start of its own.
    #[test]
    fn an_offset_in_a_definition_resolves_to_the_footnotes_block() {
        let text = "a[^1] b\n\n# H\n\n[^1]: the note\n";
        let m = build("Self.md", text, &stub());
        assert_eq!(m.blocks.len(), m.starts.len());
        let last = m.blocks.len() - 1;
        assert!(matches!(m.blocks[last], RBlock::Footnotes(_)));
        let definition = text.find("[^1]: ").unwrap();
        assert_eq!(m.footnotes, vec![definition]);
        assert_eq!(m.block_at(definition), Some(last));
    }

    /// Clicking reference `n` reveals definition `n`, not the end of the
    /// note; an inline footnote has no definition and points at itself.
    #[test]
    fn each_footnote_knows_where_its_definition_is() {
        let text = "x[^b] y^[inline] z[^a]\n\n[^a]: A text\n[^b]: B text\n";
        let m = build("Self.md", text, &stub());
        assert_eq!(
            m.footnotes,
            vec![
                text.find("[^b]: ").unwrap(),
                0,
                text.find("[^a]: ").unwrap()
            ],
            "b is 1, the inline one is 2, a is 3"
        );
    }

    #[test]
    fn footnotes_number_by_first_reference_and_collect_at_the_end() {
        let m = build(
            "Self.md",
            "x[^b] y^[inline] z[^a] again[^b] none[^zz]\n\n[^a]: A text\n[^b]: B text",
            &stub(),
        );
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "x[1] y[2] z[3] again[1] nonezz");
        assert_eq!(
            t.links.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>(),
            vec![
                LinkTarget::Footnote(1),
                LinkTarget::Footnote(2),
                LinkTarget::Footnote(3),
                LinkTarget::Footnote(1)
            ]
        );
        let RBlock::Footnotes(notes) = m.blocks.last().unwrap() else {
            panic!("{:?}", m.blocks.last())
        };
        assert_eq!(notes.len(), 3);
        assert_eq!(para_text(&notes[0].1[0]).text, "B text");
        assert_eq!(para_text(&notes[1].1[0]).text, "inline");
        assert_eq!(para_text(&notes[2].1[0]).text, "A text");
    }

    #[test]
    fn properties_lists_and_callouts() {
        let m = build(
            "Self.md",
            "---\ntitle: T\ntags: [a, b]\n---\n> [!TIP]- Fold me\n> body\n\n1. one\n2. [x] two\n\n- [ ] task",
            &stub(),
        );
        let RBlock::Properties(entries) = &m.blocks[0] else {
            panic!("{:?}", m.blocks[0])
        };
        assert_eq!(
            entries.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
            vec!["title", "tags"]
        );
        assert_eq!(entries[0].value, PropertyValue::Text("T".into()));
        assert_eq!(
            entries[1].value,
            PropertyValue::List(vec!["a".into(), "b".into()])
        );
        match &m.blocks[1] {
            RBlock::Callout {
                kind, fold, title, ..
            } => {
                assert_eq!(*kind, "tip");
                assert_eq!(*fold, Some(true));
                assert_eq!(title.text, "Fold me");
            }
            other => panic!("{other:?}"),
        }
        match &m.blocks[2] {
            RBlock::List { items, .. } => {
                assert_eq!(items[0].number, Some(1));
                assert_eq!(items[1].number, Some(2));
                assert_eq!(items[1].task, Some('x'));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(m.starts.len(), 4);
        assert_eq!(m.block_at(0), Some(0));
        assert_eq!(m.block_at(m.starts[2] + 3), Some(2));
    }

    #[test]
    fn task_markers_carry_the_line_and_stop_at_an_embed() {
        let vault = Stub {
            files: HashMap::from([("Tasks.md", "- [ ] inner\n")]),
            base_calls: std::cell::Cell::new(0),
        };
        let m = build(
            "Self.md",
            "---\na: 1\n---\n\n- [ ] alpha\n  - [x] beta\n\n![[Tasks]]\n",
            &vault,
        );
        let RBlock::List { items, .. } = &m.blocks[1] else {
            panic!("{:?}", m.blocks[1])
        };
        assert_eq!(items[0].marker, Some(TaskMarker { line: 4, col: 2 }));
        let RBlock::List { items: nested, .. } = &items[0].children[1] else {
            panic!("{:?}", items[0].children)
        };
        assert_eq!(nested[0].marker, Some(TaskMarker { line: 5, col: 4 }));
        // The embedded item's bytes are another file's, so it carries no
        // marker and the reading view leaves its box inert.
        let RBlock::Paragraph(segments) = m.blocks.last().unwrap() else {
            panic!("{:?}", m.blocks.last())
        };
        let Segment::Embed(embed) = &segments[0] else {
            panic!("{:?}", segments[0])
        };
        let EmbedKind::Note { blocks, .. } = &embed.kind else {
            panic!("{:?}", embed.kind)
        };
        let RBlock::List { items, .. } = &blocks[0] else {
            panic!("{blocks:?}")
        };
        assert_eq!(items[0].task, Some(' '));
        assert_eq!(items[0].marker, None);
    }

    #[test]
    fn a_code_block_copies_lf_text_with_one_newline_removed() {
        let m = build("Self.md", "```js\nlet x = 1;\n\n\n```\n", &stub());
        match &m.blocks[0] {
            RBlock::Code {
                language,
                text,
                copy,
                ..
            } => {
                assert_eq!(language.as_deref(), Some("js"));
                assert_eq!(text, "let x = 1;");
                assert_eq!(copy, "let x = 1;\n\n");
            }
            other => panic!("{other:?}"),
        }
        // A CRLF file renders and copies as LF, the way Obsidian does,
        // while the file keeps its own endings.
        let m = build("Self.md", "```\r\none\r\ntwo\r\n```\r\n", &stub());
        match &m.blocks[0] {
            RBlock::Code { text, copy, .. } => {
                assert_eq!(text, "one\ntwo");
                assert_eq!(copy, "one\ntwo");
            }
            other => panic!("{other:?}"),
        }
    }

    fn widgets(src: &str, vault: &dyn EmbedSource) -> Vec<Widget> {
        let doc = parse(src);
        build_widgets("Self.md", &doc, vault, BuildOptions::default())
    }

    #[test]
    fn live_preview_makes_a_callout_a_widget() {
        let src = "top\n\n> [!warning]- Careful\n> body\n> more\n\n> plain quote\n";
        let w = widgets(src, &stub());
        assert_eq!(w.len(), 1, "the plain quote is not a widget: {w:?}");
        let RBlock::Callout {
            kind,
            fold,
            title,
            children,
        } = &w[0].block
        else {
            panic!("{:?}", w[0].block)
        };
        assert_eq!(*kind, "warning");
        assert_eq!(*fold, Some(true), "`-` opens collapsed, as in Obsidian");
        assert_eq!(title.text, "Careful");
        assert_eq!(children.len(), 1);
        // The span covers every line of the callout, so Live Preview folds
        // all three away behind the widget.
        assert_eq!(
            &src[w[0].span.clone()],
            "> [!warning]- Careful\n> body\n> more"
        );
    }

    #[test]
    fn an_image_html_block_becomes_an_image_widget() {
        let w = widgets("<img src=\"pic.png\" width=\"100\" alt=\"A\">\n", &stub());
        assert_eq!(w.len(), 1);
        let RBlock::Paragraph(segs) = &w[0].block else {
            panic!("{:?}", w[0].block)
        };
        match &segs[0] {
            Segment::Embed(Embed {
                kind: EmbedKind::Image(path),
                alt,
                width,
                height,
                ..
            }) => {
                assert_eq!(path, &PathBuf::from("/vault/pic.png"));
                assert_eq!(alt, "A");
                assert_eq!((*width, *height), (Some(100.0), None));
            }
            other => panic!("{other:?}"),
        }
        // Consecutive raw imgs are one block and render as one run of
        // images; a percent-escaped src is the vault path it decodes to.
        let m = build(
            "Self.md",
            "<img src='pic.png'/>\n<img src=pic%2Epng height=50>\n",
            &stub(),
        );
        let RBlock::Paragraph(segs) = &m.blocks[0] else {
            panic!("{:?}", m.blocks[0])
        };
        assert_eq!(segs.len(), 2);
        assert!(matches!(
            &segs[1],
            Segment::Embed(Embed {
                kind: EmbedKind::Image(_),
                height: Some(h),
                ..
            }) if *h == 50.0
        ));
    }

    #[test]
    fn other_html_blocks_stay_source() {
        // A remote image and a mixed block reach the builder as the same
        // `HtmlBlock` and are not something the renderer can draw, so they
        // keep their raw text and never fold their lines. (A comment and a
        // script used to be in this list; they draw nothing at all now --
        // see the test below.)
        for src in [
            "<img src=\"https://e.com/a.png\">\n",
            "<img src=\"pic.png\" width=\"50%\">\n",
            "<table><tr><td>c</td></tr></table>\n",
        ] {
            assert!(widgets(src, &stub()).is_empty(), "{src:?} became a widget");
            let m = build("Self.md", src, &stub());
            assert!(
                matches!(&m.blocks[0], RBlock::Html(_)),
                "{src:?} -> {:?}",
                m.blocks[0]
            );
        }
    }

    /// A raw `<pre>` is a wrapped code block whose copy button needs a
    /// `<code>` child; a `<details>` folds its summary over its body; a
    /// container keeps its content as a paragraph
    /// ([[Specification#`<pre>`]] and the sanitizer table).
    #[test]
    fn pre_details_and_containers_render_rather_than_showing_source() {
        let m = build(
            "Self.md",
            "<pre>**not bold** &amp; two  spaces\n  indented</pre>\n",
            &stub(),
        );
        match &m.blocks[0] {
            RBlock::Code {
                text,
                wrap,
                copyable,
                language,
                ..
            } => {
                assert_eq!(text, "**not bold** & two  spaces\n  indented");
                assert!(*wrap && !*copyable);
                assert_eq!(*language, None);
            }
            other => panic!("{other:?}"),
        }
        let m = build("Self.md", "<pre><code>let x = 1;\n</code></pre>\n", &stub());
        match &m.blocks[0] {
            RBlock::Code { text, copyable, .. } => {
                assert_eq!(text, "let x = 1;");
                assert!(*copyable);
            }
            other => panic!("{other:?}"),
        }
        let m = build(
            "Self.md",
            "<details open>\n<summary>More <b>here</b></summary>\nThe <i>body</i>.\n</details>\n",
            &stub(),
        );
        match &m.blocks[0] {
            RBlock::Details {
                open,
                summary,
                body,
                ..
            } => {
                assert!(*open);
                assert_eq!(summary.text, "More here");
                assert!(summary
                    .runs
                    .iter()
                    .any(|(r, s)| s.bold && &summary.text[r.clone()] == "here"));
                assert_eq!(body.len(), 1);
                match &body[0] {
                    RBlock::Paragraph(segments) => match &segments[0] {
                        Segment::Text(rich) => assert_eq!(rich.text, "The body."),
                        other => panic!("{other:?}"),
                    },
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
        let m = build(
            "Self.md",
            "<div class=\"x\">mixed <b>bold</b> &amp; plain</div>\n",
            &stub(),
        );
        match &m.blocks[0] {
            RBlock::Paragraph(segments) => match &segments[0] {
                Segment::Text(rich) => assert_eq!(rich.text, "mixed bold & plain"),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
        // Each of those is a widget in Live Preview; a table stays source.
        assert_eq!(widgets("<pre>x</pre>\n", &stub()).len(), 1);
        assert_eq!(
            widgets("<details><summary>s</summary>b</details>\n", &stub()).len(),
            1
        );
        assert_eq!(widgets("<center>c</center>\n", &stub()).len(), 1);
        assert!(widgets("<table><tr><td>c</td></tr></table>\n", &stub()).is_empty());
    }

    /// Obsidian's sanitizer takes a `<script>` out whole and keeps what
    /// follows it, and no DOM paints a comment node: either one drawn as
    /// muted source is the one case where that fallback is wrong rather
    /// than merely plain.
    #[test]
    fn a_comment_or_script_block_draws_nothing() {
        for src in [
            "<!-- a comment -->\n",
            "<!-- one --> <!-- two -->\n",
            "<script>window.x = 1</script>\n",
            "<script type=\"text/javascript\">a > b;</script>\n",
            "<script>a</script>\n<!-- and a comment -->\n",
            // Unterminated: the rest of the block goes with it, as a
            // parser reading to the end of its input does.
            "<!-- never closed\n",
            "<script>never closed\n",
            // The closing tag may carry whitespace before its `>`.
            "<script>x</script >\n",
        ] {
            let m = build("Self.md", src, &stub());
            assert!(m.blocks.is_empty(), "{src:?} -> {:?}", m.blocks);
            assert!(widgets(src, &stub()).is_empty(), "{src:?} became a widget");
        }
        // The text after a script is its own block and still renders, and
        // the block offsets stay in step with the blocks kept.
        let m = build("Self.md", "<script>x</script>\n\nafter\n", &stub());
        assert_eq!(m.blocks.len(), 1);
        assert!(
            matches!(&m.blocks[0], RBlock::Paragraph(_)),
            "{:?}",
            m.blocks[0]
        );
        assert_eq!(m.starts.len(), 1);
        // A comment beside a container drops out and the container draws
        // its content, the way the inline overlay reads a comment.
        for src in [
            "<!-- a comment --><div>x</div>\n",
            "<div>x</div>\n<!-- a comment -->\n",
        ] {
            let m = build("Self.md", src, &stub());
            match &m.blocks[0] {
                RBlock::Paragraph(segments) => match &segments[0] {
                    Segment::Text(rich) => assert_eq!(rich.text.trim(), "x", "{src:?}"),
                    other => panic!("{src:?} -> {other:?}"),
                },
                other => panic!("{src:?} -> {other:?}"),
            }
        }
    }

    #[test]
    fn markdown_image_size_hints_match_the_wikilink_form() {
        let m = build(
            "Self.md",
            "![a|100](pic.png)\n\n![a|200x100](pic.png)\n\n![a](pic.png)\n\n\
             ![a|wide](pic.png)\n\n![[pic.png|wide|100]]",
            &stub(),
        );
        let e = |ix: usize| {
            let e = first_embed(&m.blocks[ix]);
            assert!(matches!(e.kind, EmbedKind::Image(_)), "{e:?}");
            (e.alt.as_str(), e.width, e.height)
        };
        assert_eq!(e(0), ("a", Some(100.), None));
        assert_eq!(e(1), ("a", Some(200.), Some(100.)));
        assert_eq!(e(2), ("a", None, None));
        // A part that is not a size stays in the alt rather than vanishing.
        assert_eq!(e(3), ("a|wide", None, None));
        // The wikilink spelling goes through the same split.
        assert_eq!(e(4), ("wide", Some(100.), None));
    }

    #[test]
    fn an_embed_outside_a_paragraph_is_a_link_not_its_brackets() {
        let m = build(
            "Self.md",
            "# See ![[pic.png|200]]\n\n| a |\n| --- |\n| ![[Other]] |",
            &stub(),
        );
        let link = InlineStyle {
            link: true,
            ..InlineStyle::default()
        };
        match &m.blocks[0] {
            RBlock::Heading { text, .. } => {
                assert_eq!(text.text, "See pic.png");
                assert_eq!(text.runs, vec![(4..11, link)]);
                assert_eq!(
                    text.links,
                    vec![(4..11, LinkTarget::Internal("pic.png".into()))]
                );
            }
            other => panic!("{other:?}"),
        }
        match &m.blocks[1] {
            RBlock::Table { rows, .. } => {
                let cell = &rows[0][0];
                assert_eq!(cell.text, "Other");
                assert_eq!(cell.runs, vec![(0..5, link)]);
                assert_eq!(
                    cell.links,
                    vec![(0..5, LinkTarget::Internal("Other".into()))]
                );
            }
            other => panic!("{other:?}"),
        }
    }

    /// The style of the run holding `word`, or plain when no run does.
    fn style_at(t: &RichText, word: &str) -> InlineStyle {
        let at = t
            .text
            .find(word)
            .unwrap_or_else(|| panic!("{word:?} not in {:?}", t.text));
        t.runs
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, s)| *s)
            .unwrap_or_default()
    }

    fn only(f: fn(&mut InlineStyle)) -> InlineStyle {
        let mut s = InlineStyle::default();
        f(&mut s);
        s
    }

    #[test]
    fn inline_html_tags_become_styles() {
        let m = build(
            "Self.md",
            "a <b>b</b> <strong>s</strong> <i>i</i> <em>e</em> <s>x</s> <del>d</del> \
             <STRIKE>k</STRIKE> <mark>m</mark> <u>u</u> <kbd>K</kbd> <sup>2</sup> <sub>0</sub>",
            &stub(),
        );
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "a b s i e x d k m u K 2 0");
        assert_eq!(style_at(t, "a"), InlineStyle::default());
        assert_eq!(style_at(t, "b"), only(|s| s.bold = true));
        assert_eq!(style_at(t, "s"), only(|s| s.bold = true));
        assert_eq!(style_at(t, "i"), only(|s| s.italic = true));
        assert_eq!(style_at(t, "e"), only(|s| s.italic = true));
        assert_eq!(style_at(t, "x"), only(|s| s.strike = true));
        assert_eq!(style_at(t, "d"), only(|s| s.strike = true));
        assert_eq!(style_at(t, "k"), only(|s| s.strike = true));
        assert_eq!(style_at(t, "m"), only(|s| s.highlight = true));
        assert_eq!(style_at(t, "u"), only(|s| s.underline = true));
        assert_eq!(style_at(t, "K"), only(|s| s.kbd = true));
        assert_eq!(style_at(t, "2"), only(|s| s.sup = true));
        assert_eq!(style_at(t, "0"), only(|s| s.sub = true));
        assert!(t.runs.iter().all(|(_, s)| !s.html), "{:?}", t.runs);
    }

    #[test]
    fn inline_html_nests_drops_wrappers_and_ignores_a_stray_closer() {
        let m = build(
            "Self.md",
            "<u><b>x</b>y</u></b>z<!-- gone --><span class=\"c\">w</span><div>v</div>\
             <font color=\"red\">f</font><center>c</center><small>s</small><p>p</p>",
            &stub(),
        );
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "xyzwvfcsp");
        assert_eq!(
            t.runs,
            vec![
                (
                    0..1,
                    only(|s| {
                        s.underline = true;
                        s.bold = true;
                    })
                ),
                (1..2, only(|s| s.underline = true)),
            ]
        );
    }

    #[test]
    fn a_br_is_a_line_break() {
        let m = build("Self.md", "one<br>two<br/>three<br />four<BR>five", &stub());
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "one\ntwo\nthree\nfour\nfive");
        assert!(t.runs.is_empty(), "{:?}", t.runs);
    }

    #[test]
    fn a_raw_anchor_is_an_external_link_kept_as_written() {
        // Obsidian resolves nothing in a raw href, so `Other.md` is not an
        // internal link, and a `javascript:` href is dropped to plain text.
        let m = build(
            "Self.md",
            "see <a href=\"Other.md\">the note</a> and <a href='javascript:alert(1)'>no</a> \
             <a>bare</a>",
            &stub(),
        );
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "see the note and no bare");
        assert_eq!(t.runs, vec![(4..12, only(|s| s.link = true))]);
        assert_eq!(
            t.links,
            vec![(4..12, LinkTarget::External("Other.md".into()))]
        );
    }

    #[test]
    fn an_unclosed_tag_runs_to_the_end_of_its_paragraph() {
        // The spec's `start <b>bold never closed`: closed at the end of its
        // paragraph, nothing leaking into the next block.
        let m = build(
            "Self.md",
            "start <b>bold never closed\n\n<a href=\"https://e.com\">open link\n\nplain",
            &stub(),
        );
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "start bold never closed");
        assert_eq!(t.runs, vec![(6..23, only(|s| s.bold = true))]);
        let t = para_text(&m.blocks[1]);
        assert_eq!(
            t.links,
            vec![(0..9, LinkTarget::External("https://e.com".into()))]
        );
        assert!(para_text(&m.blocks[2]).runs.is_empty());
    }

    #[test]
    fn inline_html_outside_the_subset_stays_muted_source() {
        let m = build("Self.md", "a <script>x</script> b <q>c</q> <b", &stub());
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "a <script>x</script> b <q>c</q> <b");
        let source: Vec<&str> = t
            .runs
            .iter()
            .filter(|(_, s)| s.html)
            .map(|(r, _)| &t.text[r.clone()])
            .collect();
        assert_eq!(source, vec!["<script>", "</script>", "<q>", "</q>"]);
        assert!(t.runs.iter().all(|(_, s)| *s == only(|s| s.html = true)));
    }

    #[test]
    fn html_styles_overlay_markdown_nesting() {
        // The emphasis closes before the bold and the bold after it; each
        // extent is its own, so neither restores over the other.
        let m = build("Self.md", "*a <b>b* c</b> d", &stub());
        let t = para_text(&m.blocks[0]);
        assert_eq!(t.text, "a b c d");
        assert_eq!(style_at(t, "a"), only(|s| s.italic = true));
        assert_eq!(
            style_at(t, "b"),
            only(|s| {
                s.italic = true;
                s.bold = true;
            })
        );
        assert_eq!(style_at(t, "c"), only(|s| s.bold = true));
        assert_eq!(style_at(t, "d"), InlineStyle::default());
    }

    #[test]
    fn callout_without_title_shows_its_type() {
        let m = build("Self.md", "> [!warning]\n> text", &stub());
        match &m.blocks[0] {
            RBlock::Callout { title, fold, .. } => {
                assert_eq!(title.text, "Warning");
                assert_eq!(*fold, None);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn display_math_in_a_paragraph_is_its_own_segment() {
        let opts = BuildOptions::default();
        let m = build_with("A.md", "Inline $x$ then $$y$$ and after.", &stub(), opts);
        let RBlock::Paragraph(segments) = &m.blocks[0] else {
            panic!("{:?}", m.blocks[0])
        };
        // The space on either side of the block is its margin, not text.
        let texts: Vec<_> = segments
            .iter()
            .map(|s| match s {
                Segment::Text(t) => format!("text:{}", t.text),
                Segment::Math(tex) => format!("math:{tex}"),
                Segment::Embed(_) => "embed".to_string(),
            })
            .collect();
        assert_eq!(
            texts,
            vec!["text:Inline x then", "math:y", "text:and after."]
        );
        // Inline math is a styled run holding the TeX, for the view to
        // typeset over a placeholder.
        let Segment::Text(first) = &segments[0] else {
            unreachable!()
        };
        assert_eq!(
            first.runs,
            vec![(
                7..8,
                InlineStyle {
                    math: true,
                    ..InlineStyle::default()
                }
            )]
        );
        // The block form is a block.
        let m = build_with("A.md", "$$\nz\n$$", &stub(), BuildOptions::default());
        assert_eq!(m.blocks[0], RBlock::Math("z".to_string()));
    }
}
