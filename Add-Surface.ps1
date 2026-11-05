<#
.SYNOPSIS
  Adds the `surface` and `surface-preview` crates to a Forge.OS workspace.

.DESCRIPTION
  Writes 13 files under crates\surface and crates\surface-preview and adds both crates
  to the workspace Cargo.toml (members, default-members, workspace dependencies).
  It changes nothing else, and it refuses to overwrite an existing crates\surface unless -Force.

.EXAMPLE
  pwsh -File .\Add-Surface.ps1
  pwsh -File .\Add-Surface.ps1 -ForgeRoot 'E:\Forge.OS'
#>
param(
  [string]$ForgeRoot = 'E:\Forge.OS',
  [switch]$Force
)
$ErrorActionPreference = 'Stop'

$manifest = Join-Path $ForgeRoot 'Cargo.toml'
if (-not (Test-Path $manifest)) { throw "No Cargo.toml in $ForgeRoot - is that the Forge.OS workspace?" }
if ((Test-Path (Join-Path $ForgeRoot 'crates\surface')) -and -not $Force) {
  throw "crates\surface already exists. Re-run with -Force to overwrite the files this script writes."
}

$utf8 = [System.Text.UTF8Encoding]::new($false)
function Write-Source([string]$Relative, [string]$Text) {
  $path = Join-Path $ForgeRoot $Relative
  New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
  # Unix line endings, as rustfmt expects.
  [System.IO.File]::WriteAllText($path, (($Text -replace "`r`n", "`n") + "`n"), $utf8)
  Write-Host "  wrote $Relative"
}

Write-Host "Writing the crates..."
Write-Source 'crates\surface\Cargo.toml' @'
[package]
name = "surface"
description = "A pixel surface to draw on: rectangles, clipping, blending, blits and text. Pure logic, host-testable."
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
pixel.workspace = true
'@
Write-Source 'crates\surface\src\lib.rs' @'
//! A pixel surface to draw on.
//!
//! A [`Surface`] borrows a buffer of 32-bit pixels and draws into it: filled
//! and outlined rectangles, translucent fills, blits and text, all kept inside
//! a clip rectangle. It owns nothing and allocates nothing, so the same code
//! draws into the kernel's framebuffer, a shared-memory window buffer or a
//! `Vec` on the host, where the result can be saved as a picture and compared
//! with a golden one.
//!
//! Colour and pixel packing are Bedrock's own [`pixel`] types, so the surface
//! never disagrees with the boot console about which bits are red.
//!
//! # Examples
//!
//! ```
//! use pixel::{Channel, PixelFormat, Rgb};
//! use surface::{Rect, Surface};
//!
//! let format = PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8))
//!     .expect("valid channel masks");
//! let mut words = [0u32; 4 * 3];
//! let mut surface = Surface::new(&mut words, format, 4, 3, 16).expect("a buffer that fits");
//! surface.fill_rect(Rect::new(1, 1, 2, 1), Rgb::from_hex(0xff0000));
//! assert_eq!(surface.word(1, 1), Some(0x00ff_0000));
//! assert_eq!(surface.word(0, 0), Some(0));
//! ```

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, unreachable_pub)]

mod glyph;
mod rect;
mod surface;

pub use glyph::{Glyph, GlyphSource};
pub use rect::Rect;
pub use surface::{Image, Surface, SurfaceError};
'@
Write-Source 'crates\surface\src\rect.rs' @'
//! Rectangles in surface coordinates.

/// A rectangle whose corner may lie off the surface on any side.
///
/// The origin is signed so a window can be dragged partly off screen; the size
/// is unsigned because a rectangle cannot have a negative extent. All edge
/// arithmetic is done in 64 bits, so no rectangle can overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Rect {
    /// The rectangle with no area, which is what two disjoint rectangles meet in.
    pub const EMPTY: Self = Self::new(0, 0, 0, 0);

    /// A rectangle with its top left corner at `x`, `y`.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The rectangle between four edges, or [`Rect::EMPTY`] if they enclose nothing.
    ///
    /// Edges beyond the `i32` range are pulled in to it. Nothing that far out
    /// is on any surface, so the part that matters is unchanged.
    pub const fn from_edges(left: i64, top: i64, right: i64, bottom: i64) -> Self {
        let left = clamp(left);
        let top = clamp(top);
        let right = clamp(right);
        let bottom = clamp(bottom);
        if right <= left || bottom <= top {
            return Self::EMPTY;
        }
        Self::new(
            left as i32,
            top as i32,
            (right - left) as u32,
            (bottom - top) as u32,
        )
    }

    /// Whether the rectangle covers no pixel.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The first column to the right of the rectangle.
    pub const fn right(self) -> i64 {
        self.x as i64 + self.width as i64
    }

    /// The first row below the rectangle.
    pub const fn bottom(self) -> i64 {
        self.y as i64 + self.height as i64
    }

    /// Whether the pixel at `x`, `y` is inside the rectangle.
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x as i64 >= self.x as i64
            && (x as i64) < self.right()
            && y as i64 >= self.y as i64
            && (y as i64) < self.bottom()
    }

    /// The pixels in both rectangles, or [`Rect::EMPTY`] if there are none.
    pub const fn intersect(self, other: Self) -> Self {
        Self::from_edges(
            max(self.x as i64, other.x as i64),
            max(self.y as i64, other.y as i64),
            min(self.right(), other.right()),
            min(self.bottom(), other.bottom()),
        )
    }
}

const fn clamp(value: i64) -> i64 {
    if value < i32::MIN as i64 {
        i32::MIN as i64
    } else if value > i32::MAX as i64 {
        i32::MAX as i64
    } else {
        value
    }
}

const fn max(a: i64, b: i64) -> i64 {
    if a > b { a } else { b }
}

const fn min(a: i64, b: i64) -> i64 {
    if a < b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_the_origin_plus_the_size_even_at_the_extremes() {
        let rect = Rect::new(i32::MAX, i32::MIN, u32::MAX, u32::MAX);
        assert_eq!(rect.right(), i32::MAX as i64 + u32::MAX as i64);
        assert_eq!(rect.bottom(), i32::MIN as i64 + u32::MAX as i64);
    }

    #[test]
    fn a_rectangle_with_no_width_or_no_height_is_empty() {
        assert!(Rect::new(0, 0, 0, 5).is_empty());
        assert!(Rect::new(0, 0, 5, 0).is_empty());
        assert!(!Rect::new(0, 0, 1, 1).is_empty());
    }

    #[test]
    fn contains_includes_the_first_edge_and_excludes_the_last() {
        let rect = Rect::new(2, 3, 4, 5);
        assert!(rect.contains(2, 3));
        assert!(rect.contains(5, 7));
        assert!(!rect.contains(6, 7));
        assert!(!rect.contains(5, 8));
        assert!(!rect.contains(1, 3));
        assert!(!rect.contains(2, 2));
    }

    #[test]
    fn intersection_is_exactly_the_pixels_in_both_for_every_small_pair() {
        let mut checked = 0;
        for ax in -3..3 {
            for ay in -3..3 {
                for aw in 0..4 {
                    for ah in 0..4 {
                        for bx in -3..3 {
                            for by in -3..3 {
                                for bw in 0..4 {
                                    for bh in 0..4 {
                                        let a = Rect::new(ax, ay, aw, ah);
                                        let b = Rect::new(bx, by, bw, bh);
                                        let both = a.intersect(b);
                                        for x in -6..8 {
                                            for y in -6..8 {
                                                assert_eq!(
                                                    both.contains(x, y),
                                                    a.contains(x, y) && b.contains(x, y),
                                                    "{a:?} {b:?} at {x},{y}"
                                                );
                                            }
                                        }
                                        assert_eq!(both, b.intersect(a));
                                        checked += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 6 * 6 * 4 * 4 * 6 * 6 * 4 * 4);
    }

    #[test]
    fn disjoint_rectangles_meet_in_the_empty_rectangle() {
        let a = Rect::new(0, 0, 2, 2);
        let b = Rect::new(2, 0, 2, 2);
        assert_eq!(a.intersect(b), Rect::EMPTY);
    }

    #[test]
    fn intersecting_the_largest_rectangles_does_not_overflow() {
        let huge = Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX);
        let small = Rect::new(-1, -1, 3, 3);
        assert_eq!(huge.intersect(small), small);
        assert_eq!(huge.intersect(huge), huge);
    }

    #[test]
    fn edges_beyond_the_i32_range_are_pulled_in() {
        let rect = Rect::from_edges(-10_000_000_000, -5, 10_000_000_000, 5);
        assert_eq!(rect, Rect::new(i32::MIN, -5, u32::MAX, 10));
        assert_eq!(Rect::from_edges(3, 3, 3, 9), Rect::EMPTY);
        assert_eq!(Rect::from_edges(3, 3, 9, 3), Rect::EMPTY);
        assert_eq!(Rect::from_edges(3, 5, 9, 2), Rect::EMPTY);
        assert_eq!(Rect::from_edges(5, 0, 2, 9), Rect::EMPTY);
    }
}
'@
Write-Source 'crates\surface\src\glyph.rs' @'
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
'@
Write-Source 'crates\surface\src\surface.rs' @'
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
'@
Write-Source 'crates\surface\tests\common.rs' @'
//! Shared fixtures: pixel formats, a tiny font and a small deterministic generator.
#![allow(dead_code, unreachable_pub)]

use pixel::{Channel, PixelFormat, Rgb};
use surface::{Glyph, GlyphSource};

/// Padding words hold this, so a stray write into padding shows.
pub const SENTINEL: u32 = 0xAAAA_AAAA;

pub fn xrgb() -> PixelFormat {
    PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8)).unwrap()
}

pub fn bgrx() -> PixelFormat {
    PixelFormat::new(Channel::new(0, 8), Channel::new(8, 8), Channel::new(16, 8)).unwrap()
}

pub fn rgb565() -> PixelFormat {
    PixelFormat::new(Channel::new(11, 5), Channel::new(5, 6), Channel::new(0, 5)).unwrap()
}

pub const RED: Rgb = Rgb::from_hex(0xff0000);
pub const GREEN: Rgb = Rgb::from_hex(0x00ff00);
pub const BLUE: Rgb = Rgb::from_hex(0x0000ff);
pub const WHITE: Rgb = Rgb::from_hex(0xffffff);
pub const BLACK: Rgb = Rgb::from_hex(0x000000);

/// A font of two glyphs: `I`, a 3 by 3 serif capital, and `h`, a half-ink block.
pub struct Tiny;

const LETTER_I: [&[u8]; 3] = [&[255, 255, 255], &[0, 255, 0], &[255, 255, 255]];
const HALF: [&[u8]; 2] = [&[128, 128], &[128, 128]];

impl GlyphSource for Tiny {
    fn cell_width(&self) -> usize {
        4
    }

    fn glyph(&self, character: char) -> Option<Glyph<'_>> {
        match character {
            'I' => Glyph::new(3, &LETTER_I),
            'h' => Glyph::new(2, &HALF),
            _ => None,
        }
    }
}

/// A small linear congruential generator, so "random" tests repeat exactly.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn bits(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// A value in `low..high`.
    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        low + (self.bits() % (high - low) as u64) as i32
    }
}

/// FNV-1a over a list of words: a short fingerprint of a whole picture.
pub fn fingerprint(words: &[u32]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for word in words {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}
'@
Write-Source 'crates\surface\tests\draw.rs' @'
//! What the surface draws, checked pixel by pixel against a plain model.

mod common;

use common::*;
use pixel::Rgb;
use surface::{Image, Rect, Surface, SurfaceError};

const W: usize = 5;
const H: usize = 4;
/// Six words per row: five pixels and one word of padding.
const PITCH: usize = 24;
const STRIDE: usize = PITCH / 4;

fn fresh() -> Vec<u32> {
    vec![SENTINEL; STRIDE * H]
}

/// Every word of the buffer that is padding, not a pixel.
fn padding_untouched(words: &[u32]) -> bool {
    (0..H).all(|y| (W..STRIDE).all(|x| words[y * STRIDE + x] == SENTINEL))
}

#[test]
fn a_buffer_too_small_for_its_layout_is_refused_and_one_word_more_is_not() {
    let mut words = vec![0; STRIDE * H - 1];
    let error = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap_err();
    assert_eq!(
        error,
        SurfaceError::BufferTooSmall {
            needed: STRIDE * H,
            have: STRIDE * H - 1
        }
    );
    let mut words = vec![0; STRIDE * H];
    assert!(Surface::new(&mut words, xrgb(), W, H, PITCH).is_ok());
}

#[test]
fn a_layout_that_is_not_a_framebuffer_is_refused() {
    let mut words = vec![0; 64];
    let layout = |width, height, pitch| {
        matches!(
            Surface::new(&mut vec![0; 64], xrgb(), width, height, pitch),
            Err(SurfaceError::Layout(_))
        )
    };
    assert!(layout(0, 4, 16));
    assert!(layout(4, 0, 16));
    assert!(layout(4, 4, 12), "a pitch shorter than a row");
    assert!(layout(4, 4, 17), "a pitch that is not whole words");
    assert!(Surface::new(&mut words, xrgb(), 4, 4, 16).is_ok());
}

#[test]
fn a_side_longer_than_a_rectangle_can_address_is_refused() {
    let side = i32::MAX as usize + 1;
    let mut words = vec![0; 4];
    assert_eq!(
        Surface::new(&mut words, xrgb(), side, 1, side * 4).unwrap_err(),
        SurfaceError::TooLarge
    );
}

#[test]
fn fill_rect_paints_exactly_the_rectangle_inside_the_clip_for_every_small_case() {
    let clips = [
        None,
        Some(Rect::new(1, 1, 3, 2)),
        Some(Rect::new(-2, -2, 4, 4)),
    ];
    let mut cases = 0;
    for clip in clips {
        for x in -2..7 {
            for y in -2..6 {
                for width in 0..8 {
                    for height in 0..7 {
                        let rect = Rect::new(x, y, width, height);
                        let mut words = fresh();
                        let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
                        if let Some(clip) = clip {
                            surface.set_clip(clip);
                        }
                        let allowed = surface.clip();
                        surface.fill_rect(rect, RED);
                        for py in 0..H {
                            for px in 0..W {
                                let inside = rect.contains(px as i32, py as i32)
                                    && allowed.contains(px as i32, py as i32);
                                let expected = if inside { 0x00ff_0000 } else { SENTINEL };
                                assert_eq!(
                                    surface.word(px, py),
                                    Some(expected),
                                    "{rect:?} clip {clip:?} at {px},{py}"
                                );
                            }
                        }
                        assert!(padding_untouched(&words), "{rect:?}");
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 3 * 9 * 8 * 8 * 7);
}

#[test]
fn a_clip_beyond_the_surface_is_cut_back_to_the_surface() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.set_clip(Rect::new(-100, -100, 1000, 1000));
    assert_eq!(surface.clip(), Rect::new(0, 0, W as u32, H as u32));
    surface.set_clip(Rect::new(10, 10, 5, 5));
    assert!(surface.clip().is_empty());
    surface.fill_rect(Rect::new(0, 0, 5, 4), RED);
    assert_eq!(surface.word(0, 0), Some(SENTINEL));
    surface.reset_clip();
    assert_eq!(surface.clip(), surface.bounds());
}

#[test]
fn clear_paints_every_pixel_and_no_padding() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.clear(BLUE);
    for y in 0..H {
        for x in 0..W {
            assert_eq!(surface.word(x, y), Some(0x0000_00ff));
        }
    }
    assert!(padding_untouched(&words));
}

#[test]
fn reading_off_the_surface_gives_nothing() {
    let mut words = fresh();
    let surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    assert_eq!(surface.word(W, 0), None);
    assert_eq!(surface.word(0, H), None);
    assert_eq!(surface.pixel(W, H), None);
}

#[test]
fn the_channels_land_where_the_pixel_format_says() {
    for (format, red, green, blue) in [
        (xrgb(), 0x00ff_0000, 0x0000_ff00, 0x0000_00ff),
        (bgrx(), 0x0000_00ff, 0x0000_ff00, 0x00ff_0000),
        (rgb565(), 0xf800, 0x07e0, 0x001f),
    ] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        surface.put_pixel(0, 0, RED);
        surface.put_pixel(1, 0, GREEN);
        surface.put_pixel(2, 0, BLUE);
        assert_eq!(surface.word(0, 0), Some(red));
        assert_eq!(surface.word(1, 0), Some(green));
        assert_eq!(surface.word(2, 0), Some(blue));
    }
}

#[test]
fn a_colour_written_to_an_eight_bit_surface_reads_back_exactly() {
    let mut rng = Lcg(7);
    for format in [xrgb(), bgrx()] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        for _ in 0..500 {
            let colour = Rgb {
                red: rng.bits() as u8,
                green: rng.bits() as u8,
                blue: rng.bits() as u8,
            };
            surface.put_pixel(2, 2, colour);
            assert_eq!(surface.pixel(2, 2), Some(colour));
        }
    }
}

#[test]
fn a_colour_written_to_a_565_surface_reads_back_the_nearest_it_can_hold() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, rgb565(), W, H, PITCH).unwrap();
    for colour in [BLACK, WHITE, RED, GREEN, BLUE] {
        surface.put_pixel(0, 0, colour);
        assert_eq!(surface.pixel(0, 0), Some(colour), "full scale survives");
    }
    surface.put_pixel(
        0,
        0,
        Rgb {
            red: 100,
            green: 150,
            blue: 200,
        },
    );
    let held = surface.pixel(0, 0).unwrap();
    let before = surface.word(0, 0);
    surface.put_pixel(0, 0, held);
    assert_eq!(
        surface.word(0, 0),
        before,
        "reading back and rewriting is stable"
    );
}

#[test]
fn reading_a_pixel_and_packing_it_again_gives_back_the_same_word_on_every_format() {
    let mut rng = Lcg(565);
    for format in [xrgb(), bgrx(), rgb565()] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        for _ in 0..3000 {
            let colour = Rgb {
                red: rng.bits() as u8,
                green: rng.bits() as u8,
                blue: rng.bits() as u8,
            };
            surface.put_pixel(1, 1, colour);
            let word = surface.word(1, 1);
            let read = surface.pixel(1, 1).unwrap();
            surface.put_pixel(1, 1, read);
            assert_eq!(surface.word(1, 1), word, "{colour:?}");
        }
    }
}

#[test]
fn blending_a_colour_over_itself_never_changes_it_even_with_narrow_channels() {
    let mut rng = Lcg(31);
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, rgb565(), W, H, PITCH).unwrap();
    for _ in 0..2000 {
        let colour = Rgb {
            red: rng.bits() as u8,
            green: rng.bits() as u8,
            blue: rng.bits() as u8,
        };
        surface.put_pixel(2, 1, colour);
        let word = surface.word(2, 1);
        let held = surface.pixel(2, 1).unwrap();
        for alpha in [1, 64, 128, 200, 254] {
            surface.blend_rect(Rect::new(2, 1, 1, 1), held, alpha);
            assert_eq!(surface.word(2, 1), word, "{colour:?} at alpha {alpha}");
        }
    }
}

#[test]
fn a_translucent_fill_is_the_blend_of_what_was_there_and_the_new_colour() {
    let under = Rgb {
        red: 100,
        green: 100,
        blue: 100,
    };
    for format in [xrgb(), bgrx()] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        surface.clear(under);
        surface.blend_rect(Rect::new(1, 1, 2, 2), RED, 128);
        assert_eq!(surface.pixel(1, 1), Some(under.blend(RED, 128)));
        assert_eq!(surface.pixel(2, 2), Some(under.blend(RED, 128)));
        assert_eq!(surface.pixel(0, 0), Some(under), "outside the rectangle");
        assert_eq!(surface.pixel(3, 3), Some(under), "outside the rectangle");
    }
}

#[test]
fn alpha_zero_changes_nothing_and_alpha_full_is_a_plain_fill() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.blend_rect(Rect::new(0, 0, 5, 4), RED, 0);
    assert_eq!(surface.word(0, 0), Some(SENTINEL));
    surface.blend_rect(Rect::new(0, 0, 5, 4), RED, 255);
    assert_eq!(surface.word(0, 0), Some(0x00ff_0000));
    assert_eq!(surface.word(4, 3), Some(0x00ff_0000));
}

#[test]
fn stroke_rect_draws_a_ring_of_the_asked_thickness() {
    let mut words = vec![0; 7 * 7];
    let mut surface = Surface::new(&mut words, xrgb(), 7, 7, 28).unwrap();
    surface.stroke_rect(Rect::new(1, 1, 5, 5), 1, WHITE);
    let picture: Vec<String> = (0..7)
        .map(|y| {
            (0..7)
                .map(|x| {
                    if surface.word(x, y) == Some(0) {
                        '.'
                    } else {
                        '#'
                    }
                })
                .collect()
        })
        .collect();
    assert_eq!(
        picture,
        [
            ".......", //
            ".#####.", //
            ".#...#.", //
            ".#...#.", //
            ".#...#.", //
            ".#####.", //
            ".......",
        ]
    );
}

#[test]
fn a_thick_stroke_with_no_room_inside_fills_the_rectangle() {
    for thickness in [3, 4, 100] {
        let mut words = vec![0; 7 * 7];
        let mut surface = Surface::new(&mut words, xrgb(), 7, 7, 28).unwrap();
        surface.stroke_rect(Rect::new(1, 1, 5, 5), thickness, WHITE);
        assert_eq!(
            surface.word(3, 3),
            Some(0x00ff_ffff),
            "thickness {thickness}"
        );
        assert_eq!(surface.word(0, 0), Some(0));
    }
    let mut words = vec![0; 7 * 7];
    let mut surface = Surface::new(&mut words, xrgb(), 7, 7, 28).unwrap();
    surface.stroke_rect(Rect::new(1, 1, 5, 5), 2, WHITE);
    assert_eq!(
        surface.word(3, 3),
        Some(0),
        "a two pixel ring leaves the middle"
    );
    surface.stroke_rect(Rect::new(1, 1, 5, 5), 0, BLUE);
    assert_eq!(surface.word(3, 3), Some(0), "zero thickness draws nothing");
}

#[test]
fn a_stroke_partly_off_the_surface_draws_only_the_visible_edges() {
    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.stroke_rect(Rect::new(-2, -2, 5, 5), 1, WHITE);
    // Only the right edge (x = 2) and the bottom edge (y = 2) are on screen.
    assert_eq!(surface.word(2, 0), Some(0x00ff_ffff));
    assert_eq!(surface.word(0, 2), Some(0x00ff_ffff));
    assert_eq!(surface.word(0, 0), Some(0));
    assert_eq!(surface.word(1, 1), Some(0));
}

#[test]
fn a_blit_copies_the_image_and_clips_at_every_edge() {
    let source = [1, 2, 3, 4, 5, 6];
    let image = Image::new(&source, 3, 2).unwrap();
    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.blit(image, 1, 1);
    assert_eq!(
        [surface.word(1, 1), surface.word(2, 1), surface.word(3, 1)],
        [Some(1), Some(2), Some(3)]
    );
    assert_eq!(
        [surface.word(1, 2), surface.word(2, 2), surface.word(3, 2)],
        [Some(4), Some(5), Some(6)]
    );
    assert_eq!(surface.word(0, 1), Some(0));
    assert_eq!(surface.word(1, 3), Some(0));

    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.blit(image, -1, -1);
    assert_eq!(
        surface.word(0, 0),
        Some(5),
        "the image's second column and second row"
    );
    assert_eq!(surface.word(1, 0), Some(6));
    assert_eq!(surface.word(2, 0), Some(0));
    assert_eq!(surface.word(0, 1), Some(0));

    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.blit(image, 3, 3);
    assert_eq!(surface.word(3, 3), Some(1));
    surface.blit(image, 4, 0);
    surface.blit(image, 0, 4);
    surface.blit(image, -3, 0);
    surface.blit(image, 0, -2);
    assert_eq!(
        surface.word(0, 0),
        Some(0),
        "images entirely off the surface draw nothing"
    );
}

#[test]
fn a_blit_obeys_the_clip_and_leaves_padding_alone() {
    let source = [9; 12];
    let image = Image::new(&source, 4, 3).unwrap();
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.set_clip(Rect::new(1, 1, 2, 2));
    surface.blit(image, 0, 0);
    assert_eq!(surface.word(0, 0), Some(SENTINEL));
    assert_eq!(surface.word(1, 1), Some(9));
    assert_eq!(surface.word(2, 2), Some(9));
    assert_eq!(surface.word(3, 1), Some(SENTINEL));
    assert_eq!(surface.word(1, 3), Some(SENTINEL));
    assert!(padding_untouched(&words));
}

#[test]
fn an_image_must_be_exactly_as_long_as_its_size() {
    assert!(Image::new(&[0; 5], 2, 3).is_none());
    assert!(Image::new(&[0; 7], 2, 3).is_none());
    assert!(Image::new(&[0; 6], 2, 3).is_some());
    assert!(
        Image::new(&[], usize::MAX, 2).is_none(),
        "the size overflows"
    );
}

#[test]
fn text_draws_solid_ink_blends_partial_ink_and_advances_by_whole_cells() {
    let mut words = vec![0; 12 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 12, 4, 48).unwrap();
    let next = surface.draw_text(0, 0, &Tiny, "I I", WHITE);
    assert_eq!(next, 12, "three cells of four pixels");
    // 'I': the top bar is solid, the stem is only in the middle column.
    assert_eq!(surface.pixel(0, 0), Some(WHITE));
    assert_eq!(surface.pixel(1, 1), Some(WHITE));
    assert_eq!(surface.pixel(0, 1), Some(BLACK));
    // The space has no glyph, so the second 'I' starts a full two cells along.
    assert_eq!(surface.pixel(3, 0), Some(BLACK));
    assert_eq!(surface.pixel(8, 0), Some(WHITE));

    let next = surface.draw_text(0, 3, &Tiny, "h", RED);
    assert_eq!(next, 4);
    assert_eq!(surface.pixel(0, 3), Some(BLACK.blend(RED, 128)));
}

#[test]
fn text_starting_off_the_surface_still_advances_and_clips() {
    let mut words = vec![0; 8 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 8, 4, 32).unwrap();
    let next = surface.draw_text(-2, 0, &Tiny, "II", WHITE);
    assert_eq!(next, 6);
    assert_eq!(
        surface.pixel(0, 0),
        Some(WHITE),
        "the right of the first I shows"
    );
    assert_eq!(surface.pixel(1, 0), Some(BLACK), "the gap between cells");
    assert_eq!(surface.pixel(2, 0), Some(WHITE), "the second I starts at 2");
}

#[test]
fn every_row_of_a_glyph_must_be_as_wide_as_the_glyph() {
    use surface::Glyph;
    assert!(Glyph::new(2, &[&[0, 0], &[0]]).is_none());
    assert!(Glyph::new(2, &[&[0, 0, 0], &[0, 0]]).is_none());
    let glyph = Glyph::new(2, &[&[1, 2], &[3, 4]]).unwrap();
    assert_eq!((glyph.width(), glyph.height()), (2, 2));
    assert_eq!(glyph.coverage(1, 0), Some(2));
    assert_eq!(glyph.coverage(0, 1), Some(3));
    assert_eq!(glyph.coverage(2, 0), None);
    assert_eq!(glyph.coverage(0, 2), None);
    assert!(Glyph::new(5, &[]).is_some(), "no rows is an empty glyph");
}

#[test]
fn nothing_drawn_ever_lands_outside_the_clip_or_in_padding() {
    let (width, height, stride) = (16usize, 12usize, 20usize);
    let source: Vec<u32> = (1..=20).collect();
    let image = Image::new(&source, 5, 4).unwrap();
    let mut rng = Lcg(2026);
    for round in 0..4000 {
        let mut words: Vec<u32> = (0..stride * height)
            .map(|i| {
                if i % stride < width {
                    0x0001_0000 + i as u32
                } else {
                    SENTINEL
                }
            })
            .collect();
        let before = words.clone();
        let mut surface = Surface::new(&mut words, xrgb(), width, height, stride * 4).unwrap();
        let clip = Rect::new(
            rng.range(-5, 20),
            rng.range(-5, 16),
            rng.range(0, 24) as u32,
            rng.range(0, 18) as u32,
        );
        surface.set_clip(clip);
        let clip = surface.clip();
        for _ in 0..6 {
            let rect = Rect::new(
                rng.range(-10, 30),
                rng.range(-10, 24),
                rng.range(0, 30) as u32,
                rng.range(0, 24) as u32,
            );
            let colour = Rgb {
                red: rng.bits() as u8,
                green: rng.bits() as u8,
                blue: rng.bits() as u8,
            };
            match rng.bits() % 7 {
                0 => surface.fill_rect(rect, colour),
                1 => surface.blend_rect(rect, colour, rng.bits() as u8),
                2 => surface.stroke_rect(rect, rng.range(0, 6) as u32, colour),
                3 => surface.blit(image, rect.x, rect.y),
                4 => {
                    surface.draw_text(rect.x, rect.y, &Tiny, "IhI", colour);
                }
                5 => surface.put_pixel(rect.x, rect.y, colour),
                _ => surface.clear(colour),
            }
        }
        for y in 0..height {
            for x in 0..stride {
                let index = y * stride + x;
                let drawable = x < width && clip.contains(x as i32, y as i32);
                if !drawable {
                    assert_eq!(words[index], before[index], "round {round} at {x},{y}");
                }
            }
        }
    }
}

#[test]
fn a_word_below_the_last_row_is_not_on_the_surface_even_if_the_buffer_is_longer() {
    let mut words = vec![SENTINEL; STRIDE * H + STRIDE * 2];
    let surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    assert_eq!(surface.word(0, H), None);
    assert_eq!(surface.word(W - 1, H - 1), Some(SENTINEL));
    assert_eq!(surface.pixel(0, H), None);
}

#[test]
fn put_pixel_changes_that_pixel_and_no_other() {
    for (x, y) in [(0, 0), (4, 3), (2, 1)] {
        let mut words = vec![0; W * H];
        let mut surface = Surface::new(&mut words, xrgb(), W, H, W * 4).unwrap();
        surface.put_pixel(x, y, WHITE);
        for py in 0..H {
            for px in 0..W {
                let expected = if (px as i32, py as i32) == (x, y) {
                    0x00ff_ffff
                } else {
                    0
                };
                assert_eq!(surface.word(px, py), Some(expected), "{x},{y} at {px},{py}");
            }
        }
    }
}

#[test]
fn stroke_rect_paints_exactly_the_pixels_within_the_thickness_of_an_edge() {
    let mut cases = 0;
    for x in -2..8 {
        for y in -2..8 {
            for width in 0..9u32 {
                for height in 0..9u32 {
                    for thickness in 0..6u32 {
                        let rect = Rect::new(x, y, width, height);
                        let mut words = vec![0; 6 * 6];
                        let mut surface = Surface::new(&mut words, xrgb(), 6, 6, 24).unwrap();
                        surface.stroke_rect(rect, thickness, WHITE);
                        for py in 0..6i32 {
                            for px in 0..6i32 {
                                let near_edge = (px - x) < thickness as i32
                                    || (x + width as i32 - 1 - px) < thickness as i32
                                    || (py - y) < thickness as i32
                                    || (y + height as i32 - 1 - py) < thickness as i32;
                                let painted = rect.contains(px, py) && near_edge;
                                let expected = if painted { 0x00ff_ffff } else { 0 };
                                assert_eq!(
                                    surface.word(px as usize, py as usize),
                                    Some(expected),
                                    "{rect:?} thickness {thickness} at {px},{py}"
                                );
                            }
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 10 * 10 * 9 * 9 * 6);
}
'@
Write-Source 'crates\surface-preview\Cargo.toml' @'
[package]
name = "surface-preview"
description = "Draws a scene with the surface crate and saves it as a PNG, so it can be looked at on the host."
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
surface.workspace = true
pixel.workspace = true
noto-sans-mono-bitmap.workspace = true
'@
Write-Source 'crates\surface-preview\src\lib.rs' @'
//! Draw a scene with the `surface` crate and save it as a picture.
//!
//! This is the host half of the loop: the same drawing code that will run in
//! the kernel runs here, and the result is a PNG you can open.

pub mod font;
pub mod png;
pub mod scene;
'@
Write-Source 'crates\surface-preview\src\font.rs' @'
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
'@
Write-Source 'crates\surface-preview\src\png.rs' @'
//! A PNG writer with no dependencies.
//!
//! It stores the pixels uncompressed, which makes files large and the code
//! small: the point is to look at a picture, not to ship one.

/// The largest block a stored deflate stream can hold.
const STORED_BLOCK: usize = 65_535;

/// Encode a picture of `width` by `height` pixels as an 8-bit RGB PNG.
///
/// `rgb` holds three bytes per pixel, row by row. Returns `None` if its
/// length is not `width * height * 3` or the size does not fit a PNG.
pub fn encode(width: u32, height: u32, rgb: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return None;
    }
    let row = width as usize * 3;
    if row.checked_mul(height as usize)? != rgb.len() {
        return None;
    }

    // Each row is a filter byte (0, none) and then its pixels.
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgb.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, *b"IHDR", &header);
    chunk(&mut out, *b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, *b"IEND", &[]);
    Some(out)
}

fn chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    // 0x78 0x01: deflate with a 32 KiB window, no preset dictionary, fastest.
    let mut out = vec![0x78, 0x01];
    let mut blocks = data.chunks(STORED_BLOCK).peekable();
    if blocks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(block) = blocks.next() {
        out.push(u8::from(blocks.peek().is_none()));
        let length = block.len() as u16;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&(!length).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The CRC-32 PNG chunks are protected with.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The checksum a zlib stream ends with.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checksums_match_their_published_check_values() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        assert_eq!(crc32(b""), 0);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn a_picture_with_the_wrong_amount_of_data_is_refused() {
        assert!(encode(2, 2, &[0; 11]).is_none());
        assert!(encode(2, 2, &[0; 13]).is_none());
        assert!(encode(0, 2, &[]).is_none());
        assert!(encode(2, 2, &[0; 12]).is_some());
    }

    #[test]
    fn the_file_starts_with_the_signature_and_ends_with_iend() {
        let png = encode(1, 1, &[1, 2, 3]).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..20], &1u32.to_be_bytes());
    }

    #[test]
    fn data_longer_than_one_stored_block_is_split_and_only_the_last_block_is_marked() {
        let stream = zlib_stored(&vec![7; STORED_BLOCK + 10]);
        assert_eq!(stream[2], 0, "first block is not the last");
        let second = 2 + 5 + STORED_BLOCK;
        assert_eq!(stream[second], 1, "second block is the last");
        assert_eq!(&stream[second + 1..second + 3], &10u16.to_le_bytes());
    }

    /// Read a PNG back the strict way and return its size and pixels.
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let mut at = 8;
        let mut header = None;
        let mut data = Vec::new();
        let mut kinds = Vec::new();
        while at < png.len() {
            let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[at + 4..at + 8].try_into().unwrap();
            let body = &png[at + 8..at + 8 + length];
            let crc =
                u32::from_be_bytes(png[at + 8 + length..at + 12 + length].try_into().unwrap());
            assert_eq!(crc, crc32(&png[at + 4..at + 8 + length]), "crc of {kind:?}");
            match &kind {
                b"IHDR" => header = Some(body.to_vec()),
                b"IDAT" => data.extend_from_slice(body),
                _ => {}
            }
            kinds.push(kind);
            at += 12 + length;
        }
        assert_eq!(at, png.len());
        assert_eq!(kinds.first(), Some(b"IHDR"));
        assert_eq!(kinds.last(), Some(b"IEND"));
        let header = header.unwrap();
        let width = u32::from_be_bytes(header[0..4].try_into().unwrap());
        let height = u32::from_be_bytes(header[4..8].try_into().unwrap());
        assert_eq!(
            &header[8..],
            &[8, 2, 0, 0, 0],
            "8-bit truecolour, no interlace"
        );

        assert_eq!(&data[..2], &[0x78, 0x01]);
        let mut raw = Vec::new();
        let mut cursor = 2;
        loop {
            let last = data[cursor];
            let length = u16::from_le_bytes([data[cursor + 1], data[cursor + 2]]);
            let complement = u16::from_le_bytes([data[cursor + 3], data[cursor + 4]]);
            assert_eq!(complement, !length, "block length check");
            assert!(last <= 1);
            raw.extend_from_slice(&data[cursor + 5..cursor + 5 + length as usize]);
            cursor += 5 + length as usize;
            if last == 1 {
                break;
            }
        }
        let checksum = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap());
        assert_eq!(checksum, adler32(&raw), "adler of the decompressed data");
        assert_eq!(cursor + 4, data.len());

        let row = width as usize * 3;
        let mut pixels = Vec::new();
        for line in raw.chunks(row + 1) {
            assert_eq!(line[0], 0, "filter type none");
            pixels.extend_from_slice(&line[1..]);
        }
        (width, height, pixels)
    }

    #[test]
    fn what_goes_in_comes_out_for_several_shapes() {
        for (width, height) in [(1, 1), (3, 2), (7, 5), (300, 70), (200, 100)] {
            let rgb: Vec<u8> = (0..width * height * 3)
                .map(|i| (i * 31 + i / 7) as u8)
                .collect();
            let png = encode(width as u32, height as u32, &rgb).unwrap();
            assert_eq!(
                decode(&png),
                (width as u32, height as u32, rgb),
                "{width}x{height}"
            );
        }
    }

    #[test]
    fn adler_matches_the_definition_on_long_inputs() {
        let mut state = 1u64;
        let data: Vec<u8> = (0..200_000)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                (state >> 56) as u8
            })
            .collect();
        let n = data.len() as u64;
        let a = (1 + data.iter().map(|&d| u64::from(d)).sum::<u64>()) % 65_521;
        let b = (n + data
            .iter()
            .enumerate()
            .map(|(i, &d)| (n - i as u64) * u64::from(d))
            .sum::<u64>())
            % 65_521;
        assert_eq!(adler32(&data), ((b << 16) | a) as u32);
    }
}
'@
Write-Source 'crates\surface-preview\src\scene.rs' @'
//! The first scene: a top bar and a shell window on a dark desktop.
//!
//! It is drawn with the public surface API only, the way a UI layer will, so
//! what shows here is what the kernel framebuffer will show.

use pixel::{Channel, PixelFormat, Rgb};
use surface::{Rect, Surface};

use crate::font::NotoMono;

/// Width of the preview picture.
pub const WIDTH: usize = 800;
/// Height of the preview picture.
pub const HEIGHT: usize = 480;

const SKY_TOP: Rgb = Rgb::from_hex(0x0f1420);
const SKY_BOTTOM: Rgb = Rgb::from_hex(0x1d2744);
const BAR: Rgb = Rgb::from_hex(0x0b0e17);
const EDGE: Rgb = Rgb::from_hex(0x34406a);
const BODY: Rgb = Rgb::from_hex(0x151a28);
const TITLE: Rgb = Rgb::from_hex(0x1f2740);
const TEXT: Rgb = Rgb::from_hex(0xd7def0);
const DIM: Rgb = Rgb::from_hex(0x7f8bb0);
const ACCENT: Rgb = Rgb::from_hex(0x4f7cff);
const CLOSE: Rgb = Rgb::from_hex(0xff5f57);
const MINIMISE: Rgb = Rgb::from_hex(0xfebc2e);
const ZOOM: Rgb = Rgb::from_hex(0x28c840);

/// The pixel format of the preview buffer: `0x00RRGGBB`, like QEMU's display.
pub fn format() -> PixelFormat {
    PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8))
        .expect("valid channel masks")
}

/// Draw the whole scene on `surface`.
pub fn draw(surface: &mut Surface<'_>) {
    let font = NotoMono;
    let width = surface.width() as u32;
    let height = surface.height();

    // A vertical gradient, one row at a time.
    for y in 0..height {
        let step = (y * 255 / height.max(1)) as u8;
        let row = Rect::new(0, y as i32, width, 1);
        surface.fill_rect(row, SKY_TOP.blend(SKY_BOTTOM, step));
    }

    // The top bar.
    surface.fill_rect(Rect::new(0, 0, width, 28), BAR);
    surface.fill_rect(Rect::new(0, 28, width, 1), EDGE);
    surface.draw_text(12, 6, &font, "Forge.OS", TEXT);
    let clock = "12:00";
    let clock_width = (clock.len() * surface::GlyphSource::cell_width(&font)) as i32;
    surface.draw_text(width as i32 - clock_width - 12, 6, &font, clock, DIM);

    window(surface, Rect::new(80, 64, 560, 320), "shell");

    // A second window, dragged half off the right edge: it must clip, not wrap.
    window(
        surface,
        Rect::new(width as i32 - 150, 300, 300, 200),
        "notes",
    );
}

/// A window: soft shadow, border, title bar, three buttons and a title.
fn window(surface: &mut Surface<'_>, frame: Rect, title: &str) {
    let font = NotoMono;

    surface.blend_rect(offset(frame, 10, 14), Rgb::from_hex(0x000000), 40);
    surface.blend_rect(offset(frame, 6, 8), Rgb::from_hex(0x000000), 70);

    surface.fill_rect(frame, BODY);
    let bar = Rect::new(frame.x, frame.y, frame.width, 30);
    surface.fill_rect(bar, TITLE);
    surface.stroke_rect(frame, 1, EDGE);

    for (index, colour) in [CLOSE, MINIMISE, ZOOM].into_iter().enumerate() {
        let x = frame.x + 12 + index as i32 * 20;
        surface.fill_rect(Rect::new(x, frame.y + 10, 10, 10), colour);
    }
    surface.draw_text(frame.x + 84, frame.y + 7, &font, title, DIM);

    if title == "shell" {
        shell_contents(surface, frame);
    }
}

fn shell_contents(surface: &mut Surface<'_>, frame: Rect) {
    let font = NotoMono;
    let content = Rect::new(
        frame.x + 1,
        frame.y + 31,
        frame.width - 2,
        frame.height - 32,
    );
    let line = NotoMono::LINE_HEIGHT as i32 + 4;
    let (x, mut y) = (content.x + 12, content.y + 12);

    // Nothing the text does may reach the border: the content is a clip.
    surface.set_clip(content);
    for (prompt, text) in [
        ("$ ", "help"),
        ("  ", "help  echo  clear  uptime  exit"),
        ("$ ", "uptime"),
        ("  ", "up 0:01:42"),
        (
            "$ ",
            "echo a line that is much too long for this window and must be cut off at the edge",
        ),
    ] {
        let after = surface.draw_text(x, y, &font, prompt, ACCENT);
        surface.draw_text(after as i32, y, &font, text, TEXT);
        y += line;
    }
    let after = surface.draw_text(x, y, &font, "$ ", ACCENT);
    let cell = surface::GlyphSource::cell_width(&font) as u32;
    surface.blend_rect(
        Rect::new(after as i32, y, cell, NotoMono::LINE_HEIGHT as u32),
        TEXT,
        200,
    );
    surface.reset_clip();

    // A button in the corner, with a translucent highlight across its top half.
    let button = Rect::new(
        frame.right() as i32 - 96,
        frame.bottom() as i32 - 44,
        80,
        28,
    );
    surface.fill_rect(button, ACCENT);
    surface.blend_rect(
        Rect::new(button.x, button.y, button.width, 14),
        Rgb::from_hex(0xffffff),
        40,
    );
    surface.stroke_rect(button, 1, Rgb::from_hex(0x8fa8ff));
    surface.draw_text(
        button.x + 24,
        button.y + 5,
        &font,
        "Run",
        Rgb::from_hex(0xffffff),
    );
}

fn offset(rect: Rect, dx: i32, dy: i32) -> Rect {
    Rect::new(rect.x + dx, rect.y + dy, rect.width, rect.height)
}

/// Render the scene into a new buffer and return the packed pixel words.
pub fn render() -> Vec<u32> {
    let mut words = vec![0; WIDTH * HEIGHT];
    let mut surface = Surface::new(&mut words, format(), WIDTH, HEIGHT, WIDTH * 4)
        .expect("the preview buffer is a valid surface");
    draw(&mut surface);
    words
}

/// The picture as bytes, three per pixel, for [`crate::png::encode`].
pub fn to_rgb(words: &[u32]) -> Vec<u8> {
    words
        .iter()
        .flat_map(|word| [(word >> 16) as u8, (word >> 8) as u8, *word as u8])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(words: &[u32]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for word in words {
            for byte in word.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash
    }

    /// The scene, fingerprinted. If drawing changes on purpose, look at the
    /// new picture, then update this number; if it changes by accident, a
    /// drawing primitive has changed behaviour.
    const GOLDEN: u64 = 728_222_996_402_039_696;

    #[test]
    fn the_scene_is_the_picture_we_looked_at() {
        assert_eq!(fingerprint(&render()), GOLDEN);
    }

    #[test]
    fn drawing_the_scene_twice_gives_the_same_picture() {
        assert_eq!(render(), render());
    }

    #[test]
    fn the_window_clips_its_text_and_the_second_window_leaves_the_right_edge_alone() {
        let words = render();
        let at = |x: usize, y: usize| words[y * WIDTH + x];
        let pack = |colour| format().pack(colour);
        // The right border of the shell window (x = 80 + 560 - 1) is still the border
        // on the row of the over-long line, so the text did not escape its clip.
        let long_line_row = 64 + 31 + 12 + 4 * (16 + 4) + 8;
        assert_eq!(at(80 + 560 - 1, long_line_row), pack(EDGE));
        // The notes window starts at x = 650 and runs off the picture without wrapping.
        assert_eq!(at(WIDTH - 1, 330), pack(BODY));
        assert_eq!(
            at(0, 330),
            pack(SKY_TOP.blend(SKY_BOTTOM, (330 * 255 / HEIGHT) as u8))
        );
    }

    #[test]
    fn rgb_bytes_come_out_red_green_blue() {
        assert_eq!(
            to_rgb(&[0x0011_2233, 0x00ab_cdef]),
            [0x11, 0x22, 0x33, 0xab, 0xcd, 0xef]
        );
    }
}
'@
Write-Source 'crates\surface-preview\src\main.rs' @'
use std::process::ExitCode;

use surface_preview::{png, scene};

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "preview.png".into());
    let rgb = scene::to_rgb(&scene::render());
    let Some(file) = png::encode(scene::WIDTH as u32, scene::HEIGHT as u32, &rgb) else {
        eprintln!("the scene is not a valid picture");
        return ExitCode::FAILURE;
    };
    match std::fs::write(&path, file) {
        Ok(()) => {
            println!("wrote {path} ({}x{})", scene::WIDTH, scene::HEIGHT);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cannot write {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
'@

Write-Host "Adding the crates to the workspace Cargo.toml..."
$text = [System.IO.File]::ReadAllText($manifest)
$nl = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
$changed = $false

function Add-ToList([string]$Key) {
  $pattern = "(?s)(^$Key\s*=\s*\[.*?)(\r?\n\])"
  $m = [regex]::Match($script:text, $pattern, 'Multiline')
  if (-not $m.Success) { throw "Could not find the '$Key' list in Cargo.toml - add the two crates by hand." }
  $add = ''
  foreach ($crate in 'crates/surface', 'crates/surface-preview') {
    if ($m.Groups[1].Value -notmatch [regex]::Escape("`"$crate`"")) { $add += "$($script:nl)  `"$crate`"," }
  }
  if ($add) {
    $script:text = $script:text.Substring(0, $m.Groups[1].Index) + $m.Groups[1].Value + $add + $m.Groups[2].Value + $script:text.Substring($m.Index + $m.Length)
    $script:changed = $true
  }
}
Add-ToList 'members'
Add-ToList 'default-members'

function Add-Dependency([string]$Name, [string]$Line, [string]$After) {
  if ($script:text -match "(?m)^$([regex]::Escape($Name))\s*=") { return }
  $m = [regex]::Match($script:text, "(?m)^$([regex]::Escape($After))\s*=.*$")
  if (-not $m.Success) { throw "Could not find '$After' in [workspace.dependencies] - add '$Name' by hand: $Line" }
  $script:text = $script:text.Substring(0, $m.Index + $m.Length) + $script:nl + $Line + $script:text.Substring($m.Index + $m.Length)
  $script:changed = $true
}
Add-Dependency 'surface' 'surface    = { path = "crates/surface" }' 'shell-core'
Add-Dependency 'noto-sans-mono-bitmap' 'noto-sans-mono-bitmap = { version = "0.3", default-features = false, features = ["regular", "size_16", "unicode-basic-latin"] }' 'ps2-keys'

if ($changed) {
  [System.IO.File]::WriteAllText($manifest, $text, $utf8)
  Write-Host "  updated Cargo.toml"
} else {
  Write-Host "  Cargo.toml already lists the crates"
}

Write-Host ""
Write-Host "Done. Next, from $ForgeRoot :"
Write-Host "  cargo fmt --all --check"
Write-Host "  cargo clippy --workspace --all-targets -- -D warnings"
Write-Host "  cargo test -p surface -p surface-preview"
Write-Host "  cargo run -p surface-preview -- preview.png"
