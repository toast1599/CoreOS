pub mod rtc;
pub mod ps2;
pub mod pit;
pub mod kbd_buffer;

pub unsafe fn reboot() -> ! {
    // Pulse the CPU reset line via the PS/2 controller
    loop {
        let status: u8;
        core::arch::asm!("in al, 0x64", out("al") status);
        if (status & 0x02) == 0 {
            core::arch::asm!("out 0x64, al", in("al") 0xFEu8);
        }
    }
}

#[allow(dead_code)]
pub unsafe fn shutdown() -> ! {
    // Magic exit for QEMU. On real hardware, this will just hang (safe fallback).
    core::arch::asm!("out dx, ax", in("dx") 0x604u16, in("ax") 0x2000u16);
    loop { core::arch::asm!("cli; hlt"); }
}
