//! The crate against what Obsidian 1.15.0 was observed doing on the same
//! bytes (`tools/obsidian-probe/pdf-selection.mjs`, 2026-11-15): the text
//! items a `#selection=` link counts, the password-protected files, and the
//! colours of "Adapt to theme".

use scoria_pdf::{Document, Error, Theming};
use serde_json::Value;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}

/// pdf.js's items per page, as recorded: `[str, hasEOL, x, y, width, height]`.
fn recorded(name: &str) -> Vec<Vec<Value>> {
    let all: Value = serde_json::from_slice(&fixture("probe-items.json")).unwrap();
    all[name]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page.as_array().unwrap().clone())
        .collect()
}

fn check_items(name: &str) {
    let doc = Document::open(fixture(name)).unwrap();
    for (page, want) in recorded(name).iter().enumerate() {
        let height = doc.page_size(page).unwrap().height;
        let got = doc.text_items(page).unwrap();
        let got_strs: Vec<(String, bool)> =
            got.iter().map(|i| (i.text.clone(), i.has_eol)).collect();
        let want_strs: Vec<(String, bool)> = want
            .iter()
            .map(|i| (i[0].as_str().unwrap().to_string(), i[1].as_bool().unwrap()))
            .collect();
        assert_eq!(got_strs, want_strs, "{name} page {}", page + 1);
        for (item, w) in got.iter().zip(want) {
            if item.text.is_empty() {
                continue;
            }
            let x = w[2].as_f64().unwrap() as f32;
            let y = height - w[3].as_f64().unwrap() as f32;
            let width = w[4].as_f64().unwrap() as f32;
            let [left, _, right, _] = item.bounds();
            assert!(
                (left - x).abs() < 0.6,
                "{name} {:?}: x {left} vs {x}",
                item.text
            );
            assert!(
                (item.baseline - y).abs() < 0.6,
                "{name} {:?}: y {} vs {y}",
                item.text,
                item.baseline
            );
            assert!(
                (right - left - width).abs() < 1.0,
                "{name} {:?}: width {} vs {width}",
                item.text,
                right - left
            );
        }
    }
}

#[test]
fn the_hand_written_file_is_cut_into_pdf_js_items() {
    check_items("probe-text.pdf");
}

#[test]
fn the_tagged_typst_file_is_cut_into_pdf_js_items() {
    check_items("probe-doc.pdf");
}

#[test]
fn the_observed_selection_link_names_the_sentence() {
    // `[[doc.pdf#page=1&selection=8,0,8,17|doc, page 1]]` was copied from a
    // selection of exactly `This is page one.`.
    let doc = Document::open(fixture("probe-doc.pdf")).unwrap();
    let items = doc.text_items(0).unwrap();
    assert_eq!(items[8].text, "This is page one.");
    assert_eq!(items[8].utf16_offset(items[8].len()), 17);
}

#[test]
fn a_user_password_opens_the_file_and_a_wrong_one_does_not() {
    for name in ["probe-locked-aes.pdf", "probe-locked-rc4.pdf"] {
        assert_eq!(
            Document::open(fixture(name)).err(),
            Some(Error::Encrypted),
            "{name}"
        );
        assert_eq!(
            Document::open_with_password(fixture(name), "wrong").err(),
            Some(Error::Encrypted),
            "{name}"
        );
        let doc = Document::open_with_password(fixture(name), "secret").unwrap();
        assert_eq!(doc.page_count(), 5, "{name}");
        assert_eq!(doc.text_items(0).unwrap()[8].text, "This is page one.");
    }
    // An owner password alone asks nothing (observed: it opens at once).
    assert_eq!(
        Document::open(fixture("probe-owner-only.pdf"))
            .unwrap()
            .page_count(),
        5
    );
}

#[test]
fn adapt_to_theme_turns_lightness_over_and_leaves_images() {
    let doc = Document::open(fixture("probe-text.pdf")).unwrap();
    let theming = Theming {
        background: [0x1c, 0x1c, 0x1c],
    };
    let bitmap = doc.render_with(0, 1.0, Some(theming)).unwrap();
    // The sample points of the probe, in points from the bottom-left.
    let at = |x: u32, y: u32| {
        let (px, py) = (x, 792 - y);
        let ix = ((py * bitmap.width + px) * 4) as usize;
        [bitmap.rgba[ix], bitmap.rgba[ix + 1], bitmap.rgba[ix + 2]]
    };
    assert_eq!(at(560, 760), [28, 28, 28], "the ground");
    assert_eq!(at(122, 425), [255, 0, 0], "red");
    assert_eq!(at(250, 425), [0, 0, 255], "blue");
    assert_eq!(at(378, 425), [127, 255, 127], "green");
    assert_eq!(at(506, 425), [127, 127, 127], "grey");
    assert_eq!(at(97, 295), [255, 128, 0], "the image's left pixel");
    assert_eq!(at(147, 295), [0, 128, 255], "the image's right pixel");
    let plain = doc.render(0, 1.0).unwrap();
    assert_eq!(plain.rgba[..3], [255, 255, 255]);
}
