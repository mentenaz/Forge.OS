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
