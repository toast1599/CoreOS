pub static mut SH_PRESSED: bool = false;
pub static mut CAPS_LOCK: bool = false;

pub unsafe fn read_status() -> u8 {
    let status: u8;
    core::arch::asm!("in al, 0x64", out("al") status);
    status
}

pub unsafe fn read_data() -> u8 {
    let data: u8;
    core::arch::asm!("in al, 0x60", out("al") data);
    data
}

pub fn scancode_to_char(scancode: u8) -> char {
    unsafe {
        match scancode {
            0x2A | 0x36 => { SH_PRESSED = true; '\0' }
            0xAA | 0xB6 => { SH_PRESSED = false; '\0' }
            0x3A => { CAPS_LOCK = !CAPS_LOCK; '\0' }

            0x39 => ' ',
            0x1C => '\n',
            0x0E => '\x08',

            0x02 => if SH_PRESSED { '!' } else { '1' },
            0x03 => if SH_PRESSED { '@' } else { '2' },
            0x04 => if SH_PRESSED { '#' } else { '3' },
            0x05 => if SH_PRESSED { '$' } else { '4' },
            0x06 => if SH_PRESSED { '%' } else { '5' },
            0x07 => if SH_PRESSED { '^' } else { '6' },
            0x08 => if SH_PRESSED { '&' } else { '7' },
            0x09 => if SH_PRESSED { '*' } else { '8' },
            0x0A => if SH_PRESSED { '(' } else { '9' },
            0x0B => if SH_PRESSED { ')' } else { '0' },

            0x0C => if SH_PRESSED { '_' } else { '-' },
            0x0D => if SH_PRESSED { '+' } else { '=' },

            sc @ 0x10..=0x32 => {
                let is_upper = SH_PRESSED ^ CAPS_LOCK;
                match sc {
                    0x1E => if is_upper { 'A' } else { 'a' },
                    0x30 => if is_upper { 'B' } else { 'b' },
                    0x2E => if is_upper { 'C' } else { 'c' },
                    0x20 => if is_upper { 'D' } else { 'd' },
                    0x12 => if is_upper { 'E' } else { 'e' },
                    0x21 => if is_upper { 'F' } else { 'f' },
                    0x22 => if is_upper { 'G' } else { 'g' },
                    0x23 => if is_upper { 'H' } else { 'h' },
                    0x17 => if is_upper { 'I' } else { 'i' },
                    0x24 => if is_upper { 'J' } else { 'j' },
                    0x25 => if is_upper { 'K' } else { 'k' },
                    0x26 => if is_upper { 'L' } else { 'l' },
                    0x32 => if is_upper { 'M' } else { 'm' },
                    0x31 => if is_upper { 'N' } else { 'n' },
                    0x18 => if is_upper { 'O' } else { 'o' },
                    0x19 => if is_upper { 'P' } else { 'p' },
                    0x10 => if is_upper { 'Q' } else { 'q' },
                    0x13 => if is_upper { 'R' } else { 'r' },
                    0x1F => if is_upper { 'S' } else { 's' },
                    0x14 => if is_upper { 'T' } else { 't' },
                    0x16 => if is_upper { 'U' } else { 'u' },
                    0x2F => if is_upper { 'V' } else { 'v' },
                    0x11 => if is_upper { 'W' } else { 'w' },
                    0x2D => if is_upper { 'X' } else { 'x' },
                    0x15 => if is_upper { 'Y' } else { 'y' },
                    0x2C => if is_upper { 'Z' } else { 'z' },
                    _ => '\0',
                }
            }

            _ => '\0',
        }
    }
}
