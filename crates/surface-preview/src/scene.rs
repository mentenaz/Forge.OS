//! The desktop scene, rendered into a buffer on the host.

use forge_desktop::{Desktop, DesktopShell};
use forge_font::NotoMono;
use pixel::{Channel, PixelFormat};
use surface::Surface;

/// Width of the default preview picture.
pub const WIDTH: usize = 800;
/// Height of the default preview picture.
pub const HEIGHT: usize = 480;

/// What the shell's `uptime` command is told the machine has been running.
const UPTIME_NS: u64 = 102_000_000_000;

/// The pixel format of the preview buffer: `0x00RRGGBB`, like QEMU's display.
pub fn format() -> PixelFormat {
    PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8))
        .expect("valid channel masks")
}

/// Render the desktop with a scripted shell session, `width` by `height`
/// pixels, and return the packed pixel words. Returns `None` if the size is
/// not a valid surface.
pub fn render(width: usize, height: usize) -> Option<Vec<u32>> {
    let mut words = vec![0; width.checked_mul(height)?];
    let mut surface =
        Surface::new(&mut words, format(), width, height, width.checked_mul(4)?).ok()?;
    // The shell's grid is large, so it lives on the heap, as it will in the kernel.
    let mut shell = Box::new(DesktopShell::new());
    Desktop::new(width, height).draw_demo(&mut surface, &NotoMono, &mut shell, UPTIME_NS);
    Some(words)
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

    /// The default picture, fingerprinted. It is the same number the desktop's
    /// own test holds for 800 by 480, so the preview is the desktop and nothing more.
    #[test]
    fn the_default_preview_is_the_desktop_we_looked_at() {
        assert_eq!(
            fingerprint(&render(WIDTH, HEIGHT).unwrap()),
            16_229_775_929_036_131_055
        );
    }

    #[test]
    fn the_buffer_is_as_big_as_asked() {
        assert_eq!(render(320, 200).unwrap().len(), 320 * 200);
    }

    #[test]
    fn a_size_that_is_not_a_surface_is_refused() {
        assert!(render(0, 10).is_none());
        assert!(render(10, 0).is_none());
        assert!(render(usize::MAX, 2).is_none());
    }

    #[test]
    fn rgb_bytes_come_out_red_green_blue() {
        assert_eq!(
            to_rgb(&[0x0011_2233, 0x00ab_cdef]),
            [0x11, 0x22, 0x33, 0xab, 0xcd, 0xef]
        );
    }
}
