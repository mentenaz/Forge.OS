//! What the desktop looks like, in pixels.

use forge_desktop::{Desktop, DesktopShell, shell_palette, theme};
use forge_font::NotoMono;
use pixel::{Channel, PixelFormat, Rgb};
use surface::{Rect, Surface};

const UPTIME_NS: u64 = 102_000_000_000;

fn format() -> PixelFormat {
    PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8)).unwrap()
}

fn pack(colour: Rgb) -> u32 {
    format().pack(colour)
}

fn render(width: usize, height: usize, demo: bool) -> Vec<u32> {
    let mut words = vec![0; width * height];
    let mut surface = Surface::new(&mut words, format(), width, height, width * 4).unwrap();
    let desktop = Desktop::new(width, height);
    if demo {
        let mut shell = Box::new(DesktopShell::new());
        desktop.draw_demo(&mut surface, &NotoMono, &mut shell, UPTIME_NS);
    } else {
        desktop.draw_chrome(&mut surface, &NotoMono);
    }
    words
}

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

#[test]
fn the_chrome_has_the_colours_of_the_theme_where_the_layout_puts_them() {
    let words = render(800, 480, false);
    let at = |x: usize, y: usize| words[y * 800 + x];

    // The top bar, its rule, and the gradient below it.
    assert_eq!(at(400, 10), pack(theme::BAR));
    assert_eq!(at(400, 28), pack(theme::EDGE));
    let step = (470 * 255 / 480) as u8;
    assert_eq!(
        at(5, 470),
        pack(theme::SKY_TOP.blend(theme::SKY_BOTTOM, step))
    );
    assert_eq!(
        at(5, 29),
        pack(theme::SKY_TOP.blend(theme::SKY_BOTTOM, (29 * 255 / 480) as u8))
    );

    // The shell window: border, title bar, body, and its three buttons.
    assert_eq!(at(80, 200), pack(theme::EDGE), "left border");
    assert_eq!(at(639, 200), pack(theme::EDGE), "right border");
    assert_eq!(at(300, 64), pack(theme::EDGE), "top border");
    assert_eq!(at(300, 383), pack(theme::EDGE), "bottom border");
    assert_eq!(at(300, 70), pack(theme::TITLE), "title bar");
    assert_eq!(at(300, 94), pack(theme::BODY), "just below the title bar");
    assert_eq!(at(95, 78), pack(theme::CLOSE));
    assert_eq!(at(115, 78), pack(theme::MINIMISE));
    assert_eq!(at(135, 78), pack(theme::ZOOM));
    assert_eq!(
        at(85, 300),
        pack(theme::BODY),
        "the margin beside the text area"
    );

    // The text area is left as the window's body.
    assert_eq!(at(100, 200), pack(theme::BODY));
    assert_eq!(at(600, 330), pack(theme::BODY));

    // The button: accent below the highlight, the lighter outline around it.
    assert_eq!(at(550, 360), pack(theme::ACCENT));
    assert_eq!(at(544, 350), pack(theme::ACCENT_EDGE));
    assert_ne!(
        at(550, 345),
        pack(theme::ACCENT),
        "the highlight covers the top half"
    );

    // The notes window runs off the right and bottom edges without wrapping.
    assert_eq!(at(799, 330), pack(theme::BODY));
    assert_eq!(at(700, 479), pack(theme::BODY));
    assert_eq!(at(650, 400), pack(theme::EDGE), "its left border");
    assert_eq!(
        at(5, 330),
        pack(theme::SKY_TOP.blend(theme::SKY_BOTTOM, (330 * 255 / 480) as u8))
    );
}

#[test]
fn the_title_and_the_clock_are_drawn_in_the_bar() {
    let bare = render(800, 480, false);
    let ink = |x0: usize, x1: usize| {
        (0..28)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| bare[y * 800 + x] != pack(theme::BAR))
            .count()
    };
    assert!(ink(10, 80) > 40, "Forge.OS");
    assert!(ink(740, 790) > 20, "the clock");
    assert_eq!(ink(300, 500), 0, "nothing between them");
}

#[test]
fn the_demo_fills_the_text_area_and_nothing_else() {
    let (width, height) = (800, 480);
    let bare = render(width, height, false);
    let demo = render(width, height, true);
    let area = Desktop::new(width, height).shell_area();
    let mut changed_inside = 0;
    for y in 0..height {
        for x in 0..width {
            let (a, b) = (bare[y * width + x], demo[y * width + x]);
            if area.contains(x as i32, y as i32) {
                changed_inside += usize::from(a != b);
            } else {
                assert_eq!(a, b, "{x},{y} is outside the text area");
            }
        }
    }
    assert!(
        changed_inside > 1500,
        "the session drew only {changed_inside} pixels"
    );
}

#[test]
fn the_demo_text_uses_the_shell_palette() {
    let demo = render(800, 480, true);
    let palette = shell_palette();
    let area = Desktop::new(800, 480).shell_area();
    let (mut background, mut foreground, mut antialiased) = (0, 0, 0);
    for y in area.y..area.bottom() as i32 {
        for x in area.x..area.right() as i32 {
            let word = demo[y as usize * 800 + x as usize];
            if word == pack(palette.background) {
                background += 1;
            } else if word == pack(palette.foreground) {
                foreground += 1;
            } else {
                antialiased += 1;
            }
        }
    }
    // Mostly empty, some solid ink, and the soft edges of letters in between.
    assert!(background > 100_000, "{background}");
    assert!(foreground > 50, "{foreground}");
    assert!(antialiased > 1_000, "{antialiased}");
}

#[test]
fn drawing_on_a_screen_far_smaller_than_the_layout_does_not_panic_or_hang() {
    for (width, height) in [(1, 1), (7, 3), (40, 30), (100, 60)] {
        for layout in [(width, height), (usize::MAX, usize::MAX), (5000, 3000)] {
            let mut words = vec![0; width * height];
            let mut surface = Surface::new(&mut words, format(), width, height, width * 4).unwrap();
            let desktop = Desktop::new(layout.0, layout.1);
            desktop.draw_chrome(&mut surface, &NotoMono);
            let mut shell = Box::new(DesktopShell::new());
            desktop.draw_demo(&mut surface, &NotoMono, &mut shell, 0);
        }
    }
}

#[test]
fn drawing_stays_on_the_surface_when_the_screen_has_padding() {
    // A pitch wider than the picture: the padding words must never be written.
    let (width, height, stride) = (800usize, 480usize, 808usize);
    let mut words = vec![0xAAAA_AAAA_u32; stride * height];
    let mut surface = Surface::new(&mut words, format(), width, height, stride * 4).unwrap();
    let mut shell = Box::new(DesktopShell::new());
    Desktop::new(width, height).draw_demo(&mut surface, &NotoMono, &mut shell, UPTIME_NS);
    for y in 0..height {
        for x in width..stride {
            assert_eq!(words[y * stride + x], 0xAAAA_AAAA, "{x},{y}");
        }
    }
    let _ = Rect::EMPTY;
}

/// The whole picture, fingerprinted, at the three sizes that matter. A number
/// changes when the picture does: look at the new picture, then update it.
#[test]
fn the_scene_is_the_picture_we_looked_at() {
    for (width, height, golden) in [
        (800, 480, 16_229_775_929_036_131_055),
        (1280, 800, 3_796_066_497_514_460_833),
        (1024, 768, 9_923_528_318_739_660_341),
    ] {
        let got = fingerprint(&render(width, height, true));
        assert_eq!(got, golden, "{width}x{height}");
    }
}

#[test]
fn drawing_the_same_scene_twice_gives_the_same_picture() {
    assert_eq!(render(1024, 768, true), render(1024, 768, true));
}
