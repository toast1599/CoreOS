use core::arch::asm;

#[inline]
pub unsafe fn cli() {
    asm!("cli", options(nomem, nostack));
}

#[inline]
pub unsafe fn sti() {
    asm!("sti", options(nomem, nostack));
}

#[inline]
pub unsafe fn interrupts_enabled() -> bool {
    let rflags: u64;
    asm!(
        "pushfq",
        "pop {}",
        out(reg) rflags,
        options(nomem, preserves_flags)
    );
    (rflags & (1 << 9)) != 0
}

#[inline]
pub unsafe fn restore_interrupts(enabled: bool) {
    if enabled {
        sti();
    } else {
        cli();
    }
}
