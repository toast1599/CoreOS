use core::arch::global_asm;
extern crate alloc;

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

    // STAR: bits[47:32] = kernel CS (0x08), bits[63:48] = user CS - 16 (0x18)
    let star: u64 = (0x0018u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);

    wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);

    // SFMASK: clear IF (bit 9) on syscall entry so we start with interrupts off.
    // We never re-enable them inside the handler — sysretq restores R11 → RFLAGS.
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

global_asm!(
    r#"
// -----------------------------------------------------------------------
// syscall_entry
//
// Calling state (after `syscall` instruction):
//   RCX  = saved user RIP
//   R11  = saved user RFLAGS  (includes IF=1)
//   RSP  = user stack pointer  (MUST NOT be touched before saving)
//   RAX  = syscall number
//   RDI  = arg1, RSI = arg2, RDX = arg3
//   IF   = 0  (cleared by SFMASK)
//
// Stack layout after all pushes (RSP grows down):
//   [rsp+  0]  rbp
//   [rsp+  8]  r15
//   [rsp+ 16]  r14
//   [rsp+ 24]  r13
//   [rsp+ 32]  r12
//   [rsp+ 40]  r9
//   [rsp+ 48]  r8
//   [rsp+ 56]  rsi      ← arg2
//   [rsp+ 64]  rdi      ← arg1
//   [rsp+ 72]  rdx      ← arg3
//   [rsp+ 80]  rbx
//   [rsp+ 88]  rax      ← syscall number
//   [rsp+ 96]  rcx      ← saved user RIP
//   [rsp+104]  r11      ← saved user RFLAGS
//   [rsp+112]  r10      ← saved user RSP
//
// DO NOT `sti` anywhere in this path. SFMASK cleared IF; sysretq
// restores it from R11.  Any `sti` here risks a PIT preemption that
// calls switch_to() with a live syscall frame on the stack.
// -----------------------------------------------------------------------
.global syscall_entry
syscall_entry:
    // Save user RSP, switch to kernel syscall stack.
    mov  r10, rsp
    lea  rsp, [rip + TSS_RSP0]
    mov  rsp, [rsp]

    // Push full register context (interrupts are OFF — stay that way).
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
    mov  rdi, [rsp + 8*11]   // rax slot  → syscall number
    mov  rsi, [rsp + 8*8]    // rdi slot  → arg1
    mov  rdx, [rsp + 8*7]    // rsi slot  → arg2
    mov  rcx, [rsp + 8*9]    // rdx slot  → arg3

    call syscall_dispatch

    // Store return value back into the rax slot so it's restored below.
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
    pop  rax      // return value (already in rax from the mov above, but restore cleanly)
    pop  rcx      // user RIP → RCX for sysretq
    pop  r11      // user RFLAGS → R11 for sysretq  (includes IF=1)
    pop  r10      // user RSP

    mov  rsp, r10
    sysretq       // restores RIP from RCX, RFLAGS from R11 (re-enables interrupts)
"#
);

// ---------------------------------------------------------------------------
// Filesystem helpers (unchanged)
// ---------------------------------------------------------------------------

unsafe fn fs_find_idx(name: &[char]) -> Option<usize> {
    crate::fs::FILESYSTEM
        .as_ref()?
        .files
        .iter()
        .position(|f| f.name.as_slice() == name)
}

unsafe fn fs_file_size(file_idx: usize) -> usize {
    match crate::fs::FILESYSTEM.as_ref() {
        Some(fs) if file_idx < fs.files.len() => fs.files[file_idx].data.len(),
        _ => 0,
    }
}

unsafe fn fs_read(file_idx: usize, offset: usize, buf: *mut u8, count: usize) -> usize {
    let fs = match crate::fs::FILESYSTEM.as_ref() {
        Some(f) => f,
        None => return 0,
    };
    if file_idx >= fs.files.len() {
        return 0;
    }
    let data = &fs.files[file_idx].data;
    let available = data.len().saturating_sub(offset);
    let to_read = count.min(available);
    if to_read == 0 {
        return 0;
    }
    core::ptr::copy_nonoverlapping(data[offset..].as_ptr(), buf, to_read);
    to_read
}

unsafe fn fs_clone_by_name(name: &[char]) -> Option<alloc::vec::Vec<u8>> {
    let fs = crate::fs::FILESYSTEM.as_ref()?;
    let file = fs.files.iter().find(|f| f.name.as_slice() == name)?;
    Some(file.data.clone())
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn syscall_dispatch(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    match num {
        0 => unsafe { syscall_read(arg1, arg2, arg3) },
        1 => unsafe { syscall_write(arg1, arg2, arg3) },
        3 => unsafe { syscall_open(arg1, arg2) },
        4 => unsafe {
            if crate::process::close_fd(arg1 as usize) {
                0
            } else {
                u64::MAX
            }
        },
        5 => unsafe { syscall_fsize(arg1) },
        6 => unsafe { syscall_ls(arg1, arg2) },
        7 => unsafe { syscall_touch(arg1, arg2) },
        8 => unsafe { syscall_rm(arg1, arg2) },
        9 => unsafe { syscall_write_file(arg1, arg2, arg3) },
        10 => unsafe { syscall_push_file(arg1, arg2, arg3) },
        12 => unsafe { syscall_brk(arg1) },
        57 => unsafe { syscall_exec(arg1, arg2) },
        60 => {
            crate::dbg_log!("SYSCALL", "exit({})", arg1);
            unsafe {
                crate::process::exit(arg1 as i64);
                if let Some(slot) = crate::task::current_task_slot() {
                    crate::task::kill_task(slot);
                }
                // Re-enable interrupts so the scheduler can run and
                // eventually switch away from this dead task.
                core::arch::asm!("sti");
            }
            loop {
                unsafe {
                    core::arch::asm!("hlt");
                }
            }
        }
        61 => unsafe { syscall_waitpid(arg1) },
        20 => crate::pmm::free_bytes() as u64,
        21 => crate::hw::pit::uptime_seconds(),
        22 => crate::hw::pit::ticks(),
        23 => unsafe {
            crate::hw::reboot();
        },
        24 => panic!("user-requested panic via syscall"),
        25 => unsafe { syscall_boottime(arg1, arg2) },
        26 => unsafe {
            crate::vga::clear_terminal_area();
            crate::vga::set_userspace_cursor(20, 120);
            0
        },
        27 => {
            crate::hw::pit::sleep_yield(arg1);
            0
        }
        28 => {
            crate::vga::set_font_scale(arg1 as usize);
            0
        }
        _ => {
            crate::dbg_log!("SYSCALL", "unhandled syscall {}", num);
            u64::MAX
        }
    }
}

// ---------------------------------------------------------------------------
// write — fd 1/2: serial + framebuffer. No sti needed.
// ---------------------------------------------------------------------------

unsafe fn syscall_write(_fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let buf = buf_ptr as *const u8;
    let len = count as usize;
    for i in 0..len {
        let b = *buf.add(i);
        crate::serial::write_byte(b);
        crate::vga::write_byte_to_fb(b);
    }
    len as u64
}

// ---------------------------------------------------------------------------
// read — stdin blocks with sti/cli bracket; never hlt, never yields.
// ---------------------------------------------------------------------------

unsafe fn syscall_read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    crate::serial_fmt!("[READ] fd={} buf={:#x} count={}\n", fd, buf_ptr, count);
    let buf = buf_ptr as *mut u8;
    let count = count as usize;
    if count == 0 {
        return 0;
    }

    if fd == 0 {
        for i in 0..count {
            crate::serial_fmt!("[READ] waiting\n");

            let c = loop {
                // Lower IN_SYSCALL so the scheduler can preempt while we wait.
                // The syscall frame is stable here — just spinning for a keypress.
                crate::scheduler::IN_SYSCALL.store(false, core::sync::atomic::Ordering::Relaxed);

                core::arch::asm!("sti", options(nostack, nomem));
                for _ in 0..2000 {
                    core::hint::spin_loop();
                }
                core::arch::asm!("cli", options(nostack, nomem));

                // Restore before touching any kernel state.
                crate::scheduler::IN_SYSCALL.store(true, core::sync::atomic::Ordering::Relaxed);

                if let Some(c) = crate::hw::kbd_buffer::KEYBUF.pop() {
                    break c;
                }
            };

            buf.add(i).write(c as u8);
            crate::serial_fmt!("[READ] got {:#x}\n", c as u8);
        }
        return count as u64;
    }

    // File descriptor read.
    let fd_usize = fd as usize;
    let of = match crate::process::get_fd_mut(fd_usize) {
        Some(f) => f,
        None => {
            crate::dbg_log!("SYSCALL", "read: bad fd {}", fd_usize);
            return u64::MAX;
        }
    };
    let bytes_read = fs_read(of.file_idx, of.offset, buf, count);
    of.offset += bytes_read;
    bytes_read as u64
}

// ---------------------------------------------------------------------------
// open / close / fsize
// ---------------------------------------------------------------------------

unsafe fn syscall_open(path_ptr: u64, path_len: u64) -> u64 {
    if path_len == 0 || path_len > 64 {
        return u64::MAX;
    }
    let path_bytes = core::slice::from_raw_parts(path_ptr as *const u8, path_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in path_bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..path_len as usize];
    let file_idx = match fs_find_idx(name) {
        Some(i) => i,
        None => return u64::MAX,
    };
    let fd = crate::process::alloc_fd(file_idx);
    if fd < 0 {
        u64::MAX
    } else {
        fd as u64
    }
}

unsafe fn syscall_fsize(fd: u64) -> u64 {
    let of = match crate::process::get_fd(fd as usize) {
        Some(f) => f,
        None => return u64::MAX,
    };
    fs_file_size(of.file_idx) as u64
}

// ---------------------------------------------------------------------------
// exec / waitpid
// ---------------------------------------------------------------------------

unsafe fn syscall_exec(path_ptr: u64, path_len: u64) -> u64 {
    if path_len == 0 || path_len > 64 {
        return 0;
    }
    let path_bytes = core::slice::from_raw_parts(path_ptr as *const u8, path_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in path_bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..path_len as usize];
    let elf_bytes = match fs_clone_by_name(name) {
        Some(v) => v,
        None => {
            crate::dbg_log!("SYSCALL", "exec: file not found");
            return 0;
        }
    };
    let (pid, _slot) = crate::exec::exec_as_task(elf_bytes.as_slice());
    pid as u64
}

unsafe fn syscall_waitpid(pid: u64) -> u64 {
    // Find slot for this pid
    let target_pid = pid as usize;
    let mut slot = None;
    for i in 0..8 {
        if let Some(ref p) = crate::process::PROCESSES[i] {
            if p.pid == target_pid {
                slot = Some(i);
                break;
            }
        }
    }

    let slot = match slot {
        Some(s) => s,
        None => return u64::MAX,
    };

    loop {
        if !crate::process::is_running_in_slot(slot) {
            break;
        }
        crate::scheduler::yield_now();
        core::hint::spin_loop();
    }
    match crate::process::reap_slot(slot) {
        Some(code) => code as u64,
        None => u64::MAX,
    }
}

// ---------------------------------------------------------------------------
// brk
// ---------------------------------------------------------------------------

unsafe fn syscall_brk(addr: u64) -> u64 {
    let current_brk = crate::process::current_brk();
    if addr == 0 {
        return current_brk as u64;
    }
    let new_brk = addr as usize;
    if new_brk <= current_brk {
        crate::process::set_brk(new_brk);
        return new_brk as u64;
    }

    // Align both to next page boundary to see if we need new pages
    let mut page = (current_brk + 0xFFF) & !0xFFF;
    let end = (new_brk + 0xFFF) & !0xFFF;

    while page < end {
        let frame = crate::pmm::alloc_frame();
        if frame == 0 {
            crate::dbg_log!("BRK", "OOM");
            return current_brk as u64;
        }
        // Map the new physical frame to the virtual address 'page'
        crate::paging::map_page(page, frame);
        page += 0x1000;
    }

    crate::process::set_brk(new_brk);
    new_brk as u64
}

// ---------------------------------------------------------------------------
// ls / touch / rm / write_file / push_file
// ---------------------------------------------------------------------------

unsafe fn syscall_ls(buf_ptr: u64, buf_len: u64) -> u64 {
    let fs = match crate::fs::FILESYSTEM.as_ref() {
        Some(f) => f,
        None => return 0,
    };
    let buf = buf_ptr as *mut u8;
    let buf_len = buf_len as usize;
    let mut pos = 0usize;
    for file in fs.files.iter() {
        for &ch in file.name.iter() {
            if pos + 1 >= buf_len {
                break;
            }
            buf.add(pos).write(ch as u8);
            pos += 1;
        }
        if pos < buf_len {
            buf.add(pos).write(0);
            pos += 1;
        }
    }
    pos as u64
}

unsafe fn syscall_touch(name_ptr: u64, name_len: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    let bytes = core::slice::from_raw_parts(name_ptr as *const u8, name_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..name_len as usize];
    match crate::fs::FILESYSTEM.as_mut() {
        Some(fs) => {
            if fs.create(name) {
                0
            } else {
                u64::MAX
            }
        }
        None => u64::MAX,
    }
}

unsafe fn syscall_rm(name_ptr: u64, name_len: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    let bytes = core::slice::from_raw_parts(name_ptr as *const u8, name_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..name_len as usize];
    match crate::fs::FILESYSTEM.as_mut() {
        Some(fs) => {
            if fs.remove(name) {
                0
            } else {
                u64::MAX
            }
        }
        None => u64::MAX,
    }
}

unsafe fn syscall_write_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    let data_ptr = (args_ptr as *const u64).read();
    let data_len = (args_ptr as *const u64).add(1).read() as usize;
    let bytes = core::slice::from_raw_parts(name_ptr as *const u8, name_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..name_len as usize];
    let fs = match crate::fs::FILESYSTEM.as_mut() {
        Some(f) => f,
        None => return u64::MAX,
    };
    let file = match fs.find_mut(name) {
        Some(f) => f,
        None => return u64::MAX,
    };
    file.data.clear();
    let src = core::slice::from_raw_parts(data_ptr as *const u8, data_len);
    file.data.extend_from_slice(src);
    0
}

unsafe fn syscall_push_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    let data_ptr = (args_ptr as *const u64).read();
    let data_len = (args_ptr as *const u64).add(1).read() as usize;
    let bytes = core::slice::from_raw_parts(name_ptr as *const u8, name_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..name_len as usize];
    let fs = match crate::fs::FILESYSTEM.as_mut() {
        Some(f) => f,
        None => return u64::MAX,
    };
    let file = match fs.find_mut(name) {
        Some(f) => f,
        None => return u64::MAX,
    };
    let src = core::slice::from_raw_parts(data_ptr as *const u8, data_len);
    file.data.extend_from_slice(src);
    0
}

unsafe fn syscall_boottime(buf_ptr: u64, buf_len: u64) -> u64 {
    let report = crate::bench::report();
    let bytes = report.as_bytes();
    let count = (buf_len as usize).min(bytes.len());
    let buf = buf_ptr as *mut u8;
    core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, count);
    count as u64
}
