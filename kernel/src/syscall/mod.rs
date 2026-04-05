use core::arch::global_asm;

mod dispatch;
pub mod fs;
mod helpers;
pub mod io;
pub mod mm;
pub mod net;
mod nr;
pub mod sys;
pub mod process;
mod result;
pub mod time;
pub mod types;
pub mod vfs;

const MSR_EFER: u32 = 0xC000_0080;
const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;

unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr, in("eax") lo, in("edx") hi,
        options(nostack, nomem)
    );
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr, out("eax") lo, out("edx") hi,
        options(nostack, nomem)
    );
    ((hi as u64) << 32) | lo as u64
}

pub unsafe fn init() {
    let efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | 1);

    // STAR: bits[47:32] = kernel CS (0x08), bits[63:48] = user CS - 16 (0x10)
    let star: u64 = (0x0010u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);

    wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);

    // SFMASK: clear IF (bit 9) on syscall entry so we start with interrupts off.
    wrmsr(MSR_SFMASK, 1 << 9);

    crate::dbg_log!(
        "SYSCALL",
        "gate installed (LSTAR={:#x})",
        syscall_entry as *const () as u64
    );
}

extern "C" {
    fn syscall_entry();
    #[allow(dead_code)]
    static mut TSS_RSP0: u64;
}

#[no_mangle]
pub static mut SYSCALL_ARG4: u64 = 0;

global_asm!(
    r#"
.global syscall_entry
syscall_entry:
    // Save user RSP, switch to kernel syscall stack.
    mov  [rip + SYSCALL_ARG4], r10
    mov  r10, rsp
    lea  rsp, [rip + TSS_RSP0]
    mov  rsp, [rsp]

    push r10      // Padding for 16-byte alignment

    // Push full register context
    push r10      // [rsp+112] user RSP
    push r11      // [rsp+104] user RFLAGS
    push rcx      // [rsp+ 96] user RIP
    push rax      // [rsp+ 88] syscall number
    push rbx      // [rsp+ 80]
    push rdx      // [rsp+ 72] arg3
    push rdi      // [rsp+ 64] arg1
    push rsi      // [rsp+ 56] arg2
    push r8       // [rsp+ 48]
    push r9       // [rsp+ 40]
    push r12      // [rsp+ 32]
    push r13      // [rsp+ 24]
    push r14      // [rsp+ 16]
    push r15      // [rsp+  8]
    push rbp      // [rsp+  0]

    lea  rax, [rip + IN_SYSCALL]
    mov  byte ptr [rax], 1          

    // Set up syscall_dispatch(num, arg1, arg2, arg3).
    // Offsets are now relative to the new RSP which has one extra push.
    // Index 11 (rax) is now at 8*11? No, indices 0-14 are same.
    mov  rdi, [rsp + 8*11]   // rax slot  → syscall number
    mov  rsi, [rsp + 8*8]    // rdi slot  → arg1
    mov  rdx, [rsp + 8*7]    // rsi slot  → arg2
    mov  rcx, [rsp + 8*9]    // rdx slot  → arg3
    mov  r8,  [rip + SYSCALL_ARG4]

    mov  r9, rsp
    call syscall_dispatch

    // Store return value back into the rax slot
    mov  [rsp + 8*11], rax

    lea  rax, [rip + IN_SYSCALL]
    mov  byte ptr [rax], 0

    // Restore full context.
    pop  rbp
    pop  r15
    pop  r14
    pop  r13
    pop  r12
    pop  r9
    pop  r8
    pop  rsi
    pop  rdi
    pop  rdx
    pop  rbx
    pop  rax
    pop  rcx
    pop  r11
    pop  r10

    add  rsp, 8   // Skip padding
    
    mov  rsp, r10
    sysretq
"#
);

#[no_mangle]
pub extern "C" fn syscall_dispatch(
    num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    frame: u64,
) -> u64 {
    unsafe {
        let ret = dispatch::route(num, arg1, arg2, arg3, arg4, frame);
        process::finish_syscall(num, frame, ret)
    }
}
