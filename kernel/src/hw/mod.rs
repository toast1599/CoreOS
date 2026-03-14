pub mod kbd_buffer;
pub mod keyboard;
pub mod pic;
pub mod pit;
pub mod ps2;
pub mod rtc;

/// Reboot the machine by pulsing the CPU reset line via the PS/2 controller.
pub unsafe fn reboot() -> ! {
    loop {
        let status: u8;
        core::arch::asm!("in al, 0x64", out("al") status, options(nostack, nomem));
        if (status & 0x02) == 0 {
            core::arch::asm!("out 0x64, al", in("al") 0xFEu8, options(nostack, nomem));
        }
    }
}

/// QEMU magic-port shutdown. On real hardware this hangs safely.
#[allow(dead_code)]
pub unsafe fn shutdown() -> ! {
    core::arch::asm!(
        "out dx, ax",
        in("dx") 0x604u16,
        in("ax") 0x2000u16,
        options(nostack, nomem)
    );
    loop {
        core::arch::asm!("cli; hlt");
    }
}

