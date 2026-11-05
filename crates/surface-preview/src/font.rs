//! The kernel console's font, as a glyph source for the surface.

use noto_sans_mono_bitmap::{FontWeight, RasterHeight, get_raster, get_raster_width};
use surface::{Glyph, GlyphSource};

const WEIGHT: FontWeight = FontWeight::Regular;
const HEIGHT: RasterHeight = RasterHeight::Size16;

/// Noto Sans Mono, regular, 16 pixels: what the Bedrock boot console draws.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotoMono;

impl NotoMono {
    /// Pixels from the top of one text line to the top of the next, without gap.
    pub const LINE_HEIGHT: usize = HEIGHT.val();
}

impl GlyphSource for NotoMono {
    fn cell_width(&self) -> usize {
        get_raster_width(WEIGHT, HEIGHT)
    }

    fn glyph(&self, character: char) -> Option<Glyph<'_>> {
        let raster = get_raster(character, WEIGHT, HEIGHT)?;
        Glyph::new(raster.width(), raster.raster())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_has_letters_and_every_glyph_is_one_cell_wide() {
        let font = NotoMono;
        for character in ['A', 'g', '0', '~', ' '] {
            let glyph = font.glyph(character).expect("basic latin is built in");
            assert_eq!(glyph.width(), font.cell_width());
            assert_eq!(glyph.height(), NotoMono::LINE_HEIGHT);
        }
    }

    #[test]
    fn a_letter_has_some_ink_and_a_space_has_none() {
        let ink = |character| {
            let glyph = NotoMono.glyph(character).unwrap();
            (0..glyph.height())
                .flat_map(|y| (0..glyph.width()).map(move |x| (x, y)))
                .map(|(x, y)| u32::from(glyph.coverage(x, y).unwrap()))
                .sum::<u32>()
        };
        assert!(ink('W') > 0);
        assert_eq!(ink(' '), 0);
    }
}
