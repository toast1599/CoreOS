/// PS/2 keyboard driver.
///
/// Keyboard modifier state (shift, caps-lock) is encapsulated in
/// `KeyboardState` rather than exposed as raw mutable statics.
use core::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// I/O helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[inline]
pub unsafe fn read_status() -> u8 {
    let status: u8;
    core::arch::asm!("in al, 0x64", out("al") status, options(nostack, nomem));
    status
}

#[inline]
pub unsafe fn read_data() -> u8 {
    let data: u8;
    core::arch::asm!("in al, 0x60", out("al") data, options(nostack, nomem));
    data
}

// ---------------------------------------------------------------------------
// Keyboard modifier state
// ---------------------------------------------------------------------------

/// Tracks shift and caps-lock state using atomics — safe to read from
/// interrupt context without disabling interrupts.
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
// Scancode → char translation
// ---------------------------------------------------------------------------

/// Translate a PS/2 Set-1 scancode into a character.
/// Returns `'\0'` for non-printable or modifier keys.
pub fn scancode_to_char(scancode: u8) -> char {
    let state = &KBD_STATE;

    match scancode {
        // Shift press / release
        0x2A | 0x36 => {
            state.set_shift(true);
            '\0'
        }
        0xAA | 0xB6 => {
            state.set_shift(false);
            '\0'
        }

        // Caps Lock toggle
        0x3A => {
            state.toggle_caps();
            '\0'
        }

        // Whitespace / control
        0x39 => ' ',
        0x1C => '\n',
        0x0E => '\x08', // backspace

        // Number row (unshifted / shifted)
        0x02 => {
            if state.shift() {
                '!'
            } else {
                '1'
            }
        }
        0x03 => {
            if state.shift() {
                '@'
            } else {
                '2'
            }
        }
        0x04 => {
            if state.shift() {
                '#'
            } else {
                '3'
            }
        }
        0x05 => {
            if state.shift() {
                '$'
            } else {
                '4'
            }
        }
        0x06 => {
            if state.shift() {
                '%'
            } else {
                '5'
            }
        }
        0x07 => {
            if state.shift() {
                '^'
            } else {
                '6'
            }
        }
        0x08 => {
            if state.shift() {
                '&'
            } else {
                '7'
            }
        }
        0x09 => {
            if state.shift() {
                '*'
            } else {
                '8'
            }
        }
        0x0A => {
            if state.shift() {
                '('
            } else {
                '9'
            }
        }
        0x0B => {
            if state.shift() {
                ')'
            } else {
                '0'
            }
        }
        0x0C => {
            if state.shift() {
                '_'
            } else {
                '-'
            }
        }
        0x0D => {
            if state.shift() {
                '+'
            } else {
                '='
            }
        }

        // Alpha keys
        sc @ 0x10..=0x32 => {
            let upper = state.shift() ^ state.caps_lock();
            alpha_scancode(sc, upper)
        }

        // Unknown make codes — log to serial
        sc if sc < 0x80 => {
            crate::serial_fmt!("[KBD] unmapped scancode: {:#04x}\n", sc);
            '\0'
        }

        // Break codes (key release) — silently ignore
        _ => '\0',
    }
}

/// Map an alpha scancode to its character, applying case.
fn alpha_scancode(sc: u8, upper: bool) -> char {
    let ch = match sc {
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
        _ => return '\0',
    };
    if upper {
        (ch as u8 - b'a' + b'A') as char
    } else {
        ch
    }
}
