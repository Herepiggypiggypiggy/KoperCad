//! Text that Helvetica's WinAnsi encoding cannot carry (Ω, Greek, ≤, CJK
//! where Inter has it), in an embedded subset of the Inter font novo-theme
//! bundles for the interface.
//!
//! A line that fits WinAnsi stays in Helvetica, which every reader has, so
//! most exports embed nothing. Any other line is set in Inter as a whole,
//! so one line never mixes two faces. Only the glyphs used are embedded,
//! as a CID font with a ToUnicode map, so the text can be searched and
//! copied. Characters Inter lacks print as its empty `.notdef` box.

use std::collections::BTreeMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, Str};
use subsetter::GlyphRemapper;
use ttf_parser::{Face, GlyphId};

/// Inter Regular, the interface font (SIL Open Font License, see
/// `crates/novo-theme/assets/fonts/Inter-LICENSE.txt`).
const INTER: &[u8] = include_bytes!("../../novo-theme/assets/fonts/Inter-Regular.ttf");

/// Name of the font resource on pages that use it.
pub const FONT_NAME: &[u8] = b"F2";

const IDENTITY: SystemInfo = SystemInfo {
    registry: Str(b"Adobe"),
    ordering: Str(b"Identity"),
    supplement: 0,
};

/// The Inter glyphs an export uses, numbered in order of first use.
pub struct Inter {
    face: Face<'static>,
    remapper: GlyphRemapper,
    /// Subset glyph id → width (1/1000 em) and the character it shows.
    glyphs: BTreeMap<u16, (f32, Option<char>)>,
}

impl Inter {
    pub fn new() -> Self {
        Self {
            face: Face::parse(INTER, 0).expect("the bundled Inter font parses"),
            remapper: GlyphRemapper::new(),
            glyphs: BTreeMap::new(),
        }
    }

    fn per_mille(&self, units: f32) -> f32 {
        units * 1000.0 / f32::from(self.face.units_per_em())
    }

    /// One line as two-byte subset glyph ids, and its width in units of
    /// the font size.
    pub fn encode(&mut self, line: &str) -> (Vec<u8>, f32) {
        let mut bytes = Vec::with_capacity(line.len() * 2);
        let mut width = 0.0;
        for c in line.chars() {
            let c = if c == '\t' { ' ' } else { c };
            let gid = self.face.glyph_index(c).unwrap_or(GlyphId(0));
            let advance = self.face.glyph_hor_advance(gid).unwrap_or(0);
            let w = self.per_mille(f32::from(advance));
            let new = self.remapper.remap(gid.0);
            self.glyphs
                .entry(new)
                .or_insert((w, (gid.0 != 0).then_some(c)));
            bytes.extend(new.to_be_bytes());
            width += w;
        }
        (bytes, width / 1000.0)
    }

    /// Writes the font (Type 0 at `font`, with its CID font, descriptor,
    /// subset file and ToUnicode map at fresh refs from `alloc`).
    pub fn write(&self, pdf: &mut Pdf, font: Ref, mut alloc: impl FnMut() -> Ref) {
        let (cid, descriptor, file, to_unicode) = (alloc(), alloc(), alloc(), alloc());
        // A subset is named with a tag unique to its glyphs (PDF 9.6.4),
        // derived from them so exports stay reproducible.
        let mut hash: u32 = 0x811c_9dc5;
        for gid in self.remapper.remapped_gids() {
            for b in gid.to_be_bytes() {
                hash = (hash ^ u32::from(b)).wrapping_mul(0x0100_0193);
            }
        }
        let tag: String = (0..6)
            .map(|i| char::from(b'A' + ((hash >> (i * 5)) % 26) as u8))
            .collect();
        let base = format!("{tag}+Inter-Regular");

        pdf.type0_font(font)
            .base_font(Name(base.as_bytes()))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid)
            .to_unicode(to_unicode);

        let mut cid_font = pdf.cid_font(cid);
        cid_font
            .subtype(CidFontType::Type2)
            .base_font(Name(base.as_bytes()))
            .system_info(IDENTITY)
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        let mut widths = cid_font.widths();
        for (gid, (w, _)) in &self.glyphs {
            widths.consecutive(*gid, [*w]);
        }
        widths.finish();
        cid_font.finish();

        let b = self.face.global_bounding_box();
        let s = |v: i16| self.per_mille(f32::from(v));
        pdf.font_descriptor(descriptor)
            .name(Name(base.as_bytes()))
            .flags(FontFlags::NON_SYMBOLIC)
            .bbox(Rect::new(s(b.x_min), s(b.y_min), s(b.x_max), s(b.y_max)))
            .italic_angle(0.0)
            .ascent(s(self.face.ascender()))
            .descent(s(self.face.descender()))
            .cap_height(s(self.face.capital_height().unwrap_or(1490)))
            .stem_v(80.0)
            .font_file2(file);

        let data =
            subsetter::subset(INTER, 0, &self.remapper).expect("the bundled Inter font subsets");
        pdf.stream(file, &data)
            .pair(Name(b"Length1"), data.len() as i32);

        let mut cmap = UnicodeCmap::new(Name(b"Custom"), IDENTITY);
        for (gid, (_, c)) in &self.glyphs {
            if let Some(c) = c {
                cmap.pair(*gid, *c);
            }
        }
        pdf.cmap(to_unicode, &cmap.finish());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inter_has_the_symbols_electrical_drawings_use() {
        let mut inter = Inter::new();
        for c in ['Ω', 'µ', '≤', '≥', 'φ', 'Δ', '√', '→'] {
            let (bytes, width) = inter.encode(&c.to_string());
            assert_ne!(bytes, [0, 0], "{c} missing from Inter");
            assert!(width > 0.0, "{c}");
        }
    }

    #[test]
    fn glyphs_are_numbered_in_order_of_first_use() {
        let mut inter = Inter::new();
        let (a, _) = inter.encode("Ωa");
        let (b, _) = inter.encode("aΩ");
        assert_eq!(a, [0, 1, 0, 2]);
        assert_eq!(b, [0, 2, 0, 1]);
    }
}
