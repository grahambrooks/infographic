//! The one pinned font. The same bytes are used to measure text at layout
//! time, embedded (subset) into the SVG for browsers, and loaded into usvg for
//! rendering and checks, so every consumer lays text out identically.

use std::collections::{BTreeMap, HashSet};

use anyhow::{Context, Result};
use base64::Engine;

pub const FAMILY: &str = "IBM Plex Sans";
const REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Weight {
    Regular,
    SemiBold,
}

impl Weight {
    pub fn css(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::SemiBold => 600,
        }
    }

    fn data(self) -> &'static [u8] {
        match self {
            Weight::Regular => REGULAR,
            Weight::SemiBold => SEMIBOLD,
        }
    }
}

pub struct Fonts {
    regular: rustybuzz::Face<'static>,
    semibold: rustybuzz::Face<'static>,
}

impl Fonts {
    pub fn new() -> Self {
        let load =
            |w: Weight| rustybuzz::Face::from_slice(w.data(), 0).expect("bundled font parses");
        Self {
            regular: load(Weight::Regular),
            semibold: load(Weight::SemiBold),
        }
    }

    fn face(&self, w: Weight) -> &rustybuzz::Face<'static> {
        match w {
            Weight::Regular => &self.regular,
            Weight::SemiBold => &self.semibold,
        }
    }

    /// Shaped advance width in px, including kerning.
    pub fn width(&self, text: &str, size: f64, w: Weight) -> f64 {
        let face = self.face(w);
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(text);
        let glyphs = rustybuzz::shape(face, &[], buf);
        let units: i64 = glyphs
            .glyph_positions()
            .iter()
            .map(|p| p.x_advance as i64)
            .sum();
        units as f64 * size / face.units_per_em() as f64
    }

    /// Height of capital letters in px, used to centre text on a line.
    pub fn cap_height(&self, size: f64, w: Weight) -> f64 {
        let face = self.face(w);
        let cap = face
            .capital_height()
            .unwrap_or((face.ascender() as f64 * 0.7) as i16);
        cap as f64 * size / face.units_per_em() as f64
    }

    pub fn descent(&self, size: f64, w: Weight) -> f64 {
        let face = self.face(w);
        -(face.descender() as f64) * size / face.units_per_em() as f64
    }
}

/// Loads the pinned fonts into a usvg font database, and maps the generic
/// families onto them so hand-written SVGs render with known metrics too.
pub fn fontdb() -> usvg::fontdb::Database {
    let mut db = usvg::fontdb::Database::new();
    db.load_font_data(REGULAR.to_vec());
    db.load_font_data(SEMIBOLD.to_vec());
    db.set_sans_serif_family(FAMILY);
    db
}

/// `@font-face` rules embedding only the glyphs actually used, so the SVG
/// works as a self-contained `<img>` (verified in spikes/embedding-modes).
pub fn font_face_css(used: &BTreeMap<Weight, HashSet<char>>) -> Result<String> {
    let mut css = String::new();
    for (w, chars) in used {
        let mut chars = chars.clone();
        chars.insert(' ');
        let subset = fontcull::subset_font_data(w.data(), &chars, &[])
            .map_err(|e| anyhow::anyhow!("{e:?}"))
            .with_context(|| format!("subsetting {FAMILY} {}", w.css()))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(subset);
        css.push_str(&format!(
            "@font-face{{font-family:'{FAMILY}';font-weight:{};src:url(data:font/ttf;base64,{b64}) format('truetype')}}",
            w.css()
        ));
    }
    Ok(css)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wider_text_measures_wider() {
        let f = Fonts::new();
        let a = f.width("CLOSED", 12.0, Weight::SemiBold);
        let b = f.width("HALF-OPEN", 12.0, Weight::SemiBold);
        assert!(a > 20.0 && b > a, "{a} {b}");
        assert!(f.width("abc", 12.0, Weight::SemiBold) > f.width("abc", 12.0, Weight::Regular));
    }

    #[test]
    fn subset_keeps_cmap_and_shrinks() {
        let mut used = BTreeMap::new();
        used.insert(
            Weight::Regular,
            "Circuit breaker".chars().collect::<HashSet<_>>(),
        );
        let css = font_face_css(&used).unwrap();
        assert!(css.len() < 60_000, "subset css is {} bytes", css.len());
        let b64 = css
            .split("base64,")
            .nth(1)
            .unwrap()
            .split(')')
            .next()
            .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
        assert!(face.glyph_index('C').is_some(), "cmap lost 'C'");
        assert!(face.glyph_index('Z').is_none(), "unused glyph kept");
    }
}
