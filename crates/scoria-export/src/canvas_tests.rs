//! Export as image, drawn and read back.

use crate::tests::{fonts, para};
use crate::*;
use scoria_formats::canvas::Canvas;
use std::collections::HashMap;

const LOGO: &[u8] = include_bytes!("../../scoria/assets/logo.svg");

fn canvas_palette() -> CanvasPalette {
    CanvasPalette {
        background: Color::rgb(0x1e1e1e),
        dot: Color::rgb(0x444444),
        card: Color::rgb(0x1c1c1c),
        card_border: Color::rgb(0x555555),
        label: Color::rgb(0x999999),
        text: Color::rgb(0xdadada),
        muted: Color::rgb(0xb3b3b3),
        on_color: Color::rgb(0x000000),
        presets: [
            Color::rgb(0xfb464c),
            Color::rgb(0xe9973f),
            Color::rgb(0xe0de71),
            Color::rgb(0x44cf6e),
            Color::rgb(0x53dfdd),
            Color::rgb(0xa882ff),
        ],
        border_alpha: 0.7,
        tint_alpha: 0.07,
        wash_alpha: 0.1,
    }
}

/// The probe's board: nodes over x -50..800, y 0..500.
fn board() -> Canvas {
    Canvas::from_json(
        r##"{"nodes":[
            {"id":"a1","type":"text","text":"Hello canvas","x":0,"y":0,"width":250,"height":60},
            {"id":"g","type":"group","label":"Group","x":-50,"y":200,"width":300,"height":300,"color":"4"},
            {"id":"b","type":"text","text":"Other","x":500,"y":100,"width":300,"height":100,"color":"1"}
        ],"edges":[
            {"id":"e","fromNode":"a1","fromSide":"right","toNode":"b","toSide":"left","label":"to"}
        ]}"##,
    )
    .unwrap()
}

fn decode(png: &[u8]) -> (u32, u32, png::ColorType, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, info.color_type, buf)
}

fn contents() -> HashMap<String, CardContent> {
    let mut map = HashMap::new();
    map.insert(
        "a1".to_string(),
        CardContent::Blocks(vec![para("Hello canvas")]),
    );
    map.insert("b".to_string(), CardContent::Blocks(vec![para("Other")]));
    map
}

#[test]
fn full_canvas_is_the_bounds_plus_a_hundred_and_the_strip_eighty() {
    let canvas = board();
    let opts = ImageOptions::default();
    assert_eq!(estimated_size(&canvas, &opts), (1050, 700));
    assert_eq!(
        estimate_text((1050, 700)),
        "Estimated image dimensions: 1,050px x 700px"
    );
    let fonts = fonts();
    let text = Palette::light();
    let contents = contents();
    let palette = canvas_palette();
    let image = CanvasImage {
        canvas: &canvas,
        contents: &contents,
        palette: &palette,
        text: &text,
        fonts: &fonts,
        logo: LOGO,
    };
    let (w, h, color, _) = decode(&export_image(&image, &opts).unwrap());
    assert_eq!((w, h, color), (1050, 780, png::ColorType::Rgba));
    let no_logo = ImageOptions {
        show_logo: false,
        ..opts
    };
    let (w, h, _, _) = decode(&export_image(&image, &no_logo).unwrap());
    assert_eq!((w, h), (1050, 700));
}

#[test]
fn the_zoom_slider_scales_by_a_power_of_two() {
    let canvas = board();
    let opts = ImageOptions {
        zoom: 1.0,
        ..ImageOptions::default()
    };
    assert_eq!(estimated_size(&canvas, &opts), (2100, 1400));
    let opts = ImageOptions {
        zoom: -1.0,
        ..ImageOptions::default()
    };
    assert_eq!(estimated_size(&canvas, &opts), (525, 350));
}

#[test]
fn viewport_only_is_the_panes_area_without_alpha() {
    let canvas = board();
    let fonts = fonts();
    let text = Palette::light();
    let contents = contents();
    let palette = canvas_palette();
    let image = CanvasImage {
        canvas: &canvas,
        contents: &contents,
        palette: &palette,
        text: &text,
        fonts: &fonts,
        logo: LOGO,
    };
    let opts = ImageOptions {
        area: Area::Viewport {
            viewport: scoria_canvas::Viewport::default(),
            size: (870.0, 520.0),
        },
        show_logo: false,
        ..ImageOptions::default()
    };
    let (w, h, color, _) = decode(&export_image(&image, &opts).unwrap());
    assert_eq!((w, h, color), (870, 520, png::ColorType::Rgb));
}

#[test]
fn the_canvas_is_drawn_in_its_colours_and_privacy_hides_text_under_bars() {
    let canvas = board();
    let fonts = fonts();
    let text = Palette {
        text: Color::rgb(0xdadada),
        ..Palette::light()
    };
    let contents = contents();
    let palette = canvas_palette();
    let image = CanvasImage {
        canvas: &canvas,
        contents: &contents,
        palette: &palette,
        text: &text,
        fonts: &fonts,
        logo: LOGO,
    };
    let opts = ImageOptions::default();
    let pixmap = render_image(&image, &opts).unwrap();
    let at = |p: &tiny_skia::Pixmap, x: u32, y: u32| {
        let c = p.pixel(x, y).unwrap().demultiply();
        (c.red(), c.green(), c.blue())
    };
    // The ground, away from any dot.
    assert_eq!(at(&pixmap, 10, 10), (0x1e, 0x1e, 0x1e));
    // Inside card `b` (at 500,100 + the margin), tinted red over the card.
    let (r, g, b) = at(&pixmap, 650 + 50 + 100 - 10, 200 + 90);
    assert!(r > g && r > b, "{:?}", (r, g, b));
    // The logo strip has Scoria's orange in it.
    let mut orange = false;
    for y in 700..780 {
        for x in 0..200 {
            let (r, g, b) = at(&pixmap, x, y);
            if r > 200 && g < 120 && b < 60 {
                orange = true;
            }
        }
    }
    assert!(orange, "no mark in the strip");
    // Privacy: the text's glyph shapes become one solid bar, so the band
    // through a text card's first line has more text-coloured pixels.
    let count = |p: &tiny_skia::Pixmap| {
        let mut n = 0;
        for y in 100..140 {
            for x in 150..400 {
                let (r, _, _) = at(p, x, y);
                if r > 0xc0 {
                    n += 1;
                }
            }
        }
        n
    };
    let plain = count(&pixmap);
    let private = render_image(
        &image,
        &ImageOptions {
            privacy: true,
            ..opts
        },
    )
    .unwrap();
    assert!(plain > 0);
    assert!(count(&private) > plain, "{} vs {plain}", count(&private));
}
