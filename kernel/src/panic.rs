use crate::drivers::serial;
#[cfg(not(test))]
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        serial::write_str("[PERNEL KANIC] ");
    }
    if let Some(msg) = info.message().as_str() {
        unsafe {
            serial::write_str(msg);
        }
    }
    if let Some(loc) = info.location() {
        crate::serial_fmt!(" @ {}:{}\n", loc.file(), loc.line());
    } else {
        unsafe {
            serial::write_str("\n");
        }
    }
    unsafe {
        // Disable interrupts and shut down via QEMU magic port.
        core::arch::asm!("cli");
        core::arch::asm!(
            "out dx, ax",
            in("dx") 0x604u16,
            in("ax") 0x2000u16,
            options(nostack, nomem)
        );
        loop {
            core::arch::asm!("hlt");
        }
    }
}

#[repr(C)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub padding: u64,
    // Pushed by stub / CPU in a normalized order.
    pub vector: u64,
    pub error_code: u64,
    // Pushed by CPU
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[no_mangle]
pub extern "C" fn default_exception(frame: &InterruptFrame) {
    unsafe {
        core::arch::asm!("sti");
        crate::serial_fmt!(
            "\n[EXCEPTION] fault #{} in task (RIP={:#x}, ERR={:#x})\n",
            frame.vector,
            frame.rip,
            frame.error_code
        );
        crate::serial_fmt!(
            "RAX={:#018x} RBX={:#018x} RCX={:#018x} RDX={:#018x}\n",
            frame.rax,
            frame.rbx,
            frame.rcx,
            frame.rdx
        );
        crate::serial_fmt!(
            "RSI={:#018x} RDI={:#018x} RBP={:#018x} RSP={:#018x}\n",
            frame.rsi,
            frame.rdi,
            frame.rbp,
            frame.rsp
        );
        crate::serial_fmt!(
            "R8 ={:#018x} R9 ={:#018x} R10={:#018x} R11={:#018x}\n",
            frame.r8,
            frame.r9,
            frame.r10,
            frame.r11
        );
        crate::serial_fmt!(
            "R12={:#018x} R13={:#018x} R14={:#018x} R15={:#018x}\n",
            frame.r12,
            frame.r13,
            frame.r14,
            frame.r15
        );
        crate::serial_fmt!(
            "CS ={:#x} SS ={:#x} RFLAGS={:#x}\n",
            frame.cs,
            frame.ss,
            frame.rflags
        );

        // Mark process as exited with error code so kernel shell can reap it
        crate::proc::exit(frame.vector as i64);
        if let Some(slot) = crate::proc::task::current_task_slot() {
            crate::proc::task::kill_task(slot);
        }
    }
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
