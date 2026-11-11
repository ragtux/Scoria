//! The faces an export sets its text in, and the measurements the layout
//! takes from them.
//!
//! The text faces are the app's own Inter (handed in as bytes, so this
//! crate carries no copy of them); code is set in KaTeX's Typewriter, the
//! one monospaced face the workspace already embeds (through
//! `scoria-math`), with Inter standing in for whatever it lacks; math is
//! set in the KaTeX faces its metrics belong to. Advances come from
//! `ab_glyph`, and both backends draw from the same glyph ids, so the PDF
//! and the PNG place every glyph where the layout measured it.

use ab_glyph::{Font, FontRef, GlyphId};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The bytes of the faces the app bundles. Every slice is a TrueType file.
#[derive(Clone, Copy)]
pub struct FontFiles {
    pub regular: &'static [u8],
    pub italic: &'static [u8],
    pub bold: &'static [u8],
    pub bold_italic: &'static [u8],
    pub semibold: &'static [u8],
    /// The face the logo strip's product name is set in.
    pub display: &'static [u8],
}

/// A face, by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Italic,
    Bold,
    BoldItalic,
    SemiBold,
    Display,
    /// Code: KaTeX Typewriter.
    Mono,
    /// One of the KaTeX files, by file name.
    Math(&'static str),
}

impl Face {
    /// The text face for a weight and a slant.
    pub fn text(bold: bool, italic: bool) -> Face {
        match (bold, italic) {
            (false, false) => Face::Regular,
            (false, true) => Face::Italic,
            (true, false) => Face::Bold,
            (true, true) => Face::BoldItalic,
        }
    }
}

/// The KaTeX files, read once and kept for the life of the process: the
/// embedding crate hands them out as `Cow`s, borrowed in a release build and
/// owned when read from disk.
fn katex(file: &str) -> Option<&'static [u8]> {
    static FILES: OnceLock<HashMap<&'static str, &'static [u8]>> = OnceLock::new();
    let files = FILES.get_or_init(|| {
        let mut map = HashMap::new();
        // `fonts()` is in `FONT_FILES` order, skipping a file it cannot
        // find; every file is embedded, so a short list means none line up.
        let all = scoria_math::fonts();
        if all.len() == scoria_math::FONT_FILES.len() {
            for (name, bytes) in scoria_math::FONT_FILES.into_iter().zip(all) {
                let bytes: &'static [u8] = match bytes {
                    std::borrow::Cow::Borrowed(b) => b,
                    std::borrow::Cow::Owned(v) => Box::leak(v.into_boxed_slice()),
                };
                map.insert(name, bytes);
            }
        }
        map
    });
    files.get(file).copied()
}

/// The faces of one export, parsed.
pub struct Fonts {
    files: FontFiles,
    parsed: HashMap<Face, FontRef<'static>>,
}

impl Fonts {
    pub fn new(files: FontFiles) -> Fonts {
        let mut parsed = HashMap::new();
        for (face, bytes) in [
            (Face::Regular, files.regular),
            (Face::Italic, files.italic),
            (Face::Bold, files.bold),
            (Face::BoldItalic, files.bold_italic),
            (Face::SemiBold, files.semibold),
            (Face::Display, files.display),
        ] {
            if let Ok(font) = FontRef::try_from_slice(bytes) {
                parsed.insert(face, font);
            }
        }
        if let Some(bytes) = katex("KaTeX_Typewriter-Regular.ttf") {
            if let Ok(font) = FontRef::try_from_slice(bytes) {
                parsed.insert(Face::Mono, font);
            }
        }
        Fonts { files, parsed }
    }

    /// The file behind a face.
    pub fn bytes(&self, face: Face) -> Option<&'static [u8]> {
        match face {
            Face::Regular => Some(self.files.regular),
            Face::Italic => Some(self.files.italic),
            Face::Bold => Some(self.files.bold),
            Face::BoldItalic => Some(self.files.bold_italic),
            Face::SemiBold => Some(self.files.semibold),
            Face::Display => Some(self.files.display),
            Face::Mono => katex("KaTeX_Typewriter-Regular.ttf"),
            Face::Math(file) => katex(file),
        }
    }

    /// The parsed face. The math faces are parsed when first asked for.
    pub fn font(&self, face: Face) -> Option<FontRef<'static>> {
        if let Some(f) = self.parsed.get(&face) {
            return Some(f.clone());
        }
        match face {
            Face::Math(file) => katex(file).and_then(|b| FontRef::try_from_slice(b).ok()),
            _ => None,
        }
    }

    /// The face `ch` is drawn from when `face` is wanted: `face` when it has
    /// the glyph, else the regular text face (Inter's coverage is the widest
    /// the app carries), else `face` anyway, drawing its missing-glyph box.
    pub fn resolve(&self, face: Face, ch: char) -> (Face, u16) {
        if let Some(font) = self.font(face) {
            let id = font.glyph_id(ch);
            if id.0 != 0 {
                return (face, id.0);
            }
        }
        // A mono or math face falls back to the upright, an italic or bold
        // one keeps its look as near as Inter has it.
        let fallback = match face {
            Face::Mono | Face::Math(_) | Face::Display => Face::Regular,
            other => other,
        };
        if fallback != face {
            if let Some(font) = self.font(fallback) {
                let id = font.glyph_id(ch);
                if id.0 != 0 {
                    return (fallback, id.0);
                }
            }
        }
        (face, 0)
    }

    /// A glyph's advance in em.
    pub fn advance(&self, face: Face, glyph: u16) -> f32 {
        match self.font(face) {
            Some(font) => {
                let upem = font.units_per_em().unwrap_or(1000.0);
                font.h_advance_unscaled(GlyphId(glyph)) / upem
            }
            None => 0.5,
        }
    }

    /// The kerning between two glyphs of one face, in em.
    pub fn kern(&self, face: Face, a: u16, b: u16) -> f32 {
        match self.font(face) {
            Some(font) => {
                let upem = font.units_per_em().unwrap_or(1000.0);
                font.kern_unscaled(GlyphId(a), GlyphId(b)) / upem
            }
            None => 0.0,
        }
    }

    /// The face's ascent and descent in em, both positive.
    pub fn vertical(&self, face: Face) -> (f32, f32) {
        match self.font(face) {
            Some(font) => {
                let upem = font.units_per_em().unwrap_or(1000.0);
                (
                    font.ascent_unscaled() / upem,
                    -font.descent_unscaled() / upem,
                )
            }
            None => (0.8, 0.2),
        }
    }
}
