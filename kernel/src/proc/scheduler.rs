/// Scheduler
///
/// Preemptive round-robin. The PIT fires at 100 Hz and calls `pit_handler`,
/// which calls `scheduler::tick()` every tick and triggers a context switch
/// every SWITCH_INTERVAL ticks.
///
/// `switch_to(old_rsp: *mut usize, new_rsp: usize)` is a naked asm function:
///   - Pushes callee-saved registers onto the CURRENT stack
///   - Saves rsp into *old_rsp
///   - Loads new_rsp into rsp
///   - Pops callee-saved registers from the NEW stack
///   - Returns → lands at the new task's saved rip
use core::sync::atomic::{AtomicBool, Ordering};

#[no_mangle]
pub static IN_SYSCALL: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Voluntarily yield the CPU to the next task.
pub fn yield_now() {
    let prev = IN_SYSCALL.swap(false, Ordering::Relaxed);

    unsafe {
        try_switch();
    }

    IN_SYSCALL.store(prev, Ordering::Relaxed);
}

/// Wait while allowing interrupts (used for blocking-style waits).
pub fn wait_for_event(spins: usize) {
    let prev = IN_SYSCALL.swap(false, Ordering::Relaxed);

    unsafe {
        enable_interrupts();
    }

    for _ in 0..spins {
        core::hint::spin_loop();
    }

    unsafe {
        disable_interrupts();
    }

    IN_SYSCALL.store(prev, Ordering::Relaxed);
}

/// Called from PIT interrupt handler.
pub fn tick() {
    let t = crate::hw::pit::ticks();

    // switch every 10 ticks
    if t % 10 == 0 {
        unsafe {
            try_switch();
        }
    }
}

// ---------------------------------------------------------------------------
// Internal scheduling
// ---------------------------------------------------------------------------

#[inline]
unsafe fn try_switch() {
    // Do not preempt inside syscall critical section
    if IN_SYSCALL.load(Ordering::Relaxed) {
        return;
    }

    if let Some((old_rsp_ptr, new_rsp, new_pml4, new_fs_base)) = super::task::next_task_switch() {
        switch_to(old_rsp_ptr, new_rsp, new_pml4, new_fs_base);
    }
}

// ---------------------------------------------------------------------------
// Interrupt helpers (isolated unsafe)
// ---------------------------------------------------------------------------

#[inline(always)]
unsafe fn enable_interrupts() {
    core::arch::asm!("sti", options(nostack, nomem));
}

#[inline(always)]
unsafe fn disable_interrupts() {
    core::arch::asm!("cli", options(nostack, nomem));
}

// ---------------------------------------------------------------------------
// Context switch (the dangerous part)
// ---------------------------------------------------------------------------

/// Low-level context switch.
///
/// SAFETY:
/// - Caller must ensure `old_rsp` is valid
/// - `new_rsp` must point to a valid saved context
/// - `new_pml4` must be a valid page table
/// - `new_fs_base` must be valid for the target task
#[unsafe(naked)]
unsafe extern "C" fn switch_to(
    old_rsp: *mut usize,
    new_rsp: usize,
    new_pml4: usize,
    new_fs_base: u64,
) {
    core::arch::naked_asm!(
        // Save callee-saved registers
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // Save current stack pointer
        "mov [rdi], rsp",
        // Switch address space
        "mov cr3, rdx",
        // Write FS.base (TLS)
        "mov eax, ecx",
        "shr rcx, 32",
        "mov edx, ecx",
        "mov ecx, 0xC0000100",
        "wrmsr",
        // Switch stack
        "mov rsp, rsi",
        // Restore callee-saved registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        // Re-enable interrupts (explicit)
        "sti",
        // Jump into new task
        "ret",
    );
}

