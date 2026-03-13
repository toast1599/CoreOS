/// PIT (8253/8254) driver — 100 Hz timer.
///
/// `TICKS` is the single authoritative tick counter for the whole kernel.
/// `scheduler::tick()` is called from the interrupt handler here; the
/// scheduler no longer maintains its own counter.
use crate::scheduler;
use core::sync::atomic::{AtomicU64, Ordering};

/// Kernel tick counter. Incremented every PIT interrupt (100 Hz).
pub static TICKS: AtomicU64 = AtomicU64::new(0);

/// Return the current tick count.
#[inline]
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// PIT interrupt handler — called from the `pit_interrupt` ASM stub in idt.rs.
/// Sends EOI to the PIC, then increments the tick counter and runs the scheduler.
#[no_mangle]
pub extern "C" fn pit_handler(_stack: *mut u8) {
    unsafe {
        crate::hw::pic::eoi(0);
    }
    TICKS.fetch_add(1, Ordering::Relaxed);
    scheduler::tick();
}
/// Yielding wait for `wait_ticks` ticks.
pub fn sleep_yield(wait_ticks: u64) {
    let start = ticks();
    while ticks() - start < wait_ticks {
        unsafe {
            scheduler::yield_now();
        }
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
    }
}

/// Initialise the PIT to fire at 100 Hz.
pub unsafe fn init() {
    // Divisor for 100 Hz: 1193182 / 100 ≈ 11931
    let divisor = (1_193_182_u32 / 100) as u16;
    core::arch::asm!("out 0x43, al", in("al") 0x36u8, options(nostack, nomem));
    core::arch::asm!("out 0x40, al", in("al") (divisor & 0xFF) as u8, options(nostack, nomem));
    core::arch::asm!("out 0x40, al", in("al") (divisor >> 8)  as u8, options(nostack, nomem));
}

/// Seconds since boot (approximate, based on tick count).
pub fn uptime_seconds() -> u64 {
    ticks() / 100
}
