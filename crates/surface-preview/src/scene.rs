//! The first scene: a top bar and a shell window on a dark desktop.
//!
//! It is drawn with the public surface API only, the way a UI layer will, so
//! what shows here is what the kernel framebuffer will show.

use pixel::{Channel, PixelFormat, Rgb};
use platform::{Display, Event, Key, KeyEvent, KeyState, Modifiers};
use shell_core::Shell;
use shell_surface::{NoSink, Palette, SurfaceDisplay};
use surface::{Rect, Surface};

use crate::font::NotoMono;

/// Width of the preview picture.
pub const WIDTH: usize = 800;
/// Height of the preview picture.
pub const HEIGHT: usize = 480;

/// What the shell's `uptime` command is told the machine has been running.
const UPTIME_NS: u64 = 102_000_000_000;

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

    // The shell runs in a text area inside the window, above the button.
    let area = Rect::new(frame.x + 13, frame.y + 43, frame.width - 26, 224);
    let palette = Palette::new(BODY, TEXT, Rgb::from_hex(0xffffff));
    let mut display = SurfaceDisplay::new(
        &mut *surface,
        &font,
        NotoMono::LINE_HEIGHT + 4,
        area,
        palette,
        NoSink,
    );
    let mut shell: Shell<80, 24, 80> = Shell::new();
    shell.begin(display.size());
    for line in ["help", "uptime", "echo hello from Forge.OS"] {
        for ch in line.chars() {
            shell.feed(key(Key::Char(ch)), UPTIME_NS);
        }
        shell.feed(key(Key::Enter), UPTIME_NS);
    }
    for ch in "ech".chars() {
        shell.feed(key(Key::Char(ch)), UPTIME_NS);
    }
    shell.render(&mut display);

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

fn key(key: Key) -> Event {
    Event::Key(KeyEvent {
        key,
        state: KeyState::Down,
        modifiers: Modifiers::default(),
    })
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
    const GOLDEN: u64 = 16_229_775_929_036_131_055;

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
        // The shell's text stops inside the window: its right border is still the
        // border on every row of the text area.
        for row in [64 + 60, 64 + 100, 64 + 150, 64 + 200] {
            assert_eq!(at(80 + 560 - 1, row), pack(EDGE), "row {row}");
        }
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
