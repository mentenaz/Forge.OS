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
