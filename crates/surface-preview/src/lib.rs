//! Draw a scene with the `surface` crate and save it as a picture.
//!
//! This is the host half of the loop: the same drawing code that will run in
//! the kernel runs here, and the result is a PNG you can open.

pub mod font;
pub mod png;
pub mod scene;
