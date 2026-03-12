/// exec.rs — Load an ELF and drop into ring 3.
///
/// This is the point of no return. Once `exec` is called the kernel
/// hands control to userspace. The only way back is via a syscall or
/// an interrupt.
use crate::gdt::{SEG_UCODE, SEG_UDATA};
use crate::pmm::{alloc_frame, PAGE_SIZE};

/// Number of pages to allocate for the user stack.
const USER_STACK_PAGES: usize = 4; // 16 KB

/// Load `elf_data` and jump to its entry point in ring 3.
///
/// Never returns. If the ELF fails to load, halts with a serial error.
pub unsafe fn exec(elf_data: &[u8]) -> ! {
    // -----------------------------------------------------------------------
    // 1. Load the ELF into memory
    // -----------------------------------------------------------------------
    let entry = match crate::elf::load(elf_data) {
        Ok(e) => e,
        Err(err) => {
            crate::dbg_log!("EXEC", "ELF load failed: {:?}", err);
            loop {
                core::arch::asm!("hlt");
            }
        }
    };

    crate::dbg_log!("EXEC", "ELF entry={:#x}", entry);

    // -----------------------------------------------------------------------
    // 2. Allocate user stack (4 pages = 16 KB)
    // -----------------------------------------------------------------------
    let mut stack_top: usize = 0;

    for i in 0..USER_STACK_PAGES {
        let frame = alloc_frame();
        if frame == 0 {
            crate::dbg_log!("EXEC", "OOM allocating user stack page {}", i);
            loop {
                core::arch::asm!("hlt");
            }
        }
        let top = frame + PAGE_SIZE;
        if top > stack_top {
            stack_top = top;
        }
    }

    // Align to 16 bytes, minus 8 for ABI alignment at call sites.
    stack_top = (stack_top & !0xF) - 8;

    crate::dbg_log!("EXEC", "user stack top={:#x}", stack_top);

    // -----------------------------------------------------------------------
    // 3. Fresh kernel stack for syscall entry via TSS.rsp0
    // -----------------------------------------------------------------------
    let syscall_stack = alloc_frame();
    if syscall_stack == 0 {
        crate::dbg_log!("EXEC", "OOM allocating syscall stack");
        loop {
            core::arch::asm!("hlt");
        }
    }
    let syscall_stack_top = (syscall_stack + PAGE_SIZE) as u64;
    crate::gdt::TSS.rsp0 = syscall_stack_top;
    crate::gdt::TSS_RSP0 = syscall_stack_top;
    crate::dbg_log!("EXEC", "syscall stack top={:#x}", syscall_stack_top);

    // -----------------------------------------------------------------------
    // 4. Drop to ring 3 via iretq (naked function — no compiler interference)
    // -----------------------------------------------------------------------
    let user_cs: u64 = (SEG_UCODE | 3) as u64; // 0x23
    let user_ss: u64 = (SEG_UDATA | 3) as u64; // 0x1b
    let rflags: u64 = 0x202; // IF=1, reserved bit 1

    crate::dbg_log!(
        "EXEC",
        "iretq frame: rip={:#x} cs={:#x} rflags={:#x} rsp={:#x} ss={:#x}",
        entry,
        user_cs,
        rflags,
        stack_top as u64,
        user_ss
    );

    jump_to_userspace(entry, stack_top as u64, user_cs, user_ss, rflags);
}

/// Naked trampoline — builds the iretq frame with no compiler-generated
/// prologue/epilogue so we have full control over the stack at iretq time.
///
/// Win64 calling convention:
///   rcx = rip, rdx = rsp, r8 = cs, r9 = ss, stack+32 = rflags
#[unsafe(naked)]
unsafe extern "win64" fn jump_to_userspace(
    rip: u64,    // rcx
    rsp: u64,    // rdx
    cs: u64,     // r8
    ss: u64,     // r9
    rflags: u64, // [rsp+32] (5th arg on stack in win64)
) -> ! {
    core::arch::naked_asm!(
        "cli",
        "mov rax, [rsp + 40]", // rflags is 5th arg — win64 shadow space is 32 bytes, so +32+8 for return addr = 40
        "push r9",             // SS
        "push rdx",            // RSP
        "push rax",            // RFLAGS
        "push r8",             // CS
        "push rcx",            // RIP
        "iretq",
    );
}

