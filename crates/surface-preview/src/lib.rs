//! Draw the Forge.OS desktop and save it as a picture.
//!
//! This is the host half of the loop: the same drawing code that runs in the
//! kernel runs here, and the result is a PNG you can open.

pub mod png;
pub mod scene;
