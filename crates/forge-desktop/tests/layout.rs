//! Where everything goes, at the screen sizes the kernel boots on and some it does not.

use forge_desktop::{CELL_HEIGHT, Desktop};
use forge_font::NotoMono;
use platform::{Display, Size};
use shell_surface::{NoSink, Palette, SurfaceDisplay};
use surface::{Rect, Surface};

use pixel::{Channel, PixelFormat};

fn format() -> PixelFormat {
    PixelFormat::new(Channel::new(16, 8), Channel::new(8, 8), Channel::new(0, 8)).unwrap()
}

#[test]
fn the_preview_size_lays_out_as_it_always_did() {
    let desktop = Desktop::new(800, 480);
    assert_eq!(desktop.shell_window(), Rect::new(80, 64, 560, 320));
    assert_eq!(desktop.shell_area(), Rect::new(93, 107, 534, 224));
    assert_eq!(desktop.notes_window(), Rect::new(650, 300, 300, 200));
    assert_eq!(desktop.button(), Rect::new(544, 340, 80, 28));
}

#[test]
fn qemus_screen_lays_out_in_proportion() {
    let desktop = Desktop::new(1280, 800);
    assert_eq!(desktop.shell_window(), Rect::new(128, 64, 896, 533));
    assert_eq!(desktop.shell_area(), Rect::new(141, 107, 870, 437));
    assert_eq!(desktop.notes_window(), Rect::new(1040, 500, 480, 333));
    assert_eq!(desktop.button(), Rect::new(928, 553, 80, 28));
}

#[test]
fn vmwares_screen_lays_out_in_proportion() {
    let desktop = Desktop::new(1024, 768);
    assert_eq!(desktop.shell_window(), Rect::new(102, 64, 716, 512));
    assert_eq!(desktop.shell_area(), Rect::new(115, 107, 690, 416));
    assert_eq!(desktop.notes_window(), Rect::new(832, 480, 384, 320));
    assert_eq!(desktop.button(), Rect::new(722, 532, 80, 28));
}

/// How many cells of text the shell area holds, asked of the real display.
fn grid(width: usize, height: usize) -> Size {
    let desktop = Desktop::new(width, height);
    let mut words = vec![0; width * height];
    let mut surface = Surface::new(&mut words, format(), width, height, width * 4).unwrap();
    let palette = Palette::new(
        pixel::Rgb::from_hex(0),
        pixel::Rgb::from_hex(0),
        pixel::Rgb::from_hex(0),
    );
    SurfaceDisplay::new(
        &mut surface,
        &NoopFont,
        CELL_HEIGHT,
        desktop.shell_area(),
        palette,
        NoSink,
    )
    .size()
}

/// Noto Sans Mono's width with no glyphs, to count cells without drawing any.
struct NoopFont;

impl surface::GlyphSource for NoopFont {
    fn cell_width(&self) -> usize {
        surface::GlyphSource::cell_width(&NotoMono)
    }

    fn glyph(&self, _: char) -> Option<surface::Glyph<'_>> {
        None
    }
}

#[test]
fn the_text_area_holds_these_grids() {
    assert_eq!(grid(800, 480), Size::new(76, 11));
    assert_eq!(grid(1280, 800), Size::new(124, 21));
    assert_eq!(grid(1024, 768), Size::new(98, 20));
}

#[test]
fn the_shell_can_hold_the_grid_of_every_real_screen() {
    // The shell's grid is fixed at compile time; the biggest screen must fit.
    for (width, height) in [(800, 480), (1024, 768), (1280, 800), (1920, 1080)] {
        let size = grid(width, height);
        assert!(
            size.cols <= 216 && size.rows <= 64,
            "{width}x{height}: {size:?}"
        );
    }
}

#[test]
fn the_line_height_is_the_fonts_plus_four() {
    assert_eq!(CELL_HEIGHT, forge_font::NotoMono::LINE_HEIGHT + 4);
}

#[test]
fn the_text_area_is_inside_the_shell_window_inside_the_screen() {
    for (width, height) in [(800, 480), (1024, 768), (1280, 800), (1920, 1080)] {
        let desktop = Desktop::new(width, height);
        let screen = Rect::new(0, 0, width as u32, height as u32);
        let window = desktop.shell_window();
        let area = desktop.shell_area();
        assert_eq!(window.intersect(screen), window, "{width}x{height}");
        assert_eq!(area.intersect(window), area, "{width}x{height}");
        // The button is inside the window and below the text.
        assert_eq!(desktop.button().intersect(window), desktop.button());
        assert!(i64::from(area.y) + i64::from(area.height) <= i64::from(desktop.button().y));
    }
}

#[test]
fn the_notes_window_hangs_off_the_right_and_bottom_edges_on_purpose() {
    for (width, height) in [(800, 480), (1024, 768), (1280, 800)] {
        let notes = Desktop::new(width, height).notes_window();
        assert!(notes.right() > width as i64, "{width}x{height}");
        assert!(notes.bottom() > height as i64, "{width}x{height}");
        assert!(notes.x > 0 && notes.y > 0);
    }
}

#[test]
fn the_layout_never_fails_on_odd_sizes() {
    for (width, height) in [
        (0, 0),
        (1, 1),
        (10, 10),
        (100, 60),
        (200, 100),
        (5000, 3000),
        (usize::MAX, usize::MAX),
        (0, usize::MAX),
    ] {
        let desktop = Desktop::new(width, height);
        // Asking for the rectangles must not panic, and none may be negative in size.
        for rect in [
            desktop.shell_window(),
            desktop.shell_area(),
            desktop.notes_window(),
            desktop.button(),
        ] {
            assert!(rect.right() >= i64::from(rect.x), "{width}x{height}");
        }
    }
}
