//! "Adapt to theme": a page recoloured for a dark theme.
//!
//! Observed in Obsidian 1.15.0's dark theme
//! (`tools/obsidian-probe/results/2026-11-15-pdf-selection.json`, s07, the
//! page canvas read back pixel by pixel): the page's own ground becomes the
//! view's `--pdf-background` (`#1C1C1C`); every colour the content draws
//! keeps its hue and saturation and has its HSL lightness turned over —
//! black text 255, grey `#666666` 153, the `#808080` fill 127, pure red and
//! blue unchanged, `#008000` green `(127, 255, 127)`; and an image is left
//! as it is. Turning lightness over with hue and saturation kept is adding
//! `1 - max - min` to every channel, which is what [`invert_lightness`] does.
//! In the light theme the toggle changes nothing.

use hayro::hayro_interpret::hayro_syntax::page::Page;
use hayro::hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image,
    ImageDrawProps, InterpreterCache, InterpreterSettings, SoftMask,
};
use hayro::kurbo::{Affine, BezPath, Point, Rect};
use hayro::vello_cpu::Pixmap;

use crate::Bitmap;

/// How a page is recoloured: the ground it is drawn on, as straight RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Theming {
    pub background: [u8; 3],
}

/// One colour with its HSL lightness turned over, hue and saturation kept.
pub fn invert_lightness(rgb: [u8; 3]) -> [u8; 3] {
    let max = i32::from(*rgb.iter().max().unwrap_or(&0));
    let min = i32::from(*rgb.iter().min().unwrap_or(&0));
    let shift = 255 - max - min;
    rgb.map(|c| (i32::from(c) + shift).clamp(0, 255) as u8)
}

#[derive(Default)]
struct ImageDevice {
    areas: Vec<[f32; 4]>,
}

impl<'a> Device<'a> for ImageDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph_run(
        &mut self,
        _: &hayro::hayro_interpret::font::GlyphRun<'_, 'a>,
        _: DrawProps<'a>,
        _: &DrawMode,
    ) {
    }
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_image(&mut self, _: Image<'a, '_>, props: ImageDrawProps<'a>) {
        let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .map(|(x, y)| props.transform * Point::new(x, y));
        let xs = corners.map(|p| p.x as f32);
        let ys = corners.map(|p| p.y as f32);
        let fold = |v: [f32; 4], f: fn(f32, f32) -> f32, init: f32| v.into_iter().fold(init, f);
        self.areas.push([
            fold(xs, f32::min, f32::MAX),
            fold(ys, f32::min, f32::MAX),
            fold(xs, f32::max, f32::MIN),
            fold(ys, f32::max, f32::MIN),
        ]);
    }
}

/// Where the page draws images, as pixel boxes `[x0, y0, x1, y1]` under
/// `transform` (page space to pixels).
pub(crate) fn image_areas(
    page: &Page<'_>,
    settings: &InterpreterSettings,
    transform: Affine,
    width: u32,
    height: u32,
) -> Vec<[f32; 4]> {
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        transform,
        Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
        &cache,
        page.xref(),
        settings.clone(),
    );
    let mut device = ImageDevice::default();
    interpret_page(page, &mut context, &mut device);
    device.areas
}

/// A page rendered over transparency, recoloured and laid on the ground.
pub(crate) fn recolour(
    pixmap: &Pixmap,
    width: u32,
    height: u32,
    theming: Theming,
    images: &[[f32; 4]],
) -> Bitmap {
    let bg = theming.background.map(u32::from);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for (ix, pixel) in pixmap.data().iter().enumerate() {
        let a = u32::from(pixel.a);
        if a == 0 {
            rgba.extend_from_slice(&[bg[0] as u8, bg[1] as u8, bg[2] as u8, 255]);
            continue;
        }
        let straight =
            [pixel.r, pixel.g, pixel.b].map(|c| ((u32::from(c) * 255 + a / 2) / a).min(255) as u8);
        let (x, y) = (
            (ix as u32 % width) as f32 + 0.5,
            (ix as u32 / width) as f32 + 0.5,
        );
        let in_image = images
            .iter()
            .any(|b| x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3]);
        let colour = if in_image {
            straight
        } else {
            invert_lightness(straight)
        };
        let mix = |c: u8, g: u32| ((u32::from(c) * a + g * (255 - a) + 127) / 255) as u8;
        rgba.extend_from_slice(&[
            mix(colour[0], bg[0]),
            mix(colour[1], bg[1]),
            mix(colour[2], bg[2]),
            255,
        ]);
    }
    Bitmap {
        width,
        height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::invert_lightness;

    #[test]
    fn lightness_turns_over_as_observed() {
        assert_eq!(invert_lightness([0, 0, 0]), [255, 255, 255]);
        assert_eq!(invert_lightness([102, 102, 102]), [153, 153, 153]);
        assert_eq!(invert_lightness([128, 128, 128]), [127, 127, 127]);
        assert_eq!(invert_lightness([255, 0, 0]), [255, 0, 0]);
        assert_eq!(invert_lightness([0, 0, 255]), [0, 0, 255]);
        assert_eq!(invert_lightness([0, 128, 0]), [127, 255, 127]);
    }
}
