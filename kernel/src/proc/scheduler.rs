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

/// Voluntarily yield the CPU to the next task.
pub unsafe fn yield_now() {
    let in_syscall = IN_SYSCALL.load(Ordering::Relaxed);
    IN_SYSCALL.store(false, Ordering::Relaxed);
    try_switch();
    IN_SYSCALL.store(in_syscall, Ordering::Relaxed);
}

pub unsafe fn wait_for_event(spins: usize) {
    let in_syscall = IN_SYSCALL.load(Ordering::Relaxed);
    IN_SYSCALL.store(false, Ordering::Relaxed);
    core::arch::asm!("sti", options(nostack, nomem));
    for _ in 0..spins {
        core::hint::spin_loop();
    }
    core::arch::asm!("cli", options(nostack, nomem));
    IN_SYSCALL.store(in_syscall, Ordering::Relaxed);
}

// TICKS is now centrally located in hw::pit::TICKS

/// Called from the PIT interrupt handler (pit_handler in pit.rs).
pub fn tick() {
    let t = crate::hw::pit::ticks();

    if t % 10 == 0 {
        // switch every 10 ticks
        unsafe {
            try_switch();
        }
    }
}

unsafe fn try_switch() {
    if IN_SYSCALL.load(Ordering::Relaxed) {
        return;
    }
    if let Some((old_rsp_ptr, new_rsp, new_pml4)) = super::task::next_task_switch() {
        switch_to(old_rsp_ptr, new_rsp, new_pml4);
    }
}

/// Low-level context switch.
///
/// Calling convention: ts lowk a normal Rust `extern "C"` function.
/// It pushes/pops callee-saved regs (rbp, rbx, r12–r15) manually so
/// that each task's stack contains exactly a `task::Context` when suspended.
///
/// Stack layout after the pushes (growing downward):
///   [rsp+40] rbp
///   [rsp+32] rbx
///   [rsp+24] r12   (wait — push order matters, see asm below)
///   ...
///   [rsp+ 0] r15
///
/// The `ret` at the end pops the return address that was pushed when
/// `switch_to` was first called for this task — i.e., the task entry point
/// for a brand-new task, or the instruction after `switch_to` for a
/// previously-running task.
#[unsafe(naked)]
unsafe extern "C" fn switch_to(old_rsp: *mut usize, new_rsp: usize, new_pml4: usize) {
    // rdi = old_rsp pointer, rsi = new_rsp value, rdx = new_pml4  (System V ABI)
    core::arch::naked_asm!(
        // Save callee-saved registers onto current stack
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // Save current rsp into *old_rsp
        "mov [rdi], rsp",
        // Restore the target task's address space
        "mov cr3, rdx",
        // Switch to new stack
        "mov rsp, rsi",
        // Restore callee-saved registers from new stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        // Re-enable interrupts — iretq never runs when we switch stacks,
        // so rflags.IF is never restored. Do it manually.
        "sti",
        // Return to new task (pops saved rip)
        "ret",
    );
}
