//! Fixtures and an independent model of what a cell should look like.
#![allow(dead_code, unreachable_pub)]

use pixel::{Channel, PixelFormat, Rgb};
use platform::{Cell, Style};
use shell_surface::Palette;
use surface::{Glyph, GlyphSource, Rect, Surface};

pub const BASE: u32 = 0x0033_6699;
pub const PAD: u32 = 0xAAAA_AAAA;
pub const PALETTE: Palette = Palette::new(
    Rgb::from_hex(0x101820),
    Rgb::from_hex(0xd0d8e0),
    Rgb::from_hex(0xffffff),
);
pub const CELL_W: usize = 4;
pub const CELL_H: usize = 5;

pub fn format() -> PixelFormat {
    PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8)).unwrap()
}

/// A buffer of `width` by `height` pixels with two padding words at the end of
/// every row, every pixel `BASE`.
pub fn buffer(width: usize, height: usize) -> Vec<u32> {
    let stride = width + 2;
    (0..stride * height)
        .map(|i| if i % stride < width { BASE } else { PAD })
        .collect()
}

pub fn surface(words: &mut [u32], width: usize, height: usize) -> Surface<'_> {
    Surface::new(words, format(), width, height, (width + 2) * 4).unwrap()
}

/// Glyphs of four sizes, so the sizes that are not the cell's are covered.
pub struct Odd;

const I: [&[u8]; 3] = [&[255, 255, 255], &[0, 255, 0], &[255, 255, 255]];
const H: [&[u8]; 2] = [&[128, 128], &[128, 128]];
const B: [&[u8]; 5] = [
    &[255, 255, 255, 0],
    &[255, 0, 0, 255],
    &[255, 255, 255, 0],
    &[255, 0, 0, 255],
    &[255, 255, 255, 0],
];
const T: [&[u8]; 7] = [&[255; 3]; 7];
const M: [&[u8]; 2] = [&[255; 6], &[200; 6]];

impl GlyphSource for Odd {
    fn cell_width(&self) -> usize {
        CELL_W
    }

    fn glyph(&self, character: char) -> Option<Glyph<'_>> {
        match character {
            'I' => Glyph::new(3, &I),
            'h' => Glyph::new(2, &H),
            'B' => Glyph::new(4, &B),
            'T' => Glyph::new(3, &T),
            'M' => Glyph::new(6, &M),
            _ => None,
        }
    }
}

/// A font with a glyph for every printable ASCII character, each a different
/// pattern, so the shell's text is visible in the pixels.
pub struct Pattern {
    glyphs: Vec<&'static [&'static [u8]]>,
}

impl Pattern {
    pub fn new() -> Self {
        let glyphs = (32u8..127)
            .map(|code| {
                let rows: Vec<&'static [u8]> = (0..CELL_H - 1)
                    .map(|y| {
                        let row: Vec<u8> = (0..CELL_W)
                            .map(|x| {
                                if code == b' ' {
                                    0
                                } else {
                                    ((usize::from(code) * 7 + x * 31 + y * 17) % 255 + 1) as u8
                                }
                            })
                            .collect();
                        &*Box::leak(row.into_boxed_slice())
                    })
                    .collect();
                &*Box::leak(rows.into_boxed_slice())
            })
            .collect();
        Self { glyphs }
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphSource for Pattern {
    fn cell_width(&self) -> usize {
        CELL_W
    }

    fn glyph(&self, character: char) -> Option<Glyph<'_>> {
        let index = (character as usize).checked_sub(32)?;
        Glyph::new(CELL_W, self.glyphs.get(index)?)
    }
}

fn pack(colour: Rgb) -> u32 {
    (u32::from(colour.red) << 16) | (u32::from(colour.green) << 8) | u32::from(colour.blue)
}

fn unpack(word: u32) -> Rgb {
    Rgb {
        red: (word >> 16) as u8,
        green: (word >> 8) as u8,
        blue: word as u8,
    }
}

/// What `words` should hold after the display has put `cells`, worked out one
/// pixel at a time with no shared code beyond the colour blend.
pub fn expected(
    words: &[u32],
    (width, height): (usize, usize),
    area: Rect,
    clip: Rect,
    font: &impl GlyphSource,
    cells: &[(u16, u16, Cell)],
) -> Vec<u32> {
    let stride = width + 2;
    let mut out = words.to_vec();
    let surface_rect = Rect::new(0, 0, width as u32, height as u32);
    for &(col, row, cell) in cells {
        let left = area.x + (usize::from(col) * CELL_W) as i32;
        let top = area.y + (usize::from(row) * CELL_H) as i32;
        let cell_rect = Rect::new(left, top, CELL_W as u32, CELL_H as u32);
        let allowed = cell_rect
            .intersect(area)
            .intersect(clip)
            .intersect(surface_rect);

        let text = if cell.style.bold {
            PALETTE.bold
        } else {
            PALETTE.foreground
        };
        let (background, ink) = if cell.style.inverse {
            (text, PALETTE.background)
        } else {
            (PALETTE.background, text)
        };
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                if allowed.contains(x, y) {
                    out[y as usize * stride + x as usize] = pack(background);
                }
            }
        }
        if let Some(glyph) = font.glyph(cell.ch) {
            let spare = (CELL_H.saturating_sub(glyph.height()) / 2) as i32;
            for gy in 0..glyph.height() {
                for gx in 0..glyph.width() {
                    let (x, y) = (left + gx as i32, top + spare + gy as i32);
                    if !allowed.contains(x, y) {
                        continue;
                    }
                    let coverage = glyph.coverage(gx, gy).unwrap();
                    let at = y as usize * stride + x as usize;
                    out[at] = match coverage {
                        0 => out[at],
                        255 => pack(ink),
                        _ => pack(unpack(out[at]).blend(ink, coverage)),
                    };
                }
            }
        }
    }
    out
}

pub fn plain(ch: char) -> Cell {
    Cell::plain(ch)
}

pub fn styled(ch: char, bold: bool, inverse: bool) -> Cell {
    Cell::new(ch, Style { bold, inverse })
}

/// A repeatable stream of numbers.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn bits(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

/// A sink that remembers every rectangle it was told about, and what the
/// surface held at the corner of each when it was told.
#[derive(Default)]
pub struct Recorder {
    pub calls: Vec<(Rect, Option<u32>)>,
}

impl shell_surface::Sink for Recorder {
    fn present(&mut self, surface: &Surface<'_>, dirty: Rect) {
        self.calls
            .push((dirty, surface.word(dirty.x as usize, dirty.y as usize)));
    }
}
