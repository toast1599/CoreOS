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
    //   RSP = still the USER stack  ← we must swap this immediately
    //   CS/SS already set to kernel selectors

    // Swap to kernel stack via TSS.rsp0
    // We use swapgs-free approach: just load TSS.rsp0 directly.
    // Save user RSP in r10 (caller-saved, safe to clobber)
    mov r10, rsp
    // Load kernel stack from TSS.rsp0
    // TSS is a static symbol exported from gdt.rs
	lea rsp, [rip + TSS_RSP0]
	mov rsp, [rsp]

    // Now on kernel stack — save everything
    push r10        // user rsp
    push r11        // user rflags
    push rcx        // user rip (sysretq return address)
    push rax        // syscall number
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

    // Call Rust dispatcher
    // Args (System V): rdi=rax(num), rsi=rdi(arg1), rdx=rsi(arg2), rcx=rdx(arg3)
    // But we already pushed rdi/rsi/rdx — read them from where they were before push
    // Simpler: pass the stack pointer, let Rust unpack it
    mov rdi, rax    // syscall number
    mov rsi, [rsp + 8*6]   // original rdi (arg1) — index from bottom of our pushes
    mov rdx, [rsp + 8*7]   // original rsi (arg2)
    mov rcx, [rsp + 8*5]   // original rdx (arg3)
    call syscall_dispatch

    // Restore
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop r9
    pop r8
    pop rsi         // original rsi
    pop rdi         // original rdi
    pop rdx         // original rdx
    pop rbx
    pop rax         // syscall number — rax now holds return value from dispatch? no:
                    // syscall_dispatch returns in rax automatically (C ABI)
                    // so we need to save retval before pops. see note below.
    pop rcx         // user rip
    pop r11         // user rflags
    pop r10         // user rsp
    mov rsp, r10    // restore user stack

    sysretq
"#
);

/// Rust syscall dispatcher — Phase 2 will fill this out properly.
/// rax = syscall number, rdi = arg1, rsi = arg2, rdx = arg3
#[no_mangle]
pub extern "C" fn syscall_dispatch(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    match num {
        1 => {
            // write — stub: just dump to serial for now
            let buf = arg2 as *const u8;
            let len = arg3 as usize;
            unsafe {
                for i in 0..len {
                    crate::serial::write_byte(*buf.add(i));
                }
            }
            len as u64
        }
        60 => {
            // exit
            crate::dbg_log!("SYSCALL", "exit({})", arg1);
            loop {
                unsafe {
                    core::arch::asm!("hlt");
                }
            }
        }
        _ => {
            crate::dbg_log!("SYSCALL", "unhandled syscall {}", num);
            u64::MAX // -1, errno style
        }
    }
}

extern "C" {
    static mut TSS_RSP0: u64;
}

