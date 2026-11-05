//! The drawing surface itself.

use core::fmt;

use pixel::{FramebufferLayout, FramebufferLayoutError, PixelFormat, Rgb};

use crate::{Glyph, GlyphSource, Rect};

/// Bytes in one pixel word.
const WORD: usize = size_of::<u32>();

/// Why a buffer cannot be drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    /// The width, height or pitch does not describe a valid framebuffer.
    Layout(FramebufferLayoutError),
    /// The buffer holds fewer pixel words than the layout covers.
    BufferTooSmall {
        /// Words the layout covers.
        needed: usize,
        /// Words in the buffer.
        have: usize,
    },
    /// A side is longer than `i32::MAX` pixels, which rectangles cannot address.
    TooLarge,
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => write!(f, "invalid surface layout: {error}"),
            Self::BufferTooSmall { needed, have } => {
                write!(
                    f,
                    "surface needs {needed} pixel words but the buffer has {have}"
                )
            }
            Self::TooLarge => f.write_str("surface is larger than i32::MAX pixels on a side"),
        }
    }
}

impl core::error::Error for SurfaceError {}

/// A finished picture to copy onto a surface, in the surface's own pixel format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image<'a> {
    words: &'a [u32],
    width: usize,
    height: usize,
}

impl<'a> Image<'a> {
    /// An image of `width` by `height` packed pixels with no padding between
    /// rows, or `None` if `words` is not exactly that long.
    pub fn new(words: &'a [u32], width: usize, height: usize) -> Option<Self> {
        (width.checked_mul(height)? == words.len()).then_some(Self {
            words,
            width,
            height,
        })
    }

    /// Pixels across.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Pixels down.
    pub const fn height(self) -> usize {
        self.height
    }
}

/// Where one colour channel sits in a packed word.
#[derive(Clone, Copy, Debug)]
struct Field {
    mask: u32,
    shift: u32,
}

impl Field {
    /// Find a channel by packing its full-scale colour: the bits that light up
    /// are the channel. `PixelFormat` only packs, so this is how the surface
    /// learns to read pixels back for blending.
    fn probe(format: PixelFormat, full_scale: Rgb) -> Self {
        let mask = format.pack(full_scale);
        let shift = if mask == 0 { 0 } else { mask.trailing_zeros() };
        Self { mask, shift }
    }

    /// The channel's intensity in `word` as eight bits: the smallest intensity
    /// that `pixel` packs to this value, so that packing what was read gives
    /// back the word. Rounding to the nearest instead would make every
    /// translucent fill on a narrow-channel surface nudge the colour it covers.
    fn read(self, word: u32) -> u8 {
        let full_scale = u64::from(self.mask >> self.shift);
        if full_scale == 0 {
            return 0;
        }
        let value = u64::from((word & self.mask) >> self.shift);
        ((value * 255).div_ceil(full_scale)) as u8
    }
}

/// A buffer of 32-bit pixels that can be drawn on.
///
/// Every drawing call is cut off at the clip rectangle, and the clip rectangle
/// can never reach beyond the surface, so no call can write outside the
/// visible pixels. Padding at the end of each row is never touched.
pub struct Surface<'a> {
    words: &'a mut [u32],
    format: PixelFormat,
    red: Field,
    green: Field,
    blue: Field,
    width: usize,
    height: usize,
    stride: usize,
    clip: Rect,
}

impl fmt::Debug for Surface<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Surface")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("clip", &self.clip)
            .finish_non_exhaustive()
    }
}

impl<'a> Surface<'a> {
    /// Draw on `words`, laid out as `height` rows of `width` pixels with
    /// `pitch` bytes from the start of one row to the start of the next.
    ///
    /// # Errors
    ///
    /// [`SurfaceError::Layout`] if the geometry is not a valid framebuffer
    /// (zero sized, a row wider than the pitch, or arithmetic that wraps),
    /// [`SurfaceError::TooLarge`] if a side exceeds `i32::MAX`, or
    /// [`SurfaceError::BufferTooSmall`] if `words` does not cover the layout.
    pub fn new(
        words: &'a mut [u32],
        format: PixelFormat,
        width: usize,
        height: usize,
        pitch: usize,
    ) -> Result<Self, SurfaceError> {
        let layout =
            FramebufferLayout::new(0, width, height, pitch, WORD).map_err(SurfaceError::Layout)?;
        if i32::try_from(width).is_err() || i32::try_from(height).is_err() {
            return Err(SurfaceError::TooLarge);
        }
        let needed = layout.span() / WORD;
        if words.len() < needed {
            return Err(SurfaceError::BufferTooSmall {
                needed,
                have: words.len(),
            });
        }
        let probe = |red, green, blue| Field::probe(format, Rgb { red, green, blue });
        Ok(Self {
            words,
            format,
            red: probe(255, 0, 0),
            green: probe(0, 255, 0),
            blue: probe(0, 0, 255),
            width,
            height,
            stride: pitch / WORD,
            clip: Rect::new(0, 0, width as u32, height as u32),
        })
    }

    /// Pixels across.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Pixels down.
    pub const fn height(&self) -> usize {
        self.height
    }

    /// The whole visible surface.
    pub const fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width as u32, self.height as u32)
    }

    /// The rectangle drawing is currently cut off at.
    pub const fn clip(&self) -> Rect {
        self.clip
    }

    /// Cut drawing off at `clip`, or at the part of it that is on the surface.
    pub fn set_clip(&mut self, clip: Rect) {
        self.clip = clip.intersect(self.bounds());
    }

    /// Allow drawing anywhere on the surface again.
    pub fn reset_clip(&mut self) {
        self.clip = self.bounds();
    }

    /// The packed word at `x`, `y`, or `None` off the surface.
    pub fn word(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.words.get(y * self.stride + x).copied()
    }

    /// The colour at `x`, `y`, or `None` off the surface.
    ///
    /// A channel narrower than eight bits comes back rounded to what it can
    /// hold, so a colour written to a 5-6-5 surface is not always the colour read.
    pub fn pixel(&self, x: usize, y: usize) -> Option<Rgb> {
        self.word(x, y).map(|word| self.unpack(word))
    }

    /// Paint the whole clip rectangle one colour.
    pub fn clear(&mut self, colour: Rgb) {
        self.fill_rect(self.bounds(), colour);
    }

    /// Paint one pixel, if it is inside the clip rectangle.
    pub fn put_pixel(&mut self, x: i32, y: i32, colour: Rgb) {
        self.fill_rect(Rect::new(x, y, 1, 1), colour);
    }

    /// Paint a rectangle one colour.
    pub fn fill_rect(&mut self, rect: Rect, colour: Rgb) {
        let Some((left, top, right, bottom)) = self.visible(rect) else {
            return;
        };
        let word = self.format.pack(colour);
        for y in top..bottom {
            let start = y * self.stride;
            self.words[start + left..start + right].fill(word);
        }
    }

    /// Lay a colour over a rectangle, `alpha` out of 255 opaque.
    pub fn blend_rect(&mut self, rect: Rect, colour: Rgb, alpha: u8) {
        let Some((left, top, right, bottom)) = self.visible(rect) else {
            return;
        };
        for y in top..bottom {
            for x in left..right {
                self.paint(y * self.stride + x, colour, alpha);
            }
        }
    }

    /// Draw the edge of a rectangle `thickness` pixels wide, inward from its
    /// outline. Once the edges would meet, the whole rectangle is painted, and
    /// nothing is ever painted outside the rectangle.
    pub fn stroke_rect(&mut self, rect: Rect, thickness: u32, colour: Rgb) {
        let t = i64::from(thickness);
        if 2 * t >= i64::from(rect.width) || 2 * t >= i64::from(rect.height) {
            self.fill_rect(rect, colour);
            return;
        }
        let (left, top) = (i64::from(rect.x), i64::from(rect.y));
        let (right, bottom) = (rect.right(), rect.bottom());
        self.fill_rect(Rect::from_edges(left, top, right, top + t), colour);
        self.fill_rect(Rect::from_edges(left, bottom - t, right, bottom), colour);
        self.fill_rect(
            Rect::from_edges(left, top + t, left + t, bottom - t),
            colour,
        );
        self.fill_rect(
            Rect::from_edges(right - t, top + t, right, bottom - t),
            colour,
        );
    }

    /// Copy `image` with its top left corner at `x`, `y`.
    pub fn blit(&mut self, image: Image<'_>, x: i32, y: i32) {
        let size = |side: usize| u32::try_from(side).unwrap_or(u32::MAX);
        let target = Rect::new(x, y, size(image.width), size(image.height));
        let Some((left, top, right, bottom)) = self.visible(target) else {
            return;
        };
        let count = right - left;
        let source_left = (left as i64 - i64::from(x)) as usize;
        for row in top..bottom {
            let source_row = (row as i64 - i64::from(y)) as usize;
            let from = source_row * image.width + source_left;
            let to = row * self.stride + left;
            self.words[to..to + count].copy_from_slice(&image.words[from..from + count]);
        }
    }

    /// Draw one glyph with its top left corner at `x`, `y`.
    pub fn draw_glyph(&mut self, x: i32, y: i32, glyph: Glyph<'_>, colour: Rgb) {
        let size = |side: usize| u32::try_from(side).unwrap_or(u32::MAX);
        let target = Rect::new(x, y, size(glyph.width()), size(glyph.height()));
        let Some((left, top, right, bottom)) = self.visible(target) else {
            return;
        };
        for row in top..bottom {
            for column in left..right {
                let glyph_x = (column as i64 - i64::from(x)) as usize;
                let glyph_y = (row as i64 - i64::from(y)) as usize;
                if let Some(ink) = glyph.coverage(glyph_x, glyph_y) {
                    self.paint(row * self.stride + column, colour, ink);
                }
            }
        }
    }

    /// Draw `text` with its first cell at `x`, `y`, and return the column
    /// where the next cell would start. A character the font lacks leaves a
    /// blank cell.
    pub fn draw_text(
        &mut self,
        x: i32,
        y: i32,
        font: &impl GlyphSource,
        text: &str,
        colour: Rgb,
    ) -> i64 {
        let mut pen = i64::from(x);
        for character in text.chars() {
            if let (Some(glyph), Ok(at)) = (font.glyph(character), i32::try_from(pen)) {
                self.draw_glyph(at, y, glyph, colour);
            }
            pen = pen.saturating_add(i64::try_from(font.cell_width()).unwrap_or(i64::MAX));
        }
        pen
    }

    /// The part of `rect` inside the clip rectangle as `left, top, right,
    /// bottom` pixel indices, or `None` if nothing of it is.
    fn visible(&self, rect: Rect) -> Option<(usize, usize, usize, usize)> {
        let rect = rect.intersect(self.clip);
        if rect.is_empty() {
            return None;
        }
        Some((
            rect.x as usize,
            rect.y as usize,
            rect.right() as usize,
            rect.bottom() as usize,
        ))
    }

    /// Lay `colour` over the word at `index`, `alpha` out of 255 opaque.
    fn paint(&mut self, index: usize, colour: Rgb, alpha: u8) {
        match alpha {
            0 => {}
            255 => self.words[index] = self.format.pack(colour),
            _ => {
                let under = self.unpack(self.words[index]);
                self.words[index] = self.format.pack(under.blend(colour, alpha));
            }
        }
    }

    fn unpack(&self, word: u32) -> Rgb {
        Rgb {
            red: self.red.read(word),
            green: self.green.read(word),
            blue: self.blue.read(word),
        }
    }
}
