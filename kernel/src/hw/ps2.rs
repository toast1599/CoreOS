/// PS/2 keyboard driver.
///
/// Keyboard modifier state (shift, caps-lock) is encapsulated in
/// `KeyboardState` rather than exposed as raw mutable statics.
use core::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

#[inline]
pub unsafe fn read_data() -> u8 {
    let data: u8;
    core::arch::asm!("in al, 0x60", out("al") data, options(nostack, nomem));
    data
}

// ---------------------------------------------------------------------------
// Key events
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum KeyEvent {
    Press(KeyCode),
    Release(KeyCode),
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Printable { normal: char, shifted: char },
    Enter,
    Backspace,
    Space,
    Shift,
    CapsLock,
    Unknown,
}

// ---------------------------------------------------------------------------
// Keyboard state
// ---------------------------------------------------------------------------

pub struct KeyboardState {
    shift: AtomicBool,
    caps_lock: AtomicBool,
}

impl KeyboardState {
    pub const fn new() -> Self {
        Self {
            shift: AtomicBool::new(false),
            caps_lock: AtomicBool::new(false),
        }
    }

    #[inline]
    fn shift(&self) -> bool {
        self.shift.load(Ordering::Relaxed)
    }

    #[inline]
    fn caps_lock(&self) -> bool {
        self.caps_lock.load(Ordering::Relaxed)
    }

    #[inline]
    fn set_shift(&self, v: bool) {
        self.shift.store(v, Ordering::Relaxed);
    }

    #[inline]
    fn toggle_caps(&self) {
        let cur = self.caps_lock.load(Ordering::Relaxed);
        self.caps_lock.store(!cur, Ordering::Relaxed);
    }
}

pub static KBD_STATE: KeyboardState = KeyboardState::new();

// ---------------------------------------------------------------------------
// Scancode → KeyEvent
// ---------------------------------------------------------------------------

pub fn decode_scancode(scancode: u8) -> KeyEvent {
    let release = scancode & 0x80 != 0;
    let code = scancode & 0x7F;

    let key = match code {
        // modifiers
        0x2A | 0x36 => KeyCode::Shift,
        0x3A => KeyCode::CapsLock,

        // control
        0x1C => KeyCode::Enter,
        0x0E => KeyCode::Backspace,
        0x39 => KeyCode::Space,

        // number row
        0x02 => KeyCode::Printable {
            normal: '1',
            shifted: '!',
        },
        0x03 => KeyCode::Printable {
            normal: '2',
            shifted: '@',
        },
        0x04 => KeyCode::Printable {
            normal: '3',
            shifted: '#',
        },
        0x05 => KeyCode::Printable {
            normal: '4',
            shifted: '$',
        },
        0x06 => KeyCode::Printable {
            normal: '5',
            shifted: '%',
        },
        0x07 => KeyCode::Printable {
            normal: '6',
            shifted: '^',
        },
        0x08 => KeyCode::Printable {
            normal: '7',
            shifted: '&',
        },
        0x09 => KeyCode::Printable {
            normal: '8',
            shifted: '*',
        },
        0x0A => KeyCode::Printable {
            normal: '9',
            shifted: '(',
        },
        0x0B => KeyCode::Printable {
            normal: '0',
            shifted: ')',
        },

        // symbols
        0x0C => KeyCode::Printable {
            normal: '-',
            shifted: '_',
        },
        0x0D => KeyCode::Printable {
            normal: '=',
            shifted: '+',
        },

        0x1A => KeyCode::Printable {
            normal: '[',
            shifted: '{',
        },
        0x1B => KeyCode::Printable {
            normal: ']',
            shifted: '}',
        },

        0x27 => KeyCode::Printable {
            normal: ';',
            shifted: ':',
        },
        0x28 => KeyCode::Printable {
            normal: '\'',
            shifted: '"',
        },

        0x29 => KeyCode::Printable {
            normal: '`',
            shifted: '~',
        },

        0x2B => KeyCode::Printable {
            normal: '\\',
            shifted: '|',
        },

        0x33 => KeyCode::Printable {
            normal: ',',
            shifted: '<',
        },
        0x34 => KeyCode::Printable {
            normal: '.',
            shifted: '>',
        },
        0x35 => KeyCode::Printable {
            normal: '/',
            shifted: '?',
        },

        // letters
        sc @ 0x10..=0x32 => {
            let c = map_alpha(sc);
            if c != '\0' {
                KeyCode::Printable {
                    normal: c,
                    shifted: c.to_ascii_uppercase(),
                }
            } else {
                KeyCode::Unknown
            }
        }

        _ => KeyCode::Unknown,
    };

    let event = if release {
        KeyEvent::Release(key)
    } else {
        KeyEvent::Press(key)
    };

    apply_state(event);
    event
}

// ---------------------------------------------------------------------------
// State updates
// ---------------------------------------------------------------------------

fn apply_state(event: KeyEvent) {
    match event {
        KeyEvent::Press(KeyCode::Shift) => KBD_STATE.set_shift(true),
        KeyEvent::Release(KeyCode::Shift) => KBD_STATE.set_shift(false),
        KeyEvent::Press(KeyCode::CapsLock) => KBD_STATE.toggle_caps(),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// KeyEvent → char
// ---------------------------------------------------------------------------

pub fn keyevent_to_char(event: KeyEvent) -> Option<char> {
    let KeyEvent::Press(key) = event else {
        return None;
    };

    match key {
        KeyCode::Printable { normal, shifted } => {
            let shift = KBD_STATE.shift();

            if normal.is_ascii_alphabetic() {
                let upper = shift ^ KBD_STATE.caps_lock();
                Some(if upper { shifted } else { normal })
            } else {
                Some(if shift { shifted } else { normal })
            }
        }
        KeyCode::Enter => Some('\n'),
        KeyCode::Backspace => Some('\x08'),
        KeyCode::Space => Some(' '),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn map_alpha(sc: u8) -> char {
    match sc {
        0x1E => 'a',
        0x30 => 'b',
        0x2E => 'c',
        0x20 => 'd',
        0x12 => 'e',
        0x21 => 'f',
        0x22 => 'g',
        0x23 => 'h',
        0x17 => 'i',
        0x24 => 'j',
        0x25 => 'k',
        0x26 => 'l',
        0x32 => 'm',
        0x31 => 'n',
        0x18 => 'o',
        0x19 => 'p',
        0x10 => 'q',
        0x13 => 'r',
        0x1F => 's',
        0x14 => 't',
        0x16 => 'u',
        0x2F => 'v',
        0x11 => 'w',
        0x2D => 'x',
        0x15 => 'y',
        0x2C => 'z',
        _ => '\0',
    }
}

