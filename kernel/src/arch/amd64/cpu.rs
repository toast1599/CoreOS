use core::arch::asm;

const IA32_FS_BASE: u32 = 0xC000_0100;

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

#[inline]
pub unsafe fn write_fs_base(value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") IA32_FS_BASE,
        in("eax") lo,
        in("edx") hi,
        options(nostack, nomem)
    );
}

#[inline]
pub unsafe fn push_cli() -> bool {
    let enabled = interrupts_enabled();
    if enabled {
        cli();
    }
    enabled
}

#[inline]
pub unsafe fn pop_cli(was_enabled: bool) {
    if was_enabled {
        sti();
    }
}

