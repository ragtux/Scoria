use super::*;
use crate::layout::Item;
use std::sync::Arc;

const REGULAR: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-Regular.ttf");
const ITALIC: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-Italic.ttf");
const BOLD: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-Bold.ttf");
const BOLD_ITALIC: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-BoldItalic.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-SemiBold.ttf");
const DISPLAY: &[u8] = include_bytes!("../../scoria/assets/fonts/Rubik-Black.ttf");

pub(crate) fn fonts() -> Fonts {
    Fonts::new(FontFiles {
        regular: REGULAR,
        italic: ITALIC,
        bold: BOLD,
        bold_italic: BOLD_ITALIC,
        semibold: SEMIBOLD,
        display: DISPLAY,
    })
}

pub(crate) fn para(text: &str) -> Block {
    Block::Paragraph(vec![Inline::plain(text)])
}

fn lorem(n: usize) -> Vec<Block> {
    (1..=n)
        .map(|i| {
            para(&format!(
                "Paragraph {i}. Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
                 Lorem ipsum dolor sit amet, consectetur adipiscing elit. Lorem ipsum dolor \
                 sit amet, consectetur adipiscing elit."
            ))
        })
        .collect()
}

// ---- settings and geometry ------------------------------------------------------

#[test]
fn page_sizes_are_chromiums_points_and_landscape_swaps_them() {
    let mut s = PdfSettings::default();
    assert_eq!(s.page_size, PageSize::Letter);
    assert_eq!(s.geometry().width, 612.0);
    assert_eq!(s.geometry().height, 792.0);
    s.page_size = PageSize::A4;
    assert_eq!(s.page_size.points(), (595.92, 842.88));
    s.landscape = true;
    let g = s.geometry();
    assert_eq!((g.width, g.height), (842.88, 595.92));
    assert_eq!(PageSize::parse("Tabloid"), Some(PageSize::Tabloid));
    assert_eq!(PageSize::parse("letter"), None);
}

#[test]
fn margin_values_are_obsidians_strings_and_the_text_edge_is_where_it_was_measured() {
    assert_eq!(Margin::Default.value(), "0");
    assert_eq!(Margin::None.value(), "1");
    assert_eq!(Margin::Minimal.value(), "2");
    assert_eq!(Margin::parse("2"), Some(Margin::Minimal));
    let labels: Vec<_> = Margin::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(labels, ["Default", "Minimal", "None"]);
    let mut s = PdfSettings::default();
    assert_eq!(s.geometry().origin.0, 51.75);
    s.margin = Margin::Minimal;
    assert_eq!(s.geometry().origin.0, 24.0);
    s.margin = Margin::None;
    assert_eq!(s.geometry().origin.0, 24.0);
}

#[test]
fn downscale_widens_the_content_box_in_layout_pixels() {
    let mut s = PdfSettings::default();
    let full = s.geometry();
    s.downscale_percent = 50;
    let half = s.geometry();
    assert!((half.content.0 - 2.0 * full.content.0).abs() < 0.01);
    assert!((half.scale - 0.375).abs() < 1e-6);
}

// ---- layout and pagination -----------------------------------------------------

#[test]
fn sixty_paragraphs_take_several_letter_pages_and_fewer_at_tabloid_landscape_and_half_scale() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = lorem(60);
    let (_, letter) = paginate("Note", &blocks, &PdfSettings::default(), &fonts, &palette);
    // Obsidian took 5 Letter pages for this fixture; the layout is not
    // Chromium's, so only the neighbourhood is held.
    assert!((4..=7).contains(&letter.len()), "{} pages", letter.len());
    let small = PdfSettings {
        page_size: PageSize::Tabloid,
        landscape: true,
        downscale_percent: 50,
        ..PdfSettings::default()
    };
    let (_, tabloid) = paginate("Note", &blocks, &small, &fonts, &palette);
    assert!((1..=3).contains(&tabloid.len()), "{} pages", tabloid.len());
    // Every paragraph is somewhere, in order.
    let all: String = letter
        .iter()
        .map(|p| p.text())
        .collect::<Vec<_>>()
        .join(" ");
    let first = all.find("Paragraph 1.").unwrap();
    let last = all.find("Paragraph 60.").unwrap();
    assert!(first < last);
}

#[test]
fn nothing_is_drawn_below_the_content_box() {
    let fonts = fonts();
    let palette = Palette::light();
    let (g, pages) = paginate(
        "Note",
        &lorem(30),
        &PdfSettings::default(),
        &fonts,
        &palette,
    );
    for page in &pages {
        for item in &page.items {
            if let Item::Text(run) = item {
                assert!(run.y <= g.content.1 + 0.01, "{} at {}", run.text, run.y);
                assert!(
                    run.x + run.width() <= g.content.0 + 0.5,
                    "{} too wide",
                    run.text
                );
            }
        }
    }
}

#[test]
fn the_title_comes_first_only_when_asked_for() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = vec![para("Body")];
    let (_, without) = paginate(
        "My note",
        &blocks,
        &PdfSettings::default(),
        &fonts,
        &palette,
    );
    assert_eq!(without[0].text(), "Body");
    let with = PdfSettings {
        include_name: true,
        ..PdfSettings::default()
    };
    let (_, pages) = paginate("My note", &blocks, &with, &fonts, &palette);
    assert_eq!(pages[0].text(), "My note Body");
}

#[test]
fn a_long_word_is_broken_rather_than_overflowing() {
    let fonts = fonts();
    let palette = Palette::light();
    let word = "x".repeat(400);
    let (pages, _) = layout::layout(&[para(&word)], None, 300.0, None, &fonts, &palette);
    let mut lines = 0;
    for item in &pages[0].items {
        if let Item::Text(run) = item {
            assert!(run.x + run.width() <= 300.5);
            lines += 1;
        }
    }
    assert!(lines > 1);
}

#[test]
fn a_heading_is_bold_and_larger_than_body_text() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = vec![
        Block::Heading {
            level: 1,
            content: vec![Inline::plain("Title")],
        },
        para("Body"),
    ];
    let (pages, _) = layout::layout(&blocks, None, 500.0, None, &fonts, &palette);
    let runs: Vec<_> = pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Text(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(runs[0].face, Face::Bold);
    assert!(runs[0].size > runs[1].size);
    assert!(runs[0].y < runs[1].y);
}

#[test]
fn code_is_set_in_the_typewriter_face_and_highlighted() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = vec![Block::Code {
        language: Some("rust".into()),
        text: "fn main() {\n    let x = 1;\n}\n".into(),
    }];
    let (pages, _) = layout::layout(&blocks, None, 500.0, None, &fonts, &palette);
    let runs: Vec<_> = pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Text(r) => Some(r),
            _ => None,
        })
        .collect();
    assert!(runs.iter().all(|r| r.face == Face::Mono));
    let keyword = runs.iter().find(|r| r.text == "fn").unwrap();
    assert_eq!(keyword.color, palette.code[2]);
    // The indentation is kept: `let` sits right of `fn`.
    let let_ = runs.iter().find(|r| r.text.contains("let")).unwrap();
    assert!(let_.x > keyword.x);
    // The block's ground is painted behind it.
    assert!(pages[0]
        .items
        .iter()
        .any(|i| matches!(i, Item::Rect { color, .. } if *color == palette.code_background)));
}

#[test]
fn math_is_typeset_in_the_katex_faces() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = vec![
        Block::Math(r"\frac{a}{b}".into()),
        Block::Paragraph(vec![Inline::plain("so "), Inline::Math("x^2".into())]),
    ];
    let (pages, _) = layout::layout(&blocks, None, 500.0, None, &fonts, &palette);
    let math: Vec<_> = pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Text(r) if matches!(r.face, Face::Math(_)) => Some(r),
            _ => None,
        })
        .collect();
    assert!(math.len() >= 4, "{math:?}");
    assert!(math
        .iter()
        .all(|r| r.glyphs[0].id != 0 && r.glyphs[0].id != u16::MAX));
    // The fraction bar.
    assert!(pages[0]
        .items
        .iter()
        .any(|i| matches!(i, Item::Rect { .. })));
}

#[test]
fn a_container_broken_across_pages_paints_its_bar_on_both() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = vec![Block::Quote(lorem(40))];
    let (pages, _) = layout::layout(&blocks, None, 400.0, Some(500.0), &fonts, &palette);
    assert!(pages.len() > 1);
    for page in &pages {
        assert!(page.items.iter().any(
            |i| matches!(i, Item::Rect { color, w, .. } if *color == palette.quote_border && *w == 2.0)
        ));
    }
}

#[test]
fn lists_draw_markers_and_tables_their_rules() {
    let fonts = fonts();
    let palette = Palette::light();
    let blocks = vec![
        Block::List {
            tight: true,
            items: vec![
                ListItem {
                    marker: Marker::Number(1),
                    children: vec![para("one")],
                },
                ListItem {
                    marker: Marker::Task { done: true },
                    children: vec![para("done")],
                },
            ],
        },
        Block::Table {
            alignments: vec![Align::Left, Align::Right],
            header: vec![vec![Inline::plain("A")], vec![Inline::plain("B")]],
            rows: vec![vec![vec![Inline::plain("1")], vec![Inline::plain("2")]]],
        },
    ];
    let (pages, _) = layout::layout(&blocks, None, 500.0, None, &fonts, &palette);
    let text = pages[0].text();
    assert!(text.contains("1. one"), "{text}");
    assert!(text.contains("A B 1 2"), "{text}");
    // The done task is struck through.
    assert!(pages[0]
        .items
        .iter()
        .any(|i| matches!(i, Item::Line { .. })));
    assert!(pages[0]
        .items
        .iter()
        .any(|i| matches!(i, Item::Path { .. })));
}

// ---- the written PDF ------------------------------------------------------------

#[test]
fn the_pdf_reads_back_with_its_pages_size_title_and_text() {
    let fonts = fonts();
    let palette = Palette::light();
    let mut blocks = vec![Block::Heading {
        level: 1,
        content: vec![Inline::plain("Heading")],
    }];
    blocks.push(Block::Paragraph(vec![
        Inline::plain("A link to "),
        Inline::Text {
            text: "the site".into(),
            style: SpanStyle::default(),
            link: Some("https://scoria.cc/".into()),
        },
        Inline::plain(" and café."),
    ]));
    blocks.extend(lorem(60));
    let settings = PdfSettings::default();
    let (_, pages) = paginate("Note", &blocks, &settings, &fonts, &palette);
    let bytes = export_pdf("Note", &blocks, &settings, &fonts, &palette).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let doc = scoria_pdf::Document::open(bytes.clone()).unwrap();
    assert_eq!(doc.page_count(), pages.len());
    let size = doc.page_size(0).unwrap();
    assert_eq!((size.width, size.height), (612.0, 792.0));
    assert_eq!(doc.title().as_deref(), Some("Note"));
    let text = doc.page_text(0).unwrap();
    assert!(text.contains("Heading"), "{text}");
    assert!(text.contains("café"), "{text}");
    assert!(text.contains("Paragraph 1."), "{text}");
    let last = doc.page_text(doc.page_count() - 1).unwrap();
    assert!(last.contains("Paragraph 60."), "{last}");
    // The link is an annotation.
    let raw = String::from_utf8_lossy(&bytes);
    assert!(
        raw.contains("/Annot") || raw.contains("/Link"),
        "no link annotation"
    );
    // The fonts are embedded subsets.
    assert!(raw.contains("FontFile2") || raw.contains("FontFile3"));
}

#[test]
fn a_landscape_a5_pdf_has_the_swapped_size() {
    let fonts = fonts();
    let palette = Palette::light();
    let settings = PdfSettings {
        page_size: PageSize::A5,
        landscape: true,
        ..PdfSettings::default()
    };
    let bytes = export_pdf("x", &[para("Hello")], &settings, &fonts, &palette).unwrap();
    let doc = scoria_pdf::Document::open(bytes).unwrap();
    let size = doc.page_size(0).unwrap();
    assert_eq!((size.width, size.height), (595.92, 420.0));
}

#[test]
fn an_empty_note_is_still_one_page() {
    let fonts = fonts();
    let palette = Palette::light();
    let bytes = export_pdf("x", &[], &PdfSettings::default(), &fonts, &palette).unwrap();
    let doc = scoria_pdf::Document::open(bytes).unwrap();
    assert_eq!(doc.page_count(), 1);
}

#[test]
fn an_image_is_drawn_into_the_pdf() {
    let fonts = fonts();
    let palette = Palette::light();
    let mut png = Vec::new();
    {
        let mut img = image::RgbaImage::new(40, 20);
        for p in img.pixels_mut() {
            *p = image::Rgba([200, 30, 30, 255]);
        }
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
    }
    let blocks = vec![Block::Image {
        data: Arc::new(png),
        width: Some(200.0),
        height: None,
    }];
    let bytes = export_pdf("x", &blocks, &PdfSettings::default(), &fonts, &palette).unwrap();
    let doc = scoria_pdf::Document::open(bytes).unwrap();
    let page = doc.render(0, 1.0).unwrap();
    // Somewhere near the top left of the content box is the red.
    let at = |x: u32, y: u32| {
        let i = ((y * page.width + x) * 4) as usize;
        (page.rgba[i], page.rgba[i + 1], page.rgba[i + 2])
    };
    let (r, g, b) = at(60, 60);
    assert!(r > 150 && g < 80 && b < 80, "{:?}", (r, g, b));
}
