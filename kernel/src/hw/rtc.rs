pub unsafe fn get_time() -> (u8, u8, u8) {
    // Wait until not updating
    while is_updating() {}

    let mut h1;
    let mut m1;
    let mut s1;

    let mut h2;
    let mut m2;
    let mut s2;

    loop {
        // First read
        h1 = decode_bcd(read_port(0x04));
        m1 = decode_bcd(read_port(0x02));
        s1 = decode_bcd(read_port(0x00));

        // Wait again
        while is_updating() {}

        // Second read
        h2 = decode_bcd(read_port(0x04));
        m2 = decode_bcd(read_port(0x02));
        s2 = decode_bcd(read_port(0x00));

        // If stable → done
        if h1 == h2 && m1 == m2 && s1 == s2 {
            return (h1, m1, s1);
        }
    }
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

unsafe fn is_updating() -> bool {
    core::arch::asm!("out 0x70, al", in("al") 0x0Au8);
    let mut status: u8;
    core::arch::asm!("in al, 0x71", out("al") status);
    (status & 0x80) != 0
}
