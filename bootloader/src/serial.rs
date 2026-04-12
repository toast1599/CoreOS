use core::arch::asm;

const COM1: u16 = 0x3F8;

pub fn init() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1 + 0, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xC7);
        outb(COM1 + 4, 0x0B);
    }
}

pub fn log(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

pub fn log_hex(prefix: &str, value: u64) {
    log(prefix);
    log("0x");
    for shift in (0..16).rev() {
        let nibble = ((value >> (shift * 4)) & 0xF) as u8;
        write_byte(if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + (nibble - 10)
        });
    }
    log("\n");
}

pub fn log_dec(prefix: &str, mut value: u64) {
    log(prefix);
    if value == 0 {
        log("0\n");
        return;
    }

    let mut buf = [0u8; 20];
    let mut idx = buf.len();
    while value > 0 {
        idx -= 1;
        buf[idx] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    for byte in &buf[idx..] {
        write_byte(*byte);
    }
    log("\n");
}

pub fn log_status(prefix: &str, status: uefi::Status) {
    log_hex(prefix, status.0 as u64);
}

fn write_byte(byte: u8) {
    unsafe {
        while (inb(COM1 + 5) & 0x20) == 0 {}
        outb(COM1, byte);
    }
}

unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", in("dx") port, out("al") value, options(nostack, nomem, preserves_flags));
    value
}
