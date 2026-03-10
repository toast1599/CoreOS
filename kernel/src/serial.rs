use core::fmt::{self, Write};

pub struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            unsafe {
                write_byte(b);
            }
        }
        Ok(())
    }
}

pub unsafe fn write_byte(b: u8) {
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b, options(nostack, nomem));
}

pub unsafe fn write_str(s: &str) {
    for b in s.bytes() {
        write_byte(b);
    }
}

#[macro_export]
macro_rules! serial_fmt {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut w = $crate::serial::SerialWriter;
        let _ = core::write!(w, $($arg)*);
    }};
}

