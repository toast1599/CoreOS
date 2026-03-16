extern "C" fn page_fault_handler(stack: &crate::arch::amd64::idt::InterruptStackFrame, error: u64) {
    let addr: u64;

    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) addr);
    }

    crate::dbg_log!(
        "PAGE",
        "fault addr={:#x} rip={:#x} err={:#x}",
        addr,
        stack.rip,
        error
    );

    loop {}
}

