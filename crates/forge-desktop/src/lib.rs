//! The Forge.OS desktop: where things go, what they look like, and how to draw
//! them on a [`Surface`].
//!
//! A [`Desktop`] works out the rectangles for a screen of a given size: a top
//! bar, a shell window with its text area and a button, and a second window that
//! hangs off the right edge. It draws all of that without the shell's text
//! ([`Desktop::draw_chrome`]), leaves the text area for a live shell, and can
//! also fill the area with a scripted session ([`Desktop::draw_demo`]).
//!
//! The crate allocates nothing and touches no hardware, so the host preview and
//! the kernel draw the same picture from the same code.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]

use platform::{Display, Event, Key, KeyEvent, KeyState, Modifiers};
use shell_core::Shell;
use shell_surface::{NoSink, Palette, SurfaceDisplay};
use surface::{GlyphSource, Rect, Surface};

/// The colours of the desktop.
pub mod theme {
    use pixel::Rgb;

    /// Top of the background gradient.
    pub const SKY_TOP: Rgb = Rgb::from_hex(0x0f1420);
    /// Bottom of the background gradient.
    pub const SKY_BOTTOM: Rgb = Rgb::from_hex(0x1d2744);
    /// The top bar.
    pub const BAR: Rgb = Rgb::from_hex(0x0b0e17);
    /// Borders and rules.
    pub const EDGE: Rgb = Rgb::from_hex(0x34406a);
    /// The inside of a window.
    pub const BODY: Rgb = Rgb::from_hex(0x151a28);
    /// A window's title bar.
    pub const TITLE: Rgb = Rgb::from_hex(0x1f2740);
    /// Ordinary text.
    pub const TEXT: Rgb = Rgb::from_hex(0xd7def0);
    /// Quiet text: titles and the clock.
    pub const DIM: Rgb = Rgb::from_hex(0x7f8bb0);
    /// The accent colour, used by buttons.
    pub const ACCENT: Rgb = Rgb::from_hex(0x4f7cff);
    /// The lighter outline of an accent button.
    pub const ACCENT_EDGE: Rgb = Rgb::from_hex(0x8fa8ff);
    /// A window's close button.
    pub const CLOSE: Rgb = Rgb::from_hex(0xff5f57);
    /// A window's minimise button.
    pub const MINIMISE: Rgb = Rgb::from_hex(0xfebc2e);
    /// A window's zoom button.
    pub const ZOOM: Rgb = Rgb::from_hex(0x28c840);
    /// Bold text and highlights.
    pub const WHITE: Rgb = Rgb::from_hex(0xffffff);
    /// Shadows.
    pub const BLACK: Rgb = Rgb::from_hex(0x000000);
}

/// Pixels from the top of one line of shell text to the top of the next: the
/// font's 16 pixels plus four of spacing.
pub const CELL_HEIGHT: usize = 20;

/// The shell the desktop hosts: up to 216 columns by 64 rows, which is more than
/// the largest screen the kernel boots on (1280 by 800) holds in the window.
pub type DesktopShell = Shell<216, 64, 216>;

/// The colours the shell's text is drawn in.
pub const fn shell_palette() -> Palette {
    Palette::new(theme::BODY, theme::TEXT, theme::WHITE)
}

const BAR_HEIGHT: u32 = 28;
const WINDOW_TOP: i32 = 64;
const TITLE_HEIGHT: u32 = 30;

/// Where everything goes on a screen of a given size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Desktop {
    width: usize,
    height: usize,
    shell_window: Rect,
    shell_area: Rect,
    notes_window: Rect,
    button: Rect,
}

impl Desktop {
    /// The layout for a `width` by `height` screen.
    ///
    /// Every rectangle is a fixed share of the screen, so the layout grows with
    /// it. A screen too small to hold the windows gives empty or clipped ones;
    /// nothing here can fail.
    pub fn new(width: usize, height: usize) -> Self {
        let shell_window = Rect::new(
            share_i32(width, 1, 10),
            WINDOW_TOP,
            share(width, 7, 10),
            share(height, 2, 3),
        );
        // The text sits 13 pixels in from the sides and 43 below the top, which
        // clears the title bar, and stops 53 above the bottom, which clears the
        // button.
        let shell_area = Rect::new(
            shell_window.x.saturating_add(13),
            shell_window.y.saturating_add(43),
            shell_window.width.saturating_sub(26),
            shell_window.height.saturating_sub(96),
        );
        let notes_window = Rect::new(
            share_i32(width, 13, 16),
            share_i32(height, 5, 8),
            share(width, 3, 8),
            share(height, 5, 12),
        );
        let button = Rect::new(
            clamp(shell_window.right() - 96),
            clamp(shell_window.bottom() - 44),
            80,
            28,
        );
        Self {
            width,
            height,
            shell_window,
            shell_area,
            notes_window,
            button,
        }
    }

    /// The shell's window.
    pub const fn shell_window(&self) -> Rect {
        self.shell_window
    }

    /// The rectangle the shell's text is drawn in. Give it to a
    /// `shell_surface::SurfaceDisplay` with [`CELL_HEIGHT`] and [`shell_palette`].
    pub const fn shell_area(&self) -> Rect {
        self.shell_area
    }

    /// The second window, which runs off the right edge on purpose.
    pub const fn notes_window(&self) -> Rect {
        self.notes_window
    }

    /// The button in the shell window's corner.
    pub const fn button(&self) -> Rect {
        self.button
    }

    /// Draw everything except the shell's text: the background, the top bar,
    /// both windows and the button. The text area is left as the window's body.
    pub fn draw_chrome<F: GlyphSource>(&self, surface: &mut Surface<'_>, font: &F) {
        let width = u32::try_from(self.width).unwrap_or(u32::MAX);

        // A vertical gradient, one row at a time. Rows below the surface are not
        // worth visiting, which also keeps an absurd layout size from looping.
        for y in 0..self.height.min(surface.height()) {
            let step = (y * 255 / self.height.max(1)) as u8;
            let row = Rect::new(0, y as i32, width, 1);
            surface.fill_rect(row, theme::SKY_TOP.blend(theme::SKY_BOTTOM, step));
        }

        // The top bar.
        surface.fill_rect(Rect::new(0, 0, width, BAR_HEIGHT), theme::BAR);
        surface.fill_rect(Rect::new(0, BAR_HEIGHT as i32, width, 1), theme::EDGE);
        surface.draw_text(12, 6, font, "Forge.OS", theme::TEXT);
        let clock = "12:00";
        let clock_width = (clock.len() * font.cell_width()) as i32;
        surface.draw_text(
            (self.width as i32).saturating_sub(clock_width + 12),
            6,
            font,
            clock,
            theme::DIM,
        );

        window(surface, font, self.shell_window, "shell");
        self.draw_button(surface, font);
        // The second window is drawn last and may run off the screen: it must
        // be cut off at the edge, not wrap round.
        window(surface, font, self.notes_window, "notes");
    }

    /// Draw the chrome, then run a short scripted shell session in the text area
    /// so there is something to look at. `shell` is restarted first, and
    /// `uptime_ns` is what its `uptime` command reports.
    pub fn draw_demo<F: GlyphSource>(
        &self,
        surface: &mut Surface<'_>,
        font: &F,
        shell: &mut DesktopShell,
        uptime_ns: u64,
    ) {
        self.draw_chrome(surface, font);
        let mut display = SurfaceDisplay::new(
            surface,
            font,
            CELL_HEIGHT,
            self.shell_area,
            shell_palette(),
            NoSink,
        );
        shell.begin(display.size());
        for line in ["help", "uptime", "echo hello from Forge.OS"] {
            for ch in line.chars() {
                shell.feed(key(Key::Char(ch)), uptime_ns);
            }
            shell.feed(key(Key::Enter), uptime_ns);
        }
        for ch in "ech".chars() {
            shell.feed(key(Key::Char(ch)), uptime_ns);
        }
        shell.render(&mut display);
    }

    /// A button in the shell window's corner, with a translucent highlight
    /// across its top half.
    fn draw_button<F: GlyphSource>(&self, surface: &mut Surface<'_>, font: &F) {
        let button = self.button;
        surface.fill_rect(button, theme::ACCENT);
        surface.blend_rect(
            Rect::new(button.x, button.y, button.width, 14),
            theme::WHITE,
            40,
        );
        surface.stroke_rect(button, 1, theme::ACCENT_EDGE);
        surface.draw_text(
            button.x.saturating_add(24),
            button.y.saturating_add(5),
            font,
            "Run",
            theme::WHITE,
        );
    }
}

/// A window: soft shadow, border, title bar, three buttons and a title.
fn window<F: GlyphSource>(surface: &mut Surface<'_>, font: &F, frame: Rect, title: &str) {
    surface.blend_rect(offset(frame, 10, 14), theme::BLACK, 40);
    surface.blend_rect(offset(frame, 6, 8), theme::BLACK, 70);

    surface.fill_rect(frame, theme::BODY);
    let bar = Rect::new(frame.x, frame.y, frame.width, TITLE_HEIGHT);
    surface.fill_rect(bar, theme::TITLE);
    surface.stroke_rect(frame, 1, theme::EDGE);

    for (index, colour) in [theme::CLOSE, theme::MINIMISE, theme::ZOOM]
        .into_iter()
        .enumerate()
    {
        let x = frame.x.saturating_add(12 + index as i32 * 20);
        surface.fill_rect(Rect::new(x, frame.y.saturating_add(10), 10, 10), colour);
    }
    surface.draw_text(
        frame.x.saturating_add(84),
        frame.y.saturating_add(7),
        font,
        title,
        theme::DIM,
    );
}

fn offset(rect: Rect, dx: i32, dy: i32) -> Rect {
    Rect::new(
        rect.x.saturating_add(dx),
        rect.y.saturating_add(dy),
        rect.width,
        rect.height,
    )
}

fn key(key: Key) -> Event {
    Event::Key(KeyEvent {
        key,
        state: KeyState::Down,
        modifiers: Modifiers::default(),
    })
}

/// `numerator / denominator` of `total`, as a size.
fn share(total: usize, numerator: usize, denominator: usize) -> u32 {
    u32::try_from(total.saturating_mul(numerator) / denominator).unwrap_or(u32::MAX)
}

/// `numerator / denominator` of `total`, as a coordinate.
fn share_i32(total: usize, numerator: usize, denominator: usize) -> i32 {
    i32::try_from(share(total, numerator, denominator)).unwrap_or(i32::MAX)
}

fn clamp(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}
