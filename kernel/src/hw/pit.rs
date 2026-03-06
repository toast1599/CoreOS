use core::sync::atomic::{AtomicU64, Ordering};

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn pit_handler(_stack: *mut u8) {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub unsafe fn init_pit() {
    let divisor: u16 = (1193182u32 / 100) as u16;

    core::arch::asm!("out 0x43, al", in("al") 0x36u8);
    core::arch::asm!("out 0x40, al", in("al") (divisor & 0xFF) as u8);
    core::arch::asm!("out 0x40, al", in("al") (divisor >> 8) as u8);
}
