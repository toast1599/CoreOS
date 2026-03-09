pub unsafe fn write_byte(b: u8) {
    core::arch::asm!("out dx, al", in("dx") 0x3F8, in("al") b);
}

pub unsafe fn write_str(s: &str) {
    for b in s.bytes() {
        write_byte(b);
    }
}

