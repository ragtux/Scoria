//! `workspace:export-pdf` ("Export to PDF...") and `canvas:export-as-image`
//! ("Canvas: Export as image"), as Obsidian 1.15.0 did them for the
//! 2026-11-10 probe ([[Specification#Export to PDF]],
//! [[Specification#Export as image]]). The modals are
//! [`crate::export_modals`]; the pages and the pixels are `scoria-export`'s,
//! laid out from the reading view's own model of each note and card, which
//! the one parser built.
//!
//! The run: the modal opens with `app.json`'s `pdfExportSettings` (the
//! defaults when there are none); "Export to PDF" writes them back, asks
//! the desktop's save dialog for `<basename>.pdf`, writes the file, closes
//! the modal, says `PDF Saved to <absolute path>` and opens the PDF in the
//! system's viewer. Cancelled at the dialog, nothing is written and the
//! modal stays open.

use super::*;
use crate::export_modals::{ImageExportEvent, PdfExportEvent};
use crate::reading::model::{EmbedKind, InlineStyle, LinkTarget, RBlock, RichText, Segment};
use crate::reading::EmbedSource;
use scoria_export as ex;
use scoria_formats::config::PdfExportSettings;

impl Workspace {
    /// The exports' check callbacks; `None` for any other id.
    pub(super) fn exports_can_run(&self, id: &str, cx: &App) -> Option<bool> {
        match id {
            // A note, in any of the three modes; not a canvas, an image or
            // an empty tab (observed).
            "workspace:export-pdf" => Some(self.active_note_is_markdown()),
            // A canvas only.
            "canvas:export-as-image" => Some(
                self.main_view(cx) == MainView::Canvas && self.canvas.read(cx).path().is_some(),
            ),
            _ => None,
        }
    }

    /// Runs one of the export ids; `false` for any other.
    pub(super) fn run_export_command(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match id {
            "workspace:export-pdf" => {
                if self.exports_can_run(id, cx) == Some(true) {
                    self.show_export_pdf(window, cx);
                }
                true
            }
            "canvas:export-as-image" => {
                if self.exports_can_run(id, cx) == Some(true) {
                    self.show_export_image(window, cx);
                }
                true
            }
            _ => false,
        }
    }

    /// `workspace:export-pdf`: the modal, over the active note, with the
    /// last export's values.
    fn show_export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.current.clone() else {
            return;
        };
        let settings = self
            .state
            .as_ref()
            .map(|s| pdf_settings(s.app().pdf_export_settings.as_ref()))
            .unwrap_or_default();
        let name = basename(&path);
        self.pdf_export
            .update(cx, |m, cx| m.open(name, settings, cx));
        self.modal = Modal::PdfExport;
        window.focus(&self.pdf_export.focus_handle(cx), cx);
        cx.notify();
    }

    pub(super) fn on_pdf_export_event(
        &mut self,
        event: &PdfExportEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            PdfExportEvent::Close => self.close_modal(window, cx),
            PdfExportEvent::Export(settings) => {
                let touched = self.pdf_export.read(cx).include_touched;
                self.export_pdf(*settings, touched, window, cx)
            }
        }
    }

    /// "Export to PDF": the settings to `app.json`, the save dialog, the
    /// file.
    fn export_pdf(
        &mut self,
        settings: ex::PdfSettings,
        include_touched: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(state), Some(path)) = (self.state.clone(), self.current.clone()) else {
            return;
        };
        let mut app = state.app().clone();
        app.pdf_export_settings = Some(pdf_export_config(
            app.pdf_export_settings.as_ref(),
            &settings,
            include_touched,
        ));
        if state.app().pdf_export_settings != app.pdf_export_settings {
            self.change_setting(SettingChange::App(app), cx);
        }
        // What the note says now: the buffer when it is being edited, which
        // the autosave may not have written yet.
        let text = if self.main_view(cx) == MainView::Editor {
            self.editor.read(cx).text().to_string()
        } else {
            match EmbedSource::read(state.as_ref(), &path) {
                Some(text) => text,
                None => {
                    self.report(format!("Could not read {path}"), cx);
                    return;
                }
            }
        };
        let model = reading::build_with(&path, &text, state.as_ref(), state.build_options());
        let light = Theme::light();
        let palette = export_palette(&light);
        let blocks = export_blocks(&model.blocks, &light, &palette);
        let name = basename(&path);
        let picked = cx.prompt_for_new_path(&state.root, Some(&format!("{name}.pdf")));
        cx.spawn_in(window, async move |this, cx| {
            let target = match picked.await {
                Ok(Ok(Some(target))) => target,
                // Cancelled: nothing written, and the modal stays.
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(err)) => {
                    this.update(cx, |this, cx| {
                        this.report(format!("Could not open the dialog: {err}"), cx)
                    })
                    .ok();
                    return;
                }
            };
            let out = target.clone();
            let written = cx
                .background_executor()
                .spawn(async move {
                    let fonts = ex::Fonts::new(crate::assets::export_fonts());
                    let bytes = ex::export_pdf(&name, &blocks, &settings, &fonts, &palette)?;
                    std::fs::write(&out, bytes).map_err(|e| e.to_string())
                })
                .await;
            this.update_in(cx, |this, window, cx| match written {
                Ok(()) => {
                    this.close_modal(window, cx);
                    let shown = target.canonicalize().unwrap_or(target.clone());
                    this.report(format!("PDF Saved to {}", shown.display()), cx);
                    cx.open_url(&format!("file://{}", shown.display()));
                }
                Err(err) => this.report(format!("Could not write {}: {err}", target.display()), cx),
            })
            .ok();
        })
        .detach();
    }

    /// `canvas:export-as-image`: the modal over the canvas on screen, every
    /// row at its default.
    fn show_export_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.canvas.read(cx);
        let Some(path) = view.path().map(str::to_string) else {
            return;
        };
        let canvas = view.scene.canvas.clone();
        let area = view.view_area();
        self.image_export
            .update(cx, |m, cx| m.open(basename(&path), canvas, area, cx));
        self.modal = Modal::ImageExport;
        window.focus(&self.image_export.focus_handle(cx), cx);
        cx.notify();
    }

    pub(super) fn on_image_export_event(
        &mut self,
        event: &ImageExportEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ImageExportEvent::Close => self.close_modal(window, cx),
            ImageExportEvent::Save(options) => self.export_image(*options, window, cx),
        }
    }

    /// "Save": the save dialog for `<basename>.png`, then the pixels. No
    /// notice and nothing opened (observed); the modal closes once the file
    /// is written and stays when the dialog is cancelled.
    fn export_image(
        &mut self,
        options: ex::ImageOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let view = self.canvas.read(cx);
        let Some(path) = view.path().map(str::to_string) else {
            return;
        };
        let canvas = view.scene.canvas.clone();
        let theme = cx.global::<Theme>().clone();
        let text = export_palette(&theme);
        let contents = card_contents(&canvas, &path, &state, &theme, &text);
        let palette = canvas_palette(&theme);
        let picked = cx.prompt_for_new_path(&state.root, Some(&format!("{}.png", basename(&path))));
        cx.spawn_in(window, async move |this, cx| {
            let target = match picked.await {
                Ok(Ok(Some(target))) => target,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(err)) => {
                    this.update(cx, |this, cx| {
                        this.report(format!("Could not open the dialog: {err}"), cx)
                    })
                    .ok();
                    return;
                }
            };
            let out = target.clone();
            let written = cx
                .background_executor()
                .spawn(async move {
                    let fonts = ex::Fonts::new(crate::assets::export_fonts());
                    let image = ex::CanvasImage {
                        canvas: &canvas,
                        contents: &contents,
                        palette: &palette,
                        text: &text,
                        fonts: &fonts,
                        logo: crate::assets::logo_svg(),
                    };
                    let bytes = ex::export_image(&image, &options)?;
                    std::fs::write(&out, bytes).map_err(|e| e.to_string())
                })
                .await;
            this.update_in(cx, |this, window, cx| match written {
                Ok(()) => this.close_modal(window, cx),
                Err(err) => this.report(format!("Could not write {}: {err}", target.display()), cx),
            })
            .ok();
        })
        .detach();
    }
}

/// The canvas view's colours in the theme on screen.
pub(crate) fn canvas_palette(theme: &Theme) -> ex::CanvasPalette {
    let c = &theme.canvas;
    let t = &theme.tokens;
    ex::CanvasPalette {
        background: color(c.background),
        dot: color(c.dot),
        card: color(t.background_primary),
        card_border: color(c.card_border),
        label: color(c.label),
        text: color(t.text_normal),
        muted: color(t.text_muted),
        on_color: color(c.on_color),
        presets: c.presets.map(color),
        border_alpha: c.border_alpha,
        tint_alpha: c.tint_alpha,
        wash_alpha: c.wash_alpha,
    }
}

/// What each card shows, worked out the way the canvas view's cards are:
/// a text card's Markdown and a note's (or its section's) through the
/// reading view's model, an image file's bytes, a line for the rest.
pub(crate) fn card_contents(
    canvas: &scoria_formats::canvas::Canvas,
    canvas_path: &str,
    state: &VaultState,
    theme: &Theme,
    palette: &ex::Palette,
) -> HashMap<String, ex::CardContent> {
    use crate::canvas::cards::{locate, subpath_linktext};
    use scoria_formats::canvas::NodeType;
    let opts = state.build_options();
    let mut out = HashMap::new();
    for node in &canvas.nodes {
        let content = match node.kind {
            NodeType::Group => continue,
            NodeType::Text => {
                let text = node.text.clone().unwrap_or_default();
                let model = reading::build_with(canvas_path, &text, state, opts);
                ex::CardContent::Blocks(export_blocks(&model.blocks, theme, palette))
            }
            NodeType::Link => ex::CardContent::Text(node.url.clone().unwrap_or_default()),
            NodeType::File => {
                let file = node.file.clone().unwrap_or_default();
                match locate(state, &file) {
                    Some((path, FileKind::Markdown)) => {
                        let Some(text) = EmbedSource::read(state, &path) else {
                            continue;
                        };
                        let section = node.subpath.as_deref().map(subpath_linktext).and_then(|l| {
                            scoria_markdown::parse_linktext(&l).subpath.and_then(|sub| {
                                reading::model::build_section(&path, &text, &sub, state, opts)
                            })
                        });
                        let model = section
                            .unwrap_or_else(|| reading::build_with(&path, &text, state, opts));
                        ex::CardContent::Blocks(export_blocks(&model.blocks, theme, palette))
                    }
                    Some((path, FileKind::Image)) => {
                        match std::fs::read(EmbedSource::absolute(state, &path)) {
                            Ok(bytes) => ex::CardContent::Image(std::sync::Arc::new(bytes)),
                            Err(_) => ex::CardContent::Text(path),
                        }
                    }
                    Some((path, _)) => ex::CardContent::Text(basename(&path)),
                    None => ex::CardContent::Text(file),
                }
            }
        };
        out.insert(node.id.clone(), content);
    }
    out
}

/// A path's file name without its extension.
fn basename(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => name.to_string(),
    }
}

/// The modal's values from `pdfExportSettings`: each key Obsidian's
/// default when absent or unreadable.
pub(crate) fn pdf_settings(config: Option<&PdfExportSettings>) -> ex::PdfSettings {
    let mut s = ex::PdfSettings::default();
    let Some(c) = config else {
        return s;
    };
    if let Some(v) = c.include_name {
        s.include_name = v;
    }
    if let Some(p) = c.page_size.as_deref().and_then(ex::PageSize::parse) {
        s.page_size = p;
    }
    if let Some(v) = c.landscape {
        s.landscape = v;
    }
    if let Some(m) = c.margin.as_deref().and_then(ex::Margin::parse) {
        s.margin = m;
    }
    if let Some(d) = c.downscale_percent {
        s.downscale_percent = d.clamp(10, 100) as u8;
    }
    s
}

/// What an export writes to `pdfExportSettings`: every value, `includeName`
/// only once the toggle has been touched or the file already had it (the
/// first export from `{}` wrote the other four). Keys Scoria does not know
/// are kept.
pub(crate) fn pdf_export_config(
    previous: Option<&PdfExportSettings>,
    s: &ex::PdfSettings,
    include_touched: bool,
) -> PdfExportSettings {
    let had_include = previous.is_some_and(|p| p.include_name.is_some());
    PdfExportSettings {
        include_name: (had_include || include_touched).then_some(s.include_name),
        page_size: Some(s.page_size.name().to_string()),
        landscape: Some(s.landscape),
        margin: Some(s.margin.value().to_string()),
        downscale_percent: Some(u32::from(s.downscale_percent)),
        extra: previous.map(|p| p.extra.clone()).unwrap_or_default(),
    }
}

pub(crate) fn color(hsla: Hsla) -> ex::Color {
    let rgba: gpui::Rgba = hsla.into();
    ex::Color::from_f32(rgba.r, rgba.g, rgba.b, rgba.a)
}

/// The colours a theme paints a note in, for the export.
pub(crate) fn export_palette(theme: &Theme) -> ex::Palette {
    let t = &theme.tokens;
    ex::Palette {
        background: color(t.background_primary),
        text: color(t.text_normal),
        muted: color(t.text_muted),
        faint: color(t.text_faint),
        link: color(t.link_color),
        accent: color(t.interactive_accent),
        tag: color(t.tag_color),
        code_background: color(t.code_background),
        code: [
            color(t.code_normal),
            color(t.code_comment),
            color(t.code_keyword),
            color(t.code_string),
            color(t.code_function),
            color(t.code_value),
            color(t.code_operator),
            color(t.code_punctuation),
            color(t.code_property),
            color(t.code_tag),
            color(t.code_important),
        ],
        highlight: color(t.highlight_background),
        rule: color(t.hr_color),
        quote_border: color(t.blockquote_border_color),
        table_border: color(t.table_border_color),
        math_error: color(t.color_red),
    }
}

/// A rich text's runs as export inlines: its styled ranges and its links
/// cut at each other's edges.
fn inlines(text: &RichText, theme: &Theme, palette: &ex::Palette) -> Vec<ex::Inline> {
    let mut cuts = vec![0, text.text.len()];
    for (r, _) in &text.runs {
        cuts.extend([r.start, r.end]);
    }
    for (r, _) in &text.links {
        cuts.extend([r.start, r.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a >= b || !text.text.is_char_boundary(a) || !text.text.is_char_boundary(b) {
            continue;
        }
        let piece = &text.text[a..b];
        let style = text
            .runs
            .iter()
            .find(|(r, _)| r.start <= a && b <= r.end)
            .map(|(_, s)| *s)
            .unwrap_or_default();
        let link = text
            .links
            .iter()
            .find(|(r, _)| r.start <= a && b <= r.end)
            .map(|(_, l)| l);
        if style.math {
            out.push(ex::Inline::Math(piece.to_string()));
            continue;
        }
        let (span, url) = span_style(&style, link, theme, palette);
        out.push(ex::Inline::Text {
            text: piece.to_string(),
            style: span,
            link: url,
        });
    }
    out
}

fn span_style(
    s: &InlineStyle,
    link: Option<&LinkTarget>,
    theme: &Theme,
    palette: &ex::Palette,
) -> (ex::SpanStyle, Option<String>) {
    let mut out = ex::SpanStyle {
        bold: s.bold,
        italic: s.italic,
        strike: s.strike,
        underline: s.underline,
        code: s.code || s.kbd,
        superscript: s.footnote || s.sup,
        ..ex::SpanStyle::default()
    };
    if s.highlight {
        out.background = Some(match s.highlight_color {
            Some(c) => color(theme.highlight_color(c)),
            None => palette.highlight,
        });
    }
    if s.html {
        out.color = Some(palette.muted);
    }
    if s.tag {
        out.color = Some(palette.tag);
    }
    // Only a link to the web is live on paper; one into the vault is drawn
    // as a link and goes nowhere, the vault not being where the PDF is.
    let url = match link {
        Some(LinkTarget::External(url)) => Some(url.clone()),
        Some(_) => {
            out.color = Some(palette.link);
            out.underline = !s.footnote;
            None
        }
        None if s.link || s.footnote => {
            out.color = Some(palette.link);
            None
        }
        None => None,
    };
    (out, url)
}

/// The reading view's blocks as export blocks. Images are read from the
/// vault now; the frontmatter is left out, as Obsidian's export leaves it.
pub(crate) fn export_blocks(
    blocks: &[RBlock],
    theme: &Theme,
    palette: &ex::Palette,
) -> Vec<ex::Block> {
    let mut out = Vec::new();
    for block in blocks {
        export_block(block, theme, palette, &mut out);
    }
    out
}

fn export_block(block: &RBlock, theme: &Theme, palette: &ex::Palette, out: &mut Vec<ex::Block>) {
    let blocks = |b: &[RBlock]| export_blocks(b, theme, palette);
    match block {
        RBlock::Properties(_) => {}
        RBlock::Paragraph(segments) => {
            for segment in segments {
                match segment {
                    Segment::Text(text) => {
                        out.push(ex::Block::Paragraph(inlines(text, theme, palette)))
                    }
                    Segment::Math(tex) => out.push(ex::Block::Math(tex.clone())),
                    Segment::Embed(embed) => match &embed.kind {
                        EmbedKind::Image(path) => match std::fs::read(path) {
                            Ok(bytes) => out.push(ex::Block::Image {
                                data: std::sync::Arc::new(bytes),
                                width: embed.width,
                                height: embed.height,
                            }),
                            Err(_) => out.push(muted_line(&embed.link, palette)),
                        },
                        EmbedKind::Note { blocks: inner, .. } => {
                            out.push(ex::Block::Embed(blocks(inner)))
                        }
                        // A base's rows, a PDF's pages, a remote image and
                        // a missing file are not drawn on paper: the link's
                        // text stands for them.
                        _ => out.push(muted_line(
                            if embed.alt.is_empty() {
                                &embed.link
                            } else {
                                &embed.alt
                            },
                            palette,
                        )),
                    },
                }
            }
        }
        RBlock::Heading { level, text, .. } => out.push(ex::Block::Heading {
            level: *level,
            content: inlines(text, theme, palette),
        }),
        RBlock::ThematicBreak => out.push(ex::Block::Rule),
        RBlock::Code { language, text, .. } => out.push(ex::Block::Code {
            language: language.clone(),
            text: text.clone(),
        }),
        RBlock::Details {
            open,
            summary,
            body,
            ..
        } => {
            out.push(ex::Block::Paragraph(inlines(summary, theme, palette)));
            if *open {
                out.extend(blocks(body));
            }
        }
        RBlock::Html(html) => out.push(muted_line(html.trim_end(), palette)),
        RBlock::Quote(children) => out.push(ex::Block::Quote(blocks(children))),
        RBlock::Callout {
            kind,
            title,
            children,
            fold,
        } => out.push(ex::Block::Callout {
            color: color(theme.callout_color(kind)),
            title: inlines(title, theme, palette),
            // A folded callout prints folded, as the reading view shows it.
            children: if *fold == Some(false) {
                Vec::new()
            } else {
                blocks(children)
            },
        }),
        RBlock::List { tight, items } => out.push(ex::Block::List {
            tight: *tight,
            items: items
                .iter()
                .map(|item| ex::ListItem {
                    marker: match (item.task, item.number) {
                        (Some(c), _) => ex::Marker::Task { done: c != ' ' },
                        (None, Some(n)) => ex::Marker::Number(n),
                        (None, None) => ex::Marker::Bullet,
                    },
                    children: blocks(&item.children),
                })
                .collect(),
        }),
        RBlock::Table {
            alignments,
            header,
            rows,
            ..
        } => out.push(ex::Block::Table {
            alignments: alignments
                .iter()
                .map(|a| match a {
                    scoria_markdown::Alignment::Center => ex::Align::Center,
                    scoria_markdown::Alignment::Right => ex::Align::Right,
                    _ => ex::Align::Left,
                })
                .collect(),
            header: header.iter().map(|c| inlines(c, theme, palette)).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| inlines(c, theme, palette)).collect())
                .collect(),
        }),
        RBlock::Math(tex) => out.push(ex::Block::Math(tex.clone())),
        RBlock::Footnotes(notes) => {
            out.push(ex::Block::Rule);
            out.push(ex::Block::List {
                tight: true,
                items: notes
                    .iter()
                    .map(|(n, body)| ex::ListItem {
                        marker: ex::Marker::Number(*n as u64),
                        children: blocks(body),
                    })
                    .collect(),
            });
        }
    }
}

fn muted_line(text: &str, palette: &ex::Palette) -> ex::Block {
    ex::Block::Paragraph(vec![ex::Inline::styled(
        text,
        ex::SpanStyle {
            color: Some(palette.muted),
            ..ex::SpanStyle::default()
        },
    )])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_pdf_is_runnable() {
        assert!(RUNNABLE.contains(&"workspace:export-pdf"));
    }

    #[test]
    fn export_as_image_is_runnable() {
        assert!(RUNNABLE.contains(&"canvas:export-as-image"));
    }

    /// A canvas with a text card, a note card, an image card, a link card
    /// and a group, through the cards' contents to a PNG: the bounds plus
    /// the margin, the strip, and Scoria's mark in it.
    #[test]
    fn a_canvas_exports_its_cards_and_scorias_mark() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Vault");
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(root.join("pic.png"), &png).unwrap();
        std::fs::write(root.join("Note.md"), "# Top\n\nBody\n\n## Two\n\nSecond\n").unwrap();
        let json = r##"{"nodes":[
            {"id":"t","type":"text","text":"**Hello** canvas","x":0,"y":0,"width":250,"height":60},
            {"id":"n","type":"file","file":"Note.md","subpath":"#Two","x":300,"y":0,"width":250,"height":120},
            {"id":"i","type":"file","file":"pic.png","x":0,"y":200,"width":100,"height":100},
            {"id":"l","type":"link","url":"https://scoria.cc","x":300,"y":200,"width":250,"height":100},
            {"id":"g","type":"group","label":"G","x":-20,"y":180,"width":140,"height":140}
        ],"edges":[]}"##;
        std::fs::write(root.join("Board.canvas"), json).unwrap();
        let state = VaultState::load(&root).unwrap();
        let canvas = scoria_formats::canvas::Canvas::from_json(json).unwrap();
        let theme = Theme::dark();
        let text = export_palette(&theme);
        let contents = card_contents(&canvas, "Board.canvas", &state, &theme, &text);
        assert!(!contents.contains_key("g"));
        match &contents["n"] {
            // The section under `## Two`, not the whole note.
            ex::CardContent::Blocks(blocks) => {
                assert!(matches!(&blocks[0], ex::Block::Heading { level: 2, .. }));
                assert_eq!(blocks.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(contents["i"], ex::CardContent::Image(_)));
        assert_eq!(
            contents["l"],
            ex::CardContent::Text("https://scoria.cc".into())
        );
        let fonts = ex::Fonts::new(crate::assets::export_fonts());
        let palette = canvas_palette(&theme);
        let image = ex::CanvasImage {
            canvas: &canvas,
            contents: &contents,
            palette: &palette,
            text: &text,
            fonts: &fonts,
            logo: crate::assets::logo_svg(),
        };
        let pixmap = ex::render_image(&image, &ex::ImageOptions::default()).unwrap();
        // x -20..550 and y 0..320, plus 100 around, plus the 80 px strip.
        assert_eq!((pixmap.width(), pixmap.height()), (770, 520 + 80));
        // The image card's blue is drawn.
        let px = pixmap.pixel(100 + 50, 100 + 250).unwrap().demultiply();
        assert!(px.blue() > 200 && px.red() < 60, "{px:?}");
        // The strip carries the mark's orange.
        let orange = (520..600).any(|y| {
            (0..200).any(|x| {
                let c = pixmap.pixel(x, y).unwrap().demultiply();
                c.red() > 200 && c.green() < 120 && c.blue() < 60
            })
        });
        assert!(orange);
    }

    #[test]
    fn the_modal_opens_with_the_files_values_and_obsidians_defaults_for_the_rest() {
        assert_eq!(pdf_settings(None), ex::PdfSettings::default());
        let config = PdfExportSettings {
            page_size: Some("A5".into()),
            margin: Some("1".into()),
            downscale_percent: Some(5),
            ..PdfExportSettings::default()
        };
        let s = pdf_settings(Some(&config));
        assert_eq!(s.page_size, ex::PageSize::A5);
        assert_eq!(s.margin, ex::Margin::None);
        assert_eq!(s.downscale_percent, 10);
        assert!(!s.landscape);
        // A page size Obsidian does not offer is its default.
        let odd = PdfExportSettings {
            page_size: Some("B5".into()),
            ..PdfExportSettings::default()
        };
        assert_eq!(pdf_settings(Some(&odd)).page_size, ex::PageSize::Letter);
    }

    #[test]
    fn the_first_export_writes_four_keys_and_include_name_joins_once_touched() {
        let s = ex::PdfSettings::default();
        let first = pdf_export_config(None, &s, false);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            r#"{"pageSize":"Letter","landscape":false,"margin":"0","downscalePercent":100}"#
        );
        let touched = pdf_export_config(Some(&first), &s, true);
        assert_eq!(
            serde_json::to_string(&touched).unwrap(),
            r#"{"includeName":false,"pageSize":"Letter","landscape":false,"margin":"0","downscalePercent":100}"#
        );
        // Once there, it stays, and an unknown key rides along.
        let mut with_extra = touched.clone();
        with_extra.extra.insert("x".into(), serde_json::json!(1));
        let again = pdf_export_config(Some(&with_extra), &s, false);
        assert_eq!(again.include_name, Some(false));
        assert_eq!(again.extra["x"], serde_json::json!(1));
    }

    #[test]
    fn a_rich_text_becomes_inlines_cut_at_its_styles_and_links() {
        let theme = Theme::light();
        let palette = export_palette(&theme);
        let text = RichText {
            id: 0,
            text: "plain bold link x^2".into(),
            runs: vec![
                (
                    6..10,
                    InlineStyle {
                        bold: true,
                        ..InlineStyle::default()
                    },
                ),
                (
                    16..19,
                    InlineStyle {
                        math: true,
                        ..InlineStyle::default()
                    },
                ),
            ],
            links: vec![(11..15, LinkTarget::External("https://scoria.cc".into()))],
        };
        let out = inlines(&text, &theme, &palette);
        let texts: Vec<String> = out
            .iter()
            .map(|i| match i {
                ex::Inline::Text { text, .. } => text.clone(),
                ex::Inline::Math(t) => format!("${t}$"),
                ex::Inline::Break => "\n".into(),
            })
            .collect();
        assert_eq!(texts, ["plain ", "bold", " ", "link", " ", "$x^2$"]);
        match &out[1] {
            ex::Inline::Text { style, .. } => assert!(style.bold),
            other => panic!("{other:?}"),
        }
        match &out[3] {
            ex::Inline::Text { link, .. } => assert_eq!(link.as_deref(), Some("https://scoria.cc")),
            other => panic!("{other:?}"),
        }
    }

    /// The whole path a note takes, the one parser through the reading
    /// view's model to pages: read back with the PDF viewer's own crate.
    #[test]
    fn a_note_with_an_embed_and_an_image_exports_to_pages_hayro_reads() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Vault");
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(root.join("pic.png"), &png).unwrap();
        std::fs::write(root.join("Other.md"), "Embedded words.\n").unwrap();
        let note = "---\ntags: [secret]\n---\n# Title\n\nSome **bold** and $x^2$.\n\n![[Other]]\n\n![[pic.png]]\n\n- [x] done\n- one\n\n```rust\nfn main() {}\n```\n";
        std::fs::write(root.join("Note.md"), note).unwrap();
        let state = VaultState::load(&root).unwrap();
        let model = reading::build_with("Note.md", note, &state, state.build_options());
        let theme = Theme::light();
        let palette = export_palette(&theme);
        let blocks = export_blocks(&model.blocks, &theme, &palette);
        assert!(blocks.iter().any(|b| matches!(b, ex::Block::Image { .. })));
        assert!(blocks.iter().any(|b| matches!(b, ex::Block::Embed(_))));
        let fonts = ex::Fonts::new(crate::assets::export_fonts());
        let settings = ex::PdfSettings {
            include_name: true,
            ..ex::PdfSettings::default()
        };
        let bytes = ex::export_pdf("Note", &blocks, &settings, &fonts, &palette).unwrap();
        let doc = scoria_pdf::Document::open(bytes).unwrap();
        assert_eq!(doc.page_count(), 1);
        let text = doc.page_text(0).unwrap();
        for want in ["Note", "Title", "bold", "Embedded words.", "done", "main"] {
            assert!(text.contains(want), "{want} missing from {text}");
        }
        // The frontmatter is not printed.
        assert!(!text.contains("secret"), "{text}");
    }
}
