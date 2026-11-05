//! The shell's display, drawn on a [`Surface`].
//!
//! [`SurfaceDisplay`] is a `platform::Display` backend: it turns each character
//! cell the shell writes into pixels inside a rectangle of a surface, so the
//! shell can run in a window of a larger picture. Like the shell, it neither
//! allocates nor touches hardware. When the shell presents, the display tells a
//! [`Sink`] which rectangle changed; on the host the sink can do nothing, and in
//! the kernel it will copy that rectangle to the screen.
//!
//! # Examples
//!
//! ```
//! use pixel::{Channel, PixelFormat, Rgb};
//! use platform::{Cell, Display};
//! use shell_surface::{NoSink, Palette, SurfaceDisplay};
//! use surface::{Glyph, GlyphSource, Rect, Surface};
//!
//! struct Block;
//! impl GlyphSource for Block {
//!     fn cell_width(&self) -> usize { 2 }
//!     fn glyph(&self, _: char) -> Option<Glyph<'_>> { Glyph::new(2, &[&[255, 255], &[255, 255]]) }
//! }
//!
//! let format = PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8)).unwrap();
//! let mut words = [0u32; 8 * 4];
//! let mut surface = Surface::new(&mut words, format, 8, 4, 32).unwrap();
//! let palette = Palette::new(Rgb::from_hex(0x000000), Rgb::from_hex(0xffffff), Rgb::from_hex(0xffffff));
//! let mut display =
//!     SurfaceDisplay::new(&mut surface, &Block, 2, Rect::new(0, 0, 8, 4), palette, NoSink);
//! assert_eq!(display.size().cols, 4);
//! display.put(1, 0, Cell::plain('x'));
//! display.present();
//! assert_eq!(surface.word(2, 0), Some(0x00ff_ffff));
//! ```

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]

use pixel::Rgb;
use platform::{Cell, CellGeometry, Display, Size};
use surface::{GlyphSource, Rect, Surface};

/// The three colours the shell is drawn in. `platform::Style` carries no colour
/// of its own, so the backend decides what plain, bold and inverse look like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Behind every cell.
    pub background: Rgb,
    /// Ordinary text.
    pub foreground: Rgb,
    /// Text of a bold cell.
    pub bold: Rgb,
}

impl Palette {
    /// A palette of a background, a text colour and a brighter text colour.
    pub const fn new(background: Rgb, foreground: Rgb, bold: Rgb) -> Self {
        Self {
            background,
            foreground,
            bold,
        }
    }
}

/// Told which rectangle of the surface changed whenever the shell presents.
pub trait Sink {
    /// `dirty` is the smallest rectangle covering every cell written since the
    /// last call, cut to the surface and the display's area. It is never empty.
    fn present(&mut self, surface: &Surface<'_>, dirty: Rect);
}

/// A sink that does nothing, for when the surface is already the destination.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSink;

impl Sink for NoSink {
    fn present(&mut self, _surface: &Surface<'_>, _dirty: Rect) {}
}

/// A `platform::Display` that draws character cells on part of a surface.
#[derive(Debug)]
pub struct SurfaceDisplay<'a, 's, F, S> {
    surface: &'a mut Surface<'s>,
    font: &'a F,
    area: Rect,
    geometry: CellGeometry,
    cell_height: usize,
    size: Size,
    palette: Palette,
    sink: S,
    dirty: Rect,
}

impl<'a, 's, F: GlyphSource, S: Sink> SurfaceDisplay<'a, 's, F, S> {
    /// Draw cells of `font.cell_width()` by `cell_height` pixels inside `area`.
    ///
    /// The grid is as many whole cells as fit in `area`; any remainder is left
    /// at the background colour. A glyph shorter than its cell is centred
    /// vertically, so extra height becomes line spacing, and a glyph larger
    /// than its cell is cut off at the cell's edge.
    pub fn new(
        surface: &'a mut Surface<'s>,
        font: &'a F,
        cell_height: usize,
        area: Rect,
        palette: Palette,
        sink: S,
    ) -> Self {
        let geometry = CellGeometry::new(0, font.cell_width(), cell_height);
        let size = geometry.fit(area.width as usize, area.height as usize);
        Self {
            surface,
            font,
            area,
            geometry,
            cell_height,
            size,
            palette,
            sink,
            dirty: Rect::EMPTY,
        }
    }

    /// The rectangle the grid lives in.
    pub const fn area(&self) -> Rect {
        self.area
    }

    /// The sink, to see what it has been told.
    pub const fn sink(&self) -> &S {
        &self.sink
    }

    /// The part of `rect` that is on the surface. Every rectangle passed in is
    /// the area or a whole cell of it, so it is already inside the area.
    fn drawable(&self, rect: Rect) -> Rect {
        rect.intersect(self.surface.bounds())
    }

    fn mark(&mut self, rect: Rect) {
        self.dirty = self.dirty.union(self.drawable(rect));
    }
}

impl<F: GlyphSource, S: Sink> Display for SurfaceDisplay<'_, '_, F, S> {
    fn size(&self) -> Size {
        self.size
    }

    fn put(&mut self, col: u16, row: u16, cell: Cell) {
        if col >= self.size.cols || row >= self.size.rows {
            return;
        }
        let Some((x, y)) = self.geometry.origin(usize::from(col), usize::from(row)) else {
            return;
        };
        let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
            return;
        };
        let left = self.area.x.saturating_add(x);
        let top = self.area.y.saturating_add(y);
        let rect = Rect::new(
            left,
            top,
            self.font.cell_width() as u32,
            self.cell_height as u32,
        );

        let text = if cell.style.bold {
            self.palette.bold
        } else {
            self.palette.foreground
        };
        let (background, ink) = if cell.style.inverse {
            (text, self.palette.background)
        } else {
            (self.palette.background, text)
        };

        let saved = self.surface.clip();
        // Nothing a cell draws may reach its neighbours, whatever its glyph's size.
        // The cell is inside the area, because only whole cells of the grid get here.
        self.surface.set_clip(saved.intersect(rect));
        self.surface.fill_rect(rect, background);
        if let Some(glyph) = self.font.glyph(cell.ch) {
            let spare = self.cell_height.saturating_sub(glyph.height()) / 2;
            self.surface
                .draw_glyph(left, top.saturating_add(spare as i32), glyph, ink);
        }
        self.surface.set_clip(saved);
        self.mark(rect);
    }

    fn present(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        let dirty = self.dirty;
        self.dirty = Rect::EMPTY;
        self.sink.present(&*self.surface, dirty);
    }

    fn clear(&mut self) {
        let saved = self.surface.clip();
        self.surface.set_clip(saved.intersect(self.area));
        self.surface.fill_rect(self.area, self.palette.background);
        self.surface.set_clip(saved);
        self.mark(self.area);
    }
}
