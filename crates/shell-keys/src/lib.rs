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
