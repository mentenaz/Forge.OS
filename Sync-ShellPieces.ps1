<#
.SYNOPSIS
  Step 0: brings Forge.OS level with the Bedrock fork's shell pieces.

.DESCRIPTION
  Adds CellGeometry to `platform`, the `shell-keys` crate (with its tests), and
  #[derive(Debug)] on the shell types, then registers shell-keys in the workspace.
  Every edit is checked first: it is skipped if already applied, and the script stops
  without writing that file if the text it expects is not there.

.EXAMPLE
  pwsh -File .\Sync-ShellPieces.ps1
#>
param([string]$ForgeRoot = 'E:\Forge.OS')
$ErrorActionPreference = 'Stop'
if (-not (Test-Path (Join-Path $ForgeRoot 'Cargo.toml'))) { throw "No Cargo.toml in $ForgeRoot." }
$utf8 = [System.Text.UTF8Encoding]::new($false)

function Write-Source([string]$Relative, [string]$Text) {
  $path = Join-Path $ForgeRoot $Relative
  New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
  [System.IO.File]::WriteAllText($path, (($Text -replace "`r`n", "`n") + "`n"), $utf8)
  Write-Host "  wrote   $Relative"
}

# Replace $Old with $New once. Keeps the file's own line endings.
function Edit-Source([string]$Relative, [string]$Old, [string]$New) {
  $path = Join-Path $ForgeRoot $Relative
  $raw = [System.IO.File]::ReadAllText($path)
  $crlf = $raw.Contains("`r`n")
  $text = $raw -replace "`r`n", "`n"
  if ($text.Contains($New)) { Write-Host "  already $Relative"; return }
  $first = $text.IndexOf($Old, [System.StringComparison]::Ordinal)
  if ($first -lt 0 -or $text.IndexOf($Old, $first + 1, [System.StringComparison]::Ordinal) -ge 0) {
    throw "$Relative does not contain the expected text exactly once - not changed. Paste the file and I will adapt the script."
  }
  $text = $text.Substring(0, $first) + $New + $text.Substring($first + $Old.Length)
  if ($crlf) { $text = $text -replace "`n", "`r`n" }
  [System.IO.File]::WriteAllText($path, $text, $utf8)
  Write-Host "  edited  $Relative"
}

Write-Host "Adding the new files..."
Write-Source 'crates\platform\src\geometry.rs' @'
//! How a grid of character cells lies on a pixel framebuffer.
//!
//! A pixel backend needs two answers: how many cells fit on the screen, and
//! where a given cell starts. Both are arithmetic, so they live here where the
//! host can test them, and the backend keeps only the pixel writes.

use crate::Size;

/// A cell grid laid out with the same margin on all four sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellGeometry {
    margin: usize,
    cell_width: usize,
    cell_height: usize,
}

impl CellGeometry {
    /// A layout with `margin` pixels around the grid and cells of
    /// `cell_width` by `cell_height` pixels.
    pub const fn new(margin: usize, cell_width: usize, cell_height: usize) -> Self {
        Self {
            margin,
            cell_width,
            cell_height,
        }
    }

    /// The largest grid that fits a `width` by `height` pixel framebuffer
    /// with the margin kept on every side.
    ///
    /// A framebuffer too small for one cell, or a cell with no size, gives an
    /// empty grid. Each side is capped at `u16::MAX` cells.
    pub const fn fit(&self, width: usize, height: usize) -> Size {
        Size::new(
            fit_axis(width, self.margin, self.cell_width),
            fit_axis(height, self.margin, self.cell_height),
        )
    }

    /// The pixel position of the top left corner of a cell, or `None` if the
    /// position does not fit in a `usize`.
    pub fn origin(&self, col: usize, row: usize) -> Option<(usize, usize)> {
        let x = self.cell_width.checked_mul(col)?.checked_add(self.margin)?;
        let y = self
            .cell_height
            .checked_mul(row)?
            .checked_add(self.margin)?;
        Some((x, y))
    }
}

const fn fit_axis(pixels: usize, margin: usize, cell: usize) -> u16 {
    if cell == 0 {
        return 0;
    }
    let cells = pixels.saturating_sub(margin.saturating_mul(2)) / cell;
    if cells > u16::MAX as usize {
        u16::MAX
    } else {
        cells as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typical_framebuffer_fits_whole_cells_inside_its_margins() {
        let geometry = CellGeometry::new(8, 9, 18);
        assert_eq!(geometry.fit(1024, 768), Size::new(112, 41));
    }

    #[test]
    fn the_last_cell_fits_exactly_and_one_pixel_less_loses_it() {
        let geometry = CellGeometry::new(8, 9, 18);
        // 2 * 8 + 112 * 9 = 1024 and 2 * 8 + 41 * 18 = 754.
        assert_eq!(geometry.fit(1024, 754).cols, 112);
        assert_eq!(geometry.fit(1023, 754).cols, 111);
        assert_eq!(geometry.fit(1024, 754).rows, 41);
        assert_eq!(geometry.fit(1024, 753).rows, 40);
    }

    #[test]
    fn a_framebuffer_too_small_for_a_cell_gives_an_empty_grid() {
        let geometry = CellGeometry::new(8, 9, 18);
        assert_eq!(geometry.fit(0, 0), Size::new(0, 0));
        assert_eq!(geometry.fit(10, 10), Size::new(0, 0));
        assert_eq!(geometry.fit(16, 16), Size::new(0, 0));
        assert_eq!(geometry.fit(24, 33), Size::new(0, 0));
        assert_eq!(geometry.fit(25, 34), Size::new(1, 1));
    }

    #[test]
    fn a_margin_larger_than_the_framebuffer_is_not_an_underflow() {
        let geometry = CellGeometry::new(usize::MAX, 9, 18);
        assert_eq!(geometry.fit(1024, 768), Size::new(0, 0));
    }

    #[test]
    fn a_cell_with_no_size_gives_an_empty_grid() {
        assert_eq!(CellGeometry::new(8, 0, 18).fit(1024, 768).cols, 0);
        assert_eq!(CellGeometry::new(8, 9, 0).fit(1024, 768).rows, 0);
    }

    #[test]
    fn each_side_is_capped_at_the_largest_size() {
        let geometry = CellGeometry::new(0, 1, 1);
        assert_eq!(
            geometry.fit(1_000_000, 1_000_000),
            Size::new(u16::MAX, u16::MAX)
        );
    }

    #[test]
    fn width_and_height_are_not_mixed_up() {
        let geometry = CellGeometry::new(0, 10, 100);
        assert_eq!(geometry.fit(1000, 1000), Size::new(100, 10));
    }

    #[test]
    fn cells_start_after_the_margin_and_step_by_the_cell_size() {
        let geometry = CellGeometry::new(8, 9, 18);
        assert_eq!(geometry.origin(0, 0), Some((8, 8)));
        assert_eq!(geometry.origin(2, 3), Some((26, 62)));
        assert_eq!(geometry.origin(1, 0), Some((17, 8)));
        assert_eq!(geometry.origin(0, 1), Some((8, 26)));
    }

    #[test]
    fn a_position_beyond_usize_is_refused_on_each_axis() {
        assert_eq!(CellGeometry::new(0, usize::MAX, 1).origin(2, 0), None);
        assert_eq!(CellGeometry::new(0, 1, usize::MAX).origin(0, 2), None);
        assert_eq!(CellGeometry::new(usize::MAX, 1, 1).origin(1, 0), None);
        assert_eq!(CellGeometry::new(usize::MAX, 1, 1).origin(0, 1), None);
    }

    #[test]
    fn every_cell_of_the_fitted_grid_is_on_screen_and_one_more_is_not() {
        let geometry = CellGeometry::new(3, 5, 7);
        for width in 0..120 {
            for height in 0..120 {
                let size = geometry.fit(width, height);
                let (cols, rows) = (usize::from(size.cols), usize::from(size.rows));
                if cols > 0 && rows > 0 {
                    let (x, y) = geometry.origin(cols - 1, rows - 1).unwrap();
                    assert!(x + 5 + 3 <= width, "{width}x{height}");
                    assert!(y + 7 + 3 <= height, "{width}x{height}");
                }
                let (x, _) = geometry.origin(cols, 0).unwrap();
                let (_, y) = geometry.origin(0, rows).unwrap();
                assert!(x + 5 + 3 > width, "{width}x{height}");
                assert!(y + 7 + 3 > height, "{width}x{height}");
            }
        }
    }
}
'@
Write-Source 'crates\shell-keys\Cargo.toml' @'
[package]
name = "shell-keys"
description = "Turns the kernel's decoded keystrokes into the shell's key events. Pure logic, host-testable."
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
ps2-keys.workspace = true
platform.workspace = true

[dev-dependencies]
shell-core.workspace = true
'@
Write-Source 'crates\shell-keys\src\lib.rs' @'
//! Turns decoded keystrokes into the key events the shell understands.
//!
//! The kernel decodes the keyboard into physical keys and, on a key going down,
//! the character that key types under the current Shift state. The shell wants
//! neither a scancode nor a control character: it wants `Enter`, `Backspace`,
//! arrows, and `Char` for what is typed, with the modifiers that were held.
//!
//! [`Translator`] is that step. It takes the two kernel fields separately, so
//! it does not depend on the kernel's own keystroke type.
//!
//! The rules, in the order they apply to a key going down:
//!
//! 1. Modifier keys update the held state and produce nothing.
//! 2. Named keys (Enter, Backspace, Delete, Tab, Escape, the arrows, Home and
//!    End) are decided by the physical key alone. The kernel types `'\n'`,
//!    `'\t'` and `'\u{8}'` for three of them, and those must not reach the
//!    shell as characters.
//! 3. With Ctrl held, only the letters A to Z produce a key, and the letter is
//!    taken from the physical key, because the kernel's character can be a
//!    control code or absent.
//! 4. Otherwise the kernel's character becomes `Key::Char`.
//!
//! Key releases produce nothing, and every other key is dropped.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use platform::{Event, Key, KeyEvent, KeyState, Modifiers};
use ps2_keys::{Key as Physical, KeyEvent as Decoded, KeyState as Wire, Shift};

/// Translates the kernel's decoded keystrokes, remembering which modifier keys
/// are held.
#[derive(Debug, Clone, Copy, Default)]
pub struct Translator {
    shift: Shift,
    ctrl_left: bool,
    ctrl_right: bool,
    alt_left: bool,
    alt_right: bool,
}

impl Translator {
    /// A translator with no modifier held.
    pub const fn new() -> Self {
        Self {
            shift: Shift::new(),
            ctrl_left: false,
            ctrl_right: false,
            alt_left: false,
            alt_right: false,
        }
    }

    /// The modifiers held now.
    pub const fn modifiers(&self) -> Modifiers {
        Modifiers {
            ctrl: self.ctrl_left || self.ctrl_right,
            alt: self.alt_left || self.alt_right,
            shift: self.shift.is_down(),
        }
    }

    /// Fold one decoded keystroke in, returning the shell event it makes, if any.
    ///
    /// `character` is what the kernel resolved for this event; it is only
    /// present for a key going down.
    pub fn translate(&mut self, event: Decoded, character: Option<char>) -> Option<Event> {
        self.observe(event);
        if event.state != Wire::Pressed {
            return None;
        }
        let key = self.key_for(event.key, character)?;
        Some(Event::Key(KeyEvent {
            key,
            state: KeyState::Down,
            modifiers: self.modifiers(),
        }))
    }

    fn observe(&mut self, event: Decoded) {
        self.shift.observe(event);
        let down = event.state == Wire::Pressed;
        match event.key {
            Physical::CtrlLeft => self.ctrl_left = down,
            Physical::CtrlRight => self.ctrl_right = down,
            Physical::AltLeft => self.alt_left = down,
            Physical::AltRight => self.alt_right = down,
            _ => {}
        }
    }

    fn key_for(&self, physical: Physical, character: Option<char>) -> Option<Key> {
        if let Some(named) = named(physical) {
            return Some(named);
        }
        if self.modifiers().ctrl {
            return letter(physical).map(Key::Char);
        }
        character.map(Key::Char)
    }
}

const fn named(key: Physical) -> Option<Key> {
    Some(match key {
        Physical::Enter | Physical::KeypadEnter => Key::Enter,
        Physical::Backspace => Key::Backspace,
        Physical::Delete => Key::Delete,
        Physical::Tab => Key::Tab,
        Physical::Escape => Key::Escape,
        Physical::ArrowLeft => Key::Left,
        Physical::ArrowRight => Key::Right,
        Physical::ArrowUp => Key::Up,
        Physical::ArrowDown => Key::Down,
        Physical::Home => Key::Home,
        Physical::End => Key::End,
        _ => return None,
    })
}

const fn letter(key: Physical) -> Option<char> {
    Some(match key {
        Physical::A => 'a',
        Physical::B => 'b',
        Physical::C => 'c',
        Physical::D => 'd',
        Physical::E => 'e',
        Physical::F => 'f',
        Physical::G => 'g',
        Physical::H => 'h',
        Physical::I => 'i',
        Physical::J => 'j',
        Physical::K => 'k',
        Physical::L => 'l',
        Physical::M => 'm',
        Physical::N => 'n',
        Physical::O => 'o',
        Physical::P => 'p',
        Physical::Q => 'q',
        Physical::R => 'r',
        Physical::S => 's',
        Physical::T => 't',
        Physical::U => 'u',
        Physical::V => 'v',
        Physical::W => 'w',
        Physical::X => 'x',
        Physical::Y => 'y',
        Physical::Z => 'z',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(t: &mut Translator, key: Physical, character: Option<char>) -> Option<Event> {
        t.translate(
            Decoded {
                key,
                state: Wire::Pressed,
            },
            character,
        )
    }

    fn release(t: &mut Translator, key: Physical) -> Option<Event> {
        t.translate(
            Decoded {
                key,
                state: Wire::Released,
            },
            None,
        )
    }

    fn down(key: Key, modifiers: Modifiers) -> Option<Event> {
        Some(Event::Key(KeyEvent {
            key,
            state: KeyState::Down,
            modifiers,
        }))
    }

    const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
    };
    const CTRL: Modifiers = Modifiers { ctrl: true, ..NONE };
    const ALT: Modifiers = Modifiers { alt: true, ..NONE };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..NONE
    };

    #[test]
    fn a_typed_character_becomes_a_char_key() {
        let mut t = Translator::new();
        assert_eq!(
            press(&mut t, Physical::A, Some('a')),
            down(Key::Char('a'), NONE)
        );
        assert_eq!(
            press(&mut t, Physical::Space, Some(' ')),
            down(Key::Char(' '), NONE)
        );
        assert_eq!(
            press(&mut t, Physical::Keypad5, Some('5')),
            down(Key::Char('5'), NONE)
        );
    }

    #[test]
    fn shift_is_reported_and_follows_the_key_state() {
        let mut t = Translator::new();
        assert_eq!(press(&mut t, Physical::ShiftLeft, None), None);
        assert_eq!(
            press(&mut t, Physical::A, Some('A')),
            down(Key::Char('A'), SHIFT)
        );
        assert_eq!(release(&mut t, Physical::ShiftLeft), None);
        assert_eq!(
            press(&mut t, Physical::A, Some('a')),
            down(Key::Char('a'), NONE)
        );
    }

    #[test]
    fn named_keys_ignore_the_character_the_kernel_typed() {
        let mut t = Translator::new();
        assert_eq!(
            press(&mut t, Physical::Enter, Some('\n')),
            down(Key::Enter, NONE)
        );
        assert_eq!(
            press(&mut t, Physical::KeypadEnter, Some('\n')),
            down(Key::Enter, NONE)
        );
        assert_eq!(
            press(&mut t, Physical::Backspace, Some('\u{8}')),
            down(Key::Backspace, NONE)
        );
        assert_eq!(
            press(&mut t, Physical::Tab, Some('\t')),
            down(Key::Tab, NONE)
        );
    }

    #[test]
    fn named_keys_without_a_character_map_by_the_physical_key() {
        let mut t = Translator::new();
        let cases = [
            (Physical::Delete, Key::Delete),
            (Physical::Escape, Key::Escape),
            (Physical::ArrowLeft, Key::Left),
            (Physical::ArrowRight, Key::Right),
            (Physical::ArrowUp, Key::Up),
            (Physical::ArrowDown, Key::Down),
            (Physical::Home, Key::Home),
            (Physical::End, Key::End),
        ];
        for (physical, key) in cases {
            assert_eq!(
                press(&mut t, physical, None),
                down(key, NONE),
                "{physical:?}"
            );
        }
    }

    #[test]
    fn key_releases_produce_nothing() {
        let mut t = Translator::new();
        for key in [
            Physical::A,
            Physical::Enter,
            Physical::Backspace,
            Physical::ArrowUp,
        ] {
            assert_eq!(release(&mut t, key), None, "{key:?}");
        }
    }

    #[test]
    fn modifier_keys_produce_nothing_by_themselves() {
        let mut t = Translator::new();
        for key in [
            Physical::CtrlLeft,
            Physical::CtrlRight,
            Physical::AltLeft,
            Physical::AltRight,
            Physical::ShiftLeft,
            Physical::ShiftRight,
        ] {
            assert_eq!(press(&mut t, key, None), None, "{key:?}");
        }
    }

    #[test]
    fn ctrl_takes_the_letter_from_the_physical_key() {
        let mut t = Translator::new();
        press(&mut t, Physical::CtrlLeft, None);
        assert_eq!(press(&mut t, Physical::C, None), down(Key::Char('c'), CTRL));
        // The kernel's character is ignored under Ctrl, even a misleading one.
        assert_eq!(
            press(&mut t, Physical::L, Some('\u{c}')),
            down(Key::Char('l'), CTRL)
        );
        assert_eq!(
            press(&mut t, Physical::Q, Some('x')),
            down(Key::Char('q'), CTRL)
        );
    }

    #[test]
    fn ctrl_maps_every_letter() {
        let letters = [
            (Physical::A, 'a'),
            (Physical::B, 'b'),
            (Physical::C, 'c'),
            (Physical::D, 'd'),
            (Physical::E, 'e'),
            (Physical::F, 'f'),
            (Physical::G, 'g'),
            (Physical::H, 'h'),
            (Physical::I, 'i'),
            (Physical::J, 'j'),
            (Physical::K, 'k'),
            (Physical::L, 'l'),
            (Physical::M, 'm'),
            (Physical::N, 'n'),
            (Physical::O, 'o'),
            (Physical::P, 'p'),
            (Physical::Q, 'q'),
            (Physical::R, 'r'),
            (Physical::S, 's'),
            (Physical::T, 't'),
            (Physical::U, 'u'),
            (Physical::V, 'v'),
            (Physical::W, 'w'),
            (Physical::X, 'x'),
            (Physical::Y, 'y'),
            (Physical::Z, 'z'),
        ];
        let mut t = Translator::new();
        press(&mut t, Physical::CtrlRight, None);
        for (physical, ch) in letters {
            assert_eq!(
                press(&mut t, physical, None),
                down(Key::Char(ch), CTRL),
                "{physical:?}"
            );
        }
    }

    #[test]
    fn ctrl_with_a_non_letter_produces_nothing() {
        let mut t = Translator::new();
        press(&mut t, Physical::CtrlLeft, None);
        assert_eq!(press(&mut t, Physical::Digit1, Some('1')), None);
        assert_eq!(press(&mut t, Physical::Space, Some(' ')), None);
        assert_eq!(press(&mut t, Physical::Keypad5, Some('5')), None);
    }

    #[test]
    fn named_keys_still_work_under_ctrl() {
        let mut t = Translator::new();
        press(&mut t, Physical::CtrlLeft, None);
        assert_eq!(
            press(&mut t, Physical::Enter, Some('\n')),
            down(Key::Enter, CTRL)
        );
        assert_eq!(
            press(&mut t, Physical::ArrowLeft, None),
            down(Key::Left, CTRL)
        );
    }

    #[test]
    fn alt_is_reported() {
        let mut t = Translator::new();
        press(&mut t, Physical::AltRight, None);
        assert_eq!(
            press(&mut t, Physical::X, Some('x')),
            down(Key::Char('x'), ALT)
        );
        release(&mut t, Physical::AltRight);
        assert_eq!(
            press(&mut t, Physical::X, Some('x')),
            down(Key::Char('x'), NONE)
        );
    }

    #[test]
    fn a_modifier_stays_on_until_both_sides_are_released() {
        let mut t = Translator::new();
        press(&mut t, Physical::CtrlLeft, None);
        press(&mut t, Physical::CtrlRight, None);
        release(&mut t, Physical::CtrlLeft);
        assert!(t.modifiers().ctrl);
        release(&mut t, Physical::CtrlRight);
        assert!(!t.modifiers().ctrl);

        press(&mut t, Physical::AltLeft, None);
        press(&mut t, Physical::AltRight, None);
        release(&mut t, Physical::AltRight);
        assert!(t.modifiers().alt);
        release(&mut t, Physical::AltLeft);
        assert!(!t.modifiers().alt);

        press(&mut t, Physical::ShiftLeft, None);
        press(&mut t, Physical::ShiftRight, None);
        release(&mut t, Physical::ShiftLeft);
        assert!(t.modifiers().shift);
        release(&mut t, Physical::ShiftRight);
        assert!(!t.modifiers().shift);
    }

    #[test]
    fn modifiers_are_independent_of_each_other() {
        let mut t = Translator::new();
        press(&mut t, Physical::CtrlLeft, None);
        press(&mut t, Physical::AltLeft, None);
        press(&mut t, Physical::ShiftLeft, None);
        assert_eq!(
            t.modifiers(),
            Modifiers {
                ctrl: true,
                alt: true,
                shift: true
            }
        );
        release(&mut t, Physical::AltLeft);
        assert_eq!(
            t.modifiers(),
            Modifiers {
                ctrl: true,
                alt: false,
                shift: true
            }
        );
    }

    #[test]
    fn unmapped_keys_and_keys_with_no_character_are_dropped() {
        let mut t = Translator::new();
        for key in [
            Physical::F1,
            Physical::PageUp,
            Physical::Insert,
            Physical::CapsLock,
            Physical::A,
        ] {
            assert_eq!(press(&mut t, key, None), None, "{key:?}");
        }
    }

    #[test]
    fn a_held_key_repeats_as_repeated_presses() {
        let mut t = Translator::new();
        let first = press(&mut t, Physical::Backspace, Some('\u{8}'));
        let second = press(&mut t, Physical::Backspace, Some('\u{8}'));
        assert_eq!(first, down(Key::Backspace, NONE));
        assert_eq!(second, first);
    }
}
'@
Write-Source 'crates\shell-keys\tests\typing.rs' @'
//! Raw PS/2 bytes through the decoder, the translator and the real shell.
//!
//! This is the path a keystroke takes in the kernel, minus the interrupt: the
//! decoder names the key, the US layout resolves its character against the
//! Shift state, the translator makes the shell's event, and the shell acts.

use platform::Size;
use ps2_keys::{Decoder, KeyState, Shift, us};
use shell_core::Shell;
use shell_keys::Translator;

const COLS: usize = 24;
const ROWS: usize = 8;

struct Keyboard {
    decoder: Decoder,
    shift: Shift,
    keys: Translator,
    shell: Shell<COLS, ROWS, COLS>,
}

impl Keyboard {
    fn new() -> Self {
        let mut shell = Shell::new();
        shell.begin(Size::new(COLS as u16, ROWS as u16));
        Self {
            decoder: Decoder::new(),
            shift: Shift::new(),
            keys: Translator::new(),
            shell,
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let Some(event) = self.decoder.feed(byte) else {
                continue;
            };
            self.shift.observe(event);
            let character = (event.state == KeyState::Pressed)
                .then(|| us::char_for(event.key, self.shift))
                .flatten();
            if let Some(shell_event) = self.keys.translate(event, character) {
                self.shell.feed(shell_event, 0);
            }
        }
    }

    fn row(&self, row: usize) -> String {
        (0..COLS)
            .map(|col| self.shell.terminal().char_at(col, row).unwrap())
            .collect::<String>()
            .trim_end()
            .to_string()
    }
}

// Scancode set 2 make codes; a break is 0xF0 followed by the same code.
const E: u8 = 0x24;
const C: u8 = 0x21;
const H: u8 = 0x33;
const O: u8 = 0x44;
const I: u8 = 0x43;
const A: u8 = 0x1C;
const B: u8 = 0x32;
const SPACE: u8 = 0x29;
const ENTER: u8 = 0x5A;
const BACKSPACE: u8 = 0x66;
const SHIFT_LEFT: u8 = 0x12;
const CTRL_LEFT: u8 = 0x14;

fn tap(key: u8) -> [u8; 3] {
    [key, 0xF0, key]
}

fn type_keys(keyboard: &mut Keyboard, keys: &[u8]) {
    for &key in keys {
        keyboard.bytes(&tap(key));
    }
}

#[test]
fn typing_echo_hi_and_enter_prints_hi() {
    let mut keyboard = Keyboard::new();
    type_keys(&mut keyboard, &[E, C, H, O, SPACE, H, I, ENTER]);
    assert_eq!(keyboard.row(1), "> echo hi");
    assert_eq!(keyboard.row(2), "hi");
    assert_eq!(keyboard.row(3), ">");
}

#[test]
fn backspace_edits_the_line_before_enter() {
    let mut keyboard = Keyboard::new();
    type_keys(&mut keyboard, &[E, C, H, O, SPACE, A, B, BACKSPACE, ENTER]);
    assert_eq!(keyboard.row(2), "a");
}

#[test]
fn shift_types_capitals_and_releasing_it_stops() {
    let mut keyboard = Keyboard::new();
    type_keys(&mut keyboard, &[E, C, H, O, SPACE]);
    keyboard.bytes(&[SHIFT_LEFT]);
    type_keys(&mut keyboard, &[H]);
    keyboard.bytes(&[0xF0, SHIFT_LEFT]);
    type_keys(&mut keyboard, &[I, ENTER]);
    assert_eq!(keyboard.row(2), "Hi");
}

#[test]
fn ctrl_c_abandons_the_line() {
    let mut keyboard = Keyboard::new();
    type_keys(&mut keyboard, &[A, B]);
    keyboard.bytes(&[CTRL_LEFT]);
    keyboard.bytes(&tap(C));
    keyboard.bytes(&[0xF0, CTRL_LEFT]);
    assert_eq!(keyboard.shell.line(), "");
}
'@

Write-Host "Editing existing files..."
Edit-Source 'crates\platform\src\lib.rs' @'
#![warn(missing_docs)]

/// A grid size
'@ @'
#![warn(missing_docs)]

mod geometry;

pub use geometry::CellGeometry;

/// A grid size
'@
Edit-Source 'crates\shell-core\src\lib.rs' @'

pub struct Shell<
'@ @'

#[derive(Debug)]
pub struct Shell<
'@
Edit-Source 'crates\shell-core\src\editor.rs' @'

pub struct LineEditor
'@ @'

#[derive(Debug)]
pub struct LineEditor
'@
Edit-Source 'crates\shell-core\src\terminal.rs' @'

pub struct Terminal<
'@ @'

#[derive(Debug)]
pub struct Terminal<
'@
Edit-Source 'Cargo.toml' @'
  "crates/shell-core",
  "crates/shell-host",
  "crates/shell-bedrock",

'@ @'
  "crates/shell-core",
  "crates/shell-keys",
  "crates/shell-host",
  "crates/shell-bedrock",

'@
Edit-Source 'Cargo.toml' @'
default-members = [
  "crates/platform",
  "crates/shell-core",

'@ @'
default-members = [
  "crates/platform",
  "crates/shell-core",
  "crates/shell-keys",

'@
Edit-Source 'Cargo.toml' @'
shell-core = { path = "crates/shell-core" }

'@ @'
shell-core = { path = "crates/shell-core" }
shell-keys = { path = "crates/shell-keys" }

'@

Write-Host ""
Write-Host "Done. Next, from $ForgeRoot :"
Write-Host "  cargo fmt --all"
Write-Host "  cargo clippy --workspace --all-targets -- -D warnings"
Write-Host "  cargo test --workspace"
