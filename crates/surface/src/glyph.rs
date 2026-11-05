//! Text, as the surface needs it: a coverage mask per character.

/// One character as rows of coverage: how much of each pixel is ink.
///
/// Rows are separate slices because that is how the font crates the kernel
/// already uses hand them out, so a glyph needs no copy and no allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph<'a> {
    width: usize,
    rows: &'a [&'a [u8]],
}

impl<'a> Glyph<'a> {
    /// A glyph `width` pixels across, or `None` if any row is not exactly
    /// that long.
    pub fn new(width: usize, rows: &'a [&'a [u8]]) -> Option<Self> {
        rows.iter()
            .all(|row| row.len() == width)
            .then_some(Self { width, rows })
    }

    /// Pixels across.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Pixels down.
    pub const fn height(self) -> usize {
        self.rows.len()
    }

    /// The ink at column `x` of row `y`, from 0 (none) to 255 (solid), or
    /// `None` outside the glyph.
    pub fn coverage(self, x: usize, y: usize) -> Option<u8> {
        self.rows.get(y)?.get(x).copied()
    }
}

/// Where glyphs come from. The surface does not own a font, so the font that
/// the kernel console uses and the one a host preview uses can both plug in.
pub trait GlyphSource {
    /// Pixels the pen advances after each character.
    fn cell_width(&self) -> usize;

    /// The glyph for `character`, or `None` if the font has nothing for it.
    fn glyph(&self, character: char) -> Option<Glyph<'_>>;
}
