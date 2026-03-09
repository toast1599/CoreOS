pub unsafe fn get_time() -> (u8, u8, u8) {
    core::arch::asm!("cli", options(nostack, nomem));
    let h = decode_bcd(read_port(0x04));
    let m = decode_bcd(read_port(0x02));
    let s = decode_bcd(read_port(0x00));
    core::arch::asm!("sti", options(nostack, nomem));
    (h, m, s)
}

unsafe fn read_port(reg: u8) -> u8 {
    core::arch::asm!("out 0x70, al", in("al") reg);
    let mut val: u8;
    core::arch::asm!("in al, 0x71", out("al") val);
    val
}

fn decode_bcd(val: u8) -> u8 {
    (val & 0x0F) + ((val / 16) * 10)
}

