use core::arch::global_asm;

// MSR addresses
const MSR_EFER: u32 = 0xC000_0080;
const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;

unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") lo,
        in("edx") hi,
        options(nostack, nomem)
    );
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") lo,
        out("edx") hi,
        options(nostack, nomem)
    );
    ((hi as u64) << 32) | lo as u64
}

pub unsafe fn init() {
    // Enable SCE (syscall extensions) in EFER
    let efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | 1);

    // STAR layout (bits):
    //   [63:48] user CS-8  (sysret loads this+8 for CS, this+0 for SS) → 0x18
    //   [47:32] kernel CS  (syscall loads this for CS, this+8 for SS)  → 0x08
    //   [31:0]  unused (set to 0)
    //
    // So: kernel CS = 0x08, user CS-8 = 0x18 (sysret gives 0x20 | 3 for CS, 0x18 | 3 for SS)
    let star: u64 = (0x0018u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);

    // LSTAR = address of our asm entry stub
    wrmsr(MSR_LSTAR, syscall_entry as u64);

    // SFMASK = clear IF (bit 9) on syscall entry so interrupts don't fire
    // while we're mid-entry before we've swapped the stack
    wrmsr(MSR_SFMASK, 1 << 9);

    crate::dbg_log!(
        "SYSCALL",
        "gate installed (LSTAR={:#x})",
        syscall_entry as u64
    );
}

// Declared here so we can take its address above; defined in the global_asm below
extern "C" {
    fn syscall_entry();
}

global_asm!(
    r#"
.global syscall_entry
syscall_entry:
    // On entry (CPU has done this automatically):
    //   RCX = user RIP (return address)
    //   R11 = user RFLAGS
    //   RSP = still the USER stack  <- we must swap this immediately
    //   CS/SS already set to kernel selectors

    // Save user RSP, then switch to kernel stack via TSS_RSP0
    mov r10, rsp
    lea rsp, [rip + TSS_RSP0]
    mov rsp, [rsp]

    // Stack frame (pushed in this order, so offsets below are from the
    // stack pointer AFTER all pushes):
    //
    //   [rsp+ 0] rbp
    //   [rsp+ 8] r15
    //   [rsp+16] r14
    //   [rsp+24] r13
    //   [rsp+32] r12
    //   [rsp+40] r9
    //   [rsp+48] r8
    //   [rsp+56] rsi   ← original arg2 (syscall arg2)
    //   [rsp+64] rdi   ← original arg1 (syscall arg1)
    //   [rsp+72] rdx   ← original arg3 (syscall arg3)
    //   [rsp+80] rbx
    //   [rsp+88] rax   ← syscall number (we overwrite this with retval before pop)
    //   [rsp+96] rcx   ← user RIP
    //   [rsp+104] r11  ← user RFLAGS
    //   [rsp+112] r10  ← user RSP

    push r10        // user rsp
    push r11        // user rflags
    push rcx        // user rip (sysretq return address)
    push rax        // syscall number  ← will be overwritten with retval below
    push rbx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r12
    push r13
    push r14
    push r15
    push rbp

    // Set up args for syscall_dispatch(num, arg1, arg2, arg3)
    // System V: rdi=arg0, rsi=arg1, rdx=arg2, rcx=arg3
    // We saved the originals on the stack; read them back from their slots.
    mov rdi, [rsp + 8*11]   // rax slot = syscall number
    mov rsi, [rsp + 8*7]    // rdi slot = arg1
    mov rdx, [rsp + 8*6]    // rsi slot = arg2  (note: rdx was pushed at rsp+72 = 8*9 from bottom, recount below)
    mov rcx, [rsp + 8*9]    // rdx slot = arg3

    // Stack from bottom (rsp=0 is rbp):
    // 0:rbp 1:r15 2:r14 3:r13 4:r12 5:r9 6:r8 7:rsi 8:rdi 9:rdx 10:rbx 11:rax 12:rcx 13:r11 14:r10
    mov rdi, [rsp + 8*11]   // syscall number (rax slot)
    mov rsi, [rsp + 8*8]    // arg1 (rdi slot)
    mov rdx, [rsp + 8*7]    // arg2 (rsi slot)
    mov rcx, [rsp + 8*9]    // arg3 (rdx slot)

    call syscall_dispatch    // return value lands in rax

    // Write retval directly into the rax slot on the stack so the pop
    // below picks it up correctly — no separate stash register needed.
    mov [rsp + 8*11], rax

    // Restore all saved registers
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rbx
    pop rax         // ← retval from syscall_dispatch (written above)
    pop rcx         // user rip
    pop r11         // user rflags
    pop r10         // user rsp
    mov rsp, r10    // restore user stack

    sysretq
"#
);

/// Rust syscall dispatcher.
/// num = syscall number, arg1/arg2/arg3 = first three arguments.
/// Return value is passed back to userspace in rax.
#[no_mangle]
pub extern "C" fn syscall_dispatch(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    match num {
        0 => {
            // read(fd, buf, count) — stub: no userspace stdin yet
            let _ = (arg1, arg2, arg3);
            u64::MAX
        }
        1 => {
            // write(fd, buf, count) — dump to serial for now
            let buf = arg2 as *const u8;
            let len = arg3 as usize;
            unsafe {
                for i in 0..len {
                    crate::serial::write_byte(*buf.add(i));
                }
            }
            len as u64
        }
        12 => {
            // brk(addr)
            // If addr == 0: return current break.
            // If addr > current break: grow by allocating pages.
            // Returns new break on success, old break on OOM.
            unsafe { syscall_brk(arg1) }
        }
        60 => {
            // exit(code)
            crate::dbg_log!("SYSCALL", "exit({})", arg1);
            loop {
                unsafe {
                    core::arch::asm!("hlt");
                }
            }
        }
        _ => {
            crate::dbg_log!("SYSCALL", "unhandled syscall {}", num);
            u64::MAX // errno-style -1
        }
    }
}

// ---------------------------------------------------------------------------
// brk implementation
// ---------------------------------------------------------------------------

/// Start the program break well above the kernel's identity-mapped region.
/// 0x0000_7fff_0000_0000 is in the user half of the address space.
static mut PROGRAM_BREAK: usize = 0x4000_0000;

unsafe fn syscall_brk(addr: u64) -> u64 {
    if addr == 0 {
        return PROGRAM_BREAK as u64;
    }

    let new_brk = addr as usize;
    if new_brk <= PROGRAM_BREAK {
        // Shrinking or no-op — just update the break.
        PROGRAM_BREAK = new_brk;
        return PROGRAM_BREAK as u64;
    }

    // Growing — allocate physical pages to cover [old_break..new_brk).
    // Round current break up to next page boundary to find first unmapped page.
    let mut page = (PROGRAM_BREAK + 0xFFF) & !0xFFF;
    let end = (new_brk + 0xFFF) & !0xFFF;

    while page < end {
        let frame = crate::pmm::alloc_frame();
        if frame == 0 {
            // OOM — return the old break to signal failure to libc.
            crate::dbg_log!("BRK", "OOM growing break to {:#x}", new_brk);
            return PROGRAM_BREAK as u64;
        }
        // The kernel uses an identity map for the first 4 GB, so
        // physical == virtual. For addresses above 4 GB (like our break
        // base) you will need to wire up a page-table mapping here once
        // you have proper userspace paging. For now this is fine for
        // testing musl-linked binaries running in ring 0 / kernel space.
        page += 0x1000;
    }

    PROGRAM_BREAK = new_brk;
    crate::dbg_log!("BRK", "break → {:#x}", PROGRAM_BREAK);
    PROGRAM_BREAK as u64
}

extern "C" {
    static mut TSS_RSP0: u64;
}
