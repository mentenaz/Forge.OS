//! The real shell, drawn on a surface one event at a time.
//!
//! The shell redraws only the rows that changed. After every event the picture
//! must still equal a drawing of the whole grid from scratch, which is true only
//! if the shell marks the right rows and the display draws each cell fully.

mod support;

use platform::{Cell, Display, Event, Key, KeyEvent, KeyState, Modifiers, Size, Style};
use shell_core::Shell;
use shell_surface::{NoSink, SurfaceDisplay};
use support::*;
use surface::Rect;

const COLS: usize = 12;
const ROWS: usize = 6;
const W: usize = 56;
const H: usize = 36;
const AREA: Rect = Rect::new(4, 3, 48, 30);

type TestShell = Shell<COLS, ROWS, 32>;

fn key(key: Key, ctrl: bool) -> Event {
    Event::Key(KeyEvent {
        key,
        state: KeyState::Down,
        modifiers: Modifiers {
            ctrl,
            alt: false,
            shift: false,
        },
    })
}

/// Draw whatever the shell says changed.
fn draw(shell: &mut TestShell, words: &mut [u32], font: &Pattern) -> bool {
    let mut surface = surface(words, W, H);
    let mut display = SurfaceDisplay::new(&mut surface, font, CELL_H, AREA, PALETTE, NoSink);
    shell.render(&mut display)
}

/// The whole grid, drawn from nothing, from what the shell's terminal holds.
fn whole_grid(shell: &TestShell, font: &Pattern) -> Vec<u32> {
    let term = shell.terminal();
    let (cursor_col, cursor_row) = term.cursor();
    let mut cells = Vec::new();
    for row in 0..ROWS {
        for col in 0..COLS {
            let style = Style {
                bold: false,
                inverse: col == cursor_col && row == cursor_row,
            };
            cells.push((
                col as u16,
                row as u16,
                Cell::new(term.char_at(col, row).unwrap(), style),
            ));
        }
    }
    expected(
        &buffer(W, H),
        (W, H),
        AREA,
        Rect::new(0, 0, W as u32, H as u32),
        font,
        &cells,
    )
}

struct Session {
    shell: TestShell,
    words: Vec<u32>,
    font: Pattern,
    steps: usize,
}

impl Session {
    fn start() -> Self {
        let mut session = Self {
            shell: TestShell::new(),
            words: buffer(W, H),
            font: Pattern::new(),
            steps: 0,
        };
        let mut probe = buffer(W, H);
        let mut surface = surface(&mut probe, W, H);
        let size =
            SurfaceDisplay::new(&mut surface, &session.font, CELL_H, AREA, PALETTE, NoSink).size();
        assert_eq!(size, Size::new(COLS as u16, ROWS as u16));
        session.shell.begin(size);
        session.check("after begin");
        session
    }

    fn check(&mut self, what: &str) {
        draw(&mut self.shell, &mut self.words, &self.font);
        assert_eq!(self.words, whole_grid(&self.shell, &self.font), "{what}");
        self.steps += 1;
    }

    fn send(&mut self, event: Event, what: &str) {
        self.shell.feed(event, 83_025_000_000);
        self.check(what);
    }

    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.send(
                key(Key::Char(ch), false),
                &format!("typing {ch:?} in {text:?}"),
            );
        }
    }

    fn press(&mut self, k: Key, what: &str) {
        self.send(key(k, false), what);
    }

    fn row_text(&self, row: usize) -> String {
        (0..COLS)
            .map(|col| self.shell.terminal().char_at(col, row).unwrap())
            .collect()
    }
}

#[test]
fn the_picture_equals_a_full_redraw_after_every_event_of_a_real_session() {
    let mut s = Session::start();
    s.type_text("help");
    s.press(Key::Enter, "enter after help");
    s.type_text("uptime");
    s.press(Key::Enter, "enter after uptime");
    s.type_text("echo hi");
    s.press(Key::Backspace, "backspace");
    s.press(Key::Backspace, "backspace");
    s.type_text("x");
    s.press(Key::Enter, "enter after echo");
    s.send(key(Key::Char('l'), true), "ctrl+l clears the screen");
    s.type_text("a line longer than the grid is wide");
    s.press(Key::Left, "left");
    s.press(Key::Home, "home");
    s.press(Key::Delete, "delete");
    s.press(Key::End, "end");
    s.press(Key::Enter, "enter after the long line");
    s.type_text("nosuchcommand");
    s.press(Key::Enter, "enter after an unknown command");
    for n in 0..8 {
        s.press(Key::Enter, &format!("enter number {n}, scrolling"));
    }
    s.send(key(Key::Char('c'), true), "ctrl+c abandons the line");
    assert!(
        s.steps > 60,
        "the session was too short to mean much: {}",
        s.steps
    );
}

#[test]
fn the_shell_text_is_really_on_the_screen() {
    let mut s = Session::start();
    s.type_text("echo hi");
    s.press(Key::Enter, "enter");
    assert!(
        (0..ROWS).any(|row| s.row_text(row).starts_with("hi")),
        "the shell printed 'hi' somewhere: {:?}",
        (0..ROWS).map(|row| s.row_text(row)).collect::<Vec<_>>()
    );
    // 'h' and 'i' are drawn with different patterns, so their cells differ in pixels.
    let row = (0..ROWS)
        .find(|&row| s.row_text(row).starts_with("hi"))
        .unwrap();
    let cell_pixels = |col: usize| {
        let x = AREA.x as usize + col * CELL_W;
        let y = AREA.y as usize + row * CELL_H;
        (0..CELL_H)
            .flat_map(|dy| (0..CELL_W).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| s.words[(y + dy) * (W + 2) + x + dx])
            .collect::<Vec<_>>()
    };
    assert_ne!(cell_pixels(0), cell_pixels(1));
    assert_ne!(
        cell_pixels(0),
        cell_pixels(5),
        "text differs from an empty cell"
    );
}

#[test]
fn nothing_outside_the_area_is_touched_by_a_whole_session() {
    let mut s = Session::start();
    s.type_text("help");
    s.press(Key::Enter, "enter");
    for y in 0..H {
        for x in 0..W + 2 {
            let inside = x < W && AREA.contains(x as i32, y as i32);
            if !inside {
                let want = if x < W { BASE } else { PAD };
                assert_eq!(s.words[y * (W + 2) + x], want, "{x},{y}");
            }
        }
    }
}
