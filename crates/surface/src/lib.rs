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
