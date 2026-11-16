//! Text measured as Pele measures it in Obsidian: shaped in the face the
//! diagram is drawn in. Pele asks the browser's canvas for each run's
//! width, and on every desktop Obsidian draws diagrams in Inter (its
//! `--pele-font` names the system faces first, but Chromium resolves none
//! of `ui-sans-serif`, `-apple-system`, `system-ui` there to anything ahead
//! of the Inter it bundles: the node boxes came out the same with the face
//! forced to Inter, 2026-11-15). Bold is Obsidian's `--bold-weight`, 600.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use limu::scene::text::{Face, Measurer};

/// The faces a diagram's text is measured in, as font files. The app hands
/// over the Inter it bundles; a face that is missing measures in the
/// regular one.
#[derive(Clone, Copy, Debug)]
pub struct Fonts {
    pub regular: &'static [u8],
    pub bold: Option<&'static [u8]>,
    pub italic: Option<&'static [u8]>,
    pub bold_italic: Option<&'static [u8]>,
    pub mono: Option<&'static [u8]>,
}

static FONTS: OnceLock<Fonts> = OnceLock::new();

/// Sets the faces every later layout measures in. The first call wins:
/// a layout already cached was measured in them.
pub fn set_fonts(fonts: Fonts) {
    let _ = FONTS.set(fonts);
}

pub(crate) fn fonts() -> Option<&'static Fonts> {
    FONTS.get()
}

/// Measures with rustybuzz over the faces, memoised per run.
pub(crate) struct Shaper {
    faces: [Option<rustybuzz::Face<'static>>; 5],
    memo: RefCell<HashMap<(String, u8, u64), f64>>,
}

impl Shaper {
    pub(crate) fn new(f: &Fonts) -> Shaper {
        let face = |b: Option<&'static [u8]>| b.and_then(|b| rustybuzz::Face::from_slice(b, 0));
        Shaper {
            faces: [
                face(Some(f.regular)),
                face(f.bold),
                face(f.italic),
                face(f.bold_italic),
                face(f.mono),
            ],
            memo: RefCell::default(),
        }
    }

    fn face(&self, f: Face) -> Option<&rustybuzz::Face<'static>> {
        let i = if f.contains(Face::MONO) {
            4
        } else {
            match (f.contains(Face::BOLD), f.contains(Face::ITALIC)) {
                (false, false) => 0,
                (true, false) => 1,
                (false, true) => 2,
                (true, true) => 3,
            }
        };
        self.faces[i]
            .as_ref()
            .or_else(|| {
                // Bold italic falls back to bold, then to the regular face.
                if i == 3 {
                    self.faces[1].as_ref()
                } else {
                    None
                }
            })
            .or(self.faces[0].as_ref())
    }

    /// The advance of `s` at 1 px, in the face for `f`.
    fn unit_width(&self, s: &str, f: Face) -> f64 {
        let Some(face) = self.face(f) else {
            return 0.0;
        };
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(s);
        let glyphs = rustybuzz::shape(face, &[], buf);
        let units: i64 = glyphs
            .glyph_positions()
            .iter()
            .map(|p| i64::from(p.x_advance))
            .sum();
        units as f64 / f64::from(face.units_per_em())
    }
}

impl Measurer for Shaper {
    fn width(&self, s: &str, size: f64, f: Face) -> f64 {
        if s.is_empty() {
            return 0.0;
        }
        let bits = f.bits() & (Face::BOLD | Face::ITALIC | Face::MONO).bits();
        let key = (s.to_string(), bits, 0);
        if let Some(w) = self.memo.borrow().get(&key) {
            return w * size;
        }
        let w = self.unit_width(s, f);
        self.memo.borrow_mut().insert(key, w);
        w * size
    }
}
