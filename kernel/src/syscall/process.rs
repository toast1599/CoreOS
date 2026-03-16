use crate::proc;
use crate::proc::scheduler;
use crate::proc::task;
use crate::mem::pmm;
use crate::arch::paging;
use crate::syscall::fs;

const PROT_EXEC: u32 = 0x1;
const PROT_WRITE: u32 = 0x2;
const MAP_PRIVATE: u32 = 0x02;
const MAP_FIXED: u32 = 0x10;
const MAP_ANONYMOUS: u32 = 0x20;
const MAP_FAILED: u64 = u64::MAX;
const CLOCK_REALTIME: u64 = 0;
const CLOCK_MONOTONIC: u64 = 1;
const NANOS_PER_SEC: u64 = 1_000_000_000;
const PIT_HZ: u64 = 100;
const O_CLOEXEC: u64 = 0o2000000;
const O_APPEND: u64 = 0o2000;
const O_NONBLOCK: u64 = 0o4000;
const O_RDONLY: u64 = 0;
const F_DUPFD: u64 = 0;
const F_GETFD: u64 = 1;
const F_SETFD: u64 = 2;
const F_GETFL: u64 = 3;
const F_SETFL: u64 = 4;
const F_DUPFD_CLOEXEC: u64 = 1030;
const FCNTL_STATUS_MASK: u32 = (O_APPEND | O_NONBLOCK) as u32;

#[repr(C)]
struct MmapArgs {
    addr: u64,
    len: u64,
    prot: u32,
    flags: u32,
    fd: i32,
    off: i64,
}

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

pub unsafe fn syscall_exec(path_ptr: u64, path_len: u64) -> u64 {
    if path_len == 0 || path_len > 64 {
        return 0;
    }
    let mut raw = [0u8; 64];
    if crate::usercopy::copy_from_user(&mut raw[..path_len as usize], path_ptr).is_err() {
        return 0;
    }
    let mut name_buf = ['\0'; 64];
    for i in 0..path_len as usize {
        name_buf[i] = raw[i] as char;
    }
    let name = &name_buf[..path_len as usize];
    let elf_bytes = match fs::fs_clone_by_name(name) {
        Some(v) => v,
        None => {
            crate::dbg_log!("SYSCALL", "exec: file not found");
            return 0;
        }
    };
    let (pid, _slot) = crate::proc::exec::exec_as_task(elf_bytes.as_slice());
    pid as u64
}

pub unsafe fn syscall_getpid() -> u64 {
    proc::current_pid() as u64
}

pub unsafe fn syscall_fork(frame_ptr: u64) -> u64 {
    if frame_ptr == 0 {
        return u64::MAX;
    }

    let child_pml4 = paging::clone_user_address_space(task::current_pml4());
    let child_slot = match task::spawn_forked_task(frame_ptr as *const u8, child_pml4) {
        Some(s) => s,
        None => return u64::MAX,
    };
    let child_pid = proc::fork_current(child_slot, child_pml4);
    child_pid as u64
}

pub unsafe fn syscall_dup(oldfd: u64) -> u64 {
    let fd = proc::dup_fd(oldfd as usize, 0, None, false);
    if fd < 0 {
        u64::MAX
    } else {
        fd as u64
    }
}

pub unsafe fn syscall_dup3(oldfd: u64, newfd: u64, flags: u64) -> u64 {
    if flags & !O_CLOEXEC != 0 {
        return u64::MAX;
    }
    let cloexec = (flags & O_CLOEXEC) != 0;
    let fd = if oldfd == newfd {
        match proc::get_fd_target(oldfd as usize) {
            Some(_) => {
                if cloexec {
                    if !proc::set_fd_flags(oldfd as usize, proc::FD_CLOEXEC) {
                        -1
                    } else {
                        oldfd as i64
                    }
                } else {
                    oldfd as i64
                }
            }
            None => -1,
        }
    } else {
        proc::dup_fd(oldfd as usize, 0, Some(newfd as usize), cloexec)
    };
    if fd < 0 {
        u64::MAX
    } else {
        fd as u64
    }
}

pub unsafe fn syscall_fcntl(fd: u64, cmd: u64, arg: u64) -> u64 {
    match cmd {
        F_DUPFD => {
            let new_fd = proc::dup_fd(fd as usize, arg as usize, None, false);
            if new_fd < 0 { u64::MAX } else { new_fd as u64 }
        }
        F_DUPFD_CLOEXEC => {
            let new_fd = proc::dup_fd(fd as usize, arg as usize, None, true);
            if new_fd < 0 { u64::MAX } else { new_fd as u64 }
        }
        F_GETFD => match proc::get_fd_flags(fd as usize) {
            Some(flags) => flags as u64,
            None => u64::MAX,
        },
        F_SETFD => {
            let flags = (arg as u32) & proc::FD_CLOEXEC;
            if proc::set_fd_flags(fd as usize, flags) {
                0
            } else {
                u64::MAX
            }
        }
        F_GETFL => match proc::get_status_flags(fd as usize) {
            Some(flags) => (flags as u64) | O_RDONLY,
            None => u64::MAX,
        },
        F_SETFL => {
            let flags = (arg as u32) & FCNTL_STATUS_MASK;
            if proc::set_status_flags(fd as usize, flags) {
                0
            } else {
                u64::MAX
            }
        }
        _ => u64::MAX,
    }
}

pub unsafe fn syscall_pipe(pipefd_ptr: u64) -> u64 {
    if !crate::usercopy::user_range_ok(pipefd_ptr, core::mem::size_of::<[i32; 2]>()) {
        return u64::MAX;
    }
    let (read_fd, write_fd) = match proc::alloc_pipe() {
        Some(pair) => pair,
        None => return u64::MAX,
    };
    let pipefds = [read_fd as i32, write_fd as i32];
    let bytes = core::slice::from_raw_parts(
        (&pipefds as *const [i32; 2]).cast::<u8>(),
        core::mem::size_of::<[i32; 2]>(),
    );
    if crate::usercopy::copy_to_user(pipefd_ptr, bytes).is_err() {
        let _ = proc::close_fd(read_fd);
        let _ = proc::close_fd(write_fd);
        return u64::MAX;
    }
    0
}

fn map_flags_from_prot(prot: u32) -> paging::MapFlags {
    paging::MapFlags {
        writable: (prot & PROT_WRITE) != 0,
        user: true,
        executable: (prot & PROT_EXEC) != 0,
    }
}

pub unsafe fn syscall_mmap(args_ptr: u64) -> u64 {
    if !crate::usercopy::user_range_ok(args_ptr, core::mem::size_of::<MmapArgs>()) {
        return MAP_FAILED;
    }

    let mut raw = [0u8; core::mem::size_of::<MmapArgs>()];
    if crate::usercopy::copy_from_user(&mut raw, args_ptr).is_err() {
        return MAP_FAILED;
    }
    let args = core::ptr::read_unaligned(raw.as_ptr().cast::<MmapArgs>());

    if args.len == 0 {
        return MAP_FAILED;
    }
    if (args.flags & MAP_PRIVATE) == 0 || (args.flags & MAP_ANONYMOUS) == 0 {
        return MAP_FAILED;
    }
    if args.fd != -1 || args.off != 0 {
        return MAP_FAILED;
    }

    let len = ((args.len as usize) + 0xFFF) & !0xFFF;
    let start = if args.addr == 0 {
        match proc::reserve_mmap_base(len) {
            Some(s) => s,
            None => return MAP_FAILED,
        }
    } else {
        let addr = (args.addr as usize) & !0xFFF;
        if (args.flags & MAP_FIXED) == 0 || proc::region_conflicts(addr, len) {
            return MAP_FAILED;
        }
        addr
    };

    if proc::region_conflicts(start, len) {
        return MAP_FAILED;
    }
    if !proc::alloc_vma(start, len, args.prot, args.flags) {
        return MAP_FAILED;
    }

    let pml4 = task::current_pml4();
    let flags = map_flags_from_prot(args.prot);
    for off in (0..len).step_by(0x1000) {
        let frame = pmm::alloc_frame();
        if frame == 0 {
            return MAP_FAILED;
        }
        paging::map_page_in(pml4, start + off, frame, flags);
        core::ptr::write_bytes(paging::p2v(frame) as *mut u8, 0, 0x1000);
    }
    start as u64
}

pub unsafe fn syscall_munmap(addr: u64, len: u64) -> u64 {
    if addr == 0 || len == 0 {
        return u64::MAX;
    }
    let start = (addr as usize) & !0xFFF;
    let len = ((len as usize) + 0xFFF) & !0xFFF;

    let pml4 = task::current_pml4();
    let vma = match proc::find_vma_exact_mut(start, len) {
        Some(v) => v,
        None => return u64::MAX,
    };

    for off in (0..len).step_by(0x1000) {
        if let Some(frame) = paging::unmap_page_in(pml4, start + off) {
            pmm::free_frame(frame);
        }
    }
    *vma = proc::VmRegion::empty();
    0
}

pub unsafe fn syscall_mprotect(addr: u64, len: u64, prot: u64) -> u64 {
    if addr == 0 || len == 0 {
        return u64::MAX;
    }
    let start = (addr as usize) & !0xFFF;
    let len = ((len as usize) + 0xFFF) & !0xFFF;

    let pml4 = task::current_pml4();
    let vma = match proc::find_vma_exact_mut(start, len) {
        Some(v) => v,
        None => return u64::MAX,
    };
    let flags = map_flags_from_prot(prot as u32);
    for off in (0..len).step_by(0x1000) {
        if !paging::protect_page_in(pml4, start + off, flags) {
            return u64::MAX;
        }
    }
    vma.prot = prot as u32;
    0
}

pub unsafe fn syscall_clock_gettime(clockid: u64, tp_ptr: u64) -> u64 {
    if !matches!(clockid, CLOCK_REALTIME | CLOCK_MONOTONIC) {
        return u64::MAX;
    }
    if !crate::usercopy::user_range_ok(tp_ptr, core::mem::size_of::<Timespec>()) {
        return u64::MAX;
    }

    let ticks = crate::hw::pit::ticks();
    let sec = (ticks / PIT_HZ) as i64;
    let nsec = ((ticks % PIT_HZ) * (NANOS_PER_SEC / PIT_HZ)) as i64;
    let ts = Timespec {
        tv_sec: sec,
        tv_nsec: nsec,
    };
    let bytes = core::slice::from_raw_parts(
        (&ts as *const Timespec).cast::<u8>(),
        core::mem::size_of::<Timespec>(),
    );
    if crate::usercopy::copy_to_user(tp_ptr, bytes).is_err() {
        return u64::MAX;
    }
    0
}

pub unsafe fn syscall_nanosleep(req_ptr: u64, rem_ptr: u64) -> u64 {
    if !crate::usercopy::user_range_ok(req_ptr, core::mem::size_of::<Timespec>()) {
        return u64::MAX;
    }

    let mut raw = [0u8; core::mem::size_of::<Timespec>()];
    if crate::usercopy::copy_from_user(&mut raw, req_ptr).is_err() {
        return u64::MAX;
    }
    let req = core::ptr::read_unaligned(raw.as_ptr().cast::<Timespec>());
    if req.tv_sec < 0 || req.tv_nsec < 0 || req.tv_nsec >= NANOS_PER_SEC as i64 {
        return u64::MAX;
    }

    let total_ns = (req.tv_sec as u64)
        .saturating_mul(NANOS_PER_SEC)
        .saturating_add(req.tv_nsec as u64);
    let ticks = total_ns.div_ceil(NANOS_PER_SEC / PIT_HZ);
    crate::hw::pit::sleep_yield(ticks);

    if rem_ptr != 0 {
        if !crate::usercopy::user_range_ok(rem_ptr, core::mem::size_of::<Timespec>()) {
            return u64::MAX;
        }
        let rem = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let rem_bytes = core::slice::from_raw_parts(
            (&rem as *const Timespec).cast::<u8>(),
            core::mem::size_of::<Timespec>(),
        );
        if crate::usercopy::copy_to_user(rem_ptr, rem_bytes).is_err() {
            return u64::MAX;
        }
    }

    0
}

pub unsafe fn syscall_waitpid(pid: u64) -> u64 {
    let target_pid = pid as usize;
    let self_slot = match task::current_task_slot() {
        Some(s) => s,
        None => return u64::MAX,
    };
    let mut slot = None;
    for i in 0..8 {
        if let Some(ref p) = proc::PROCESSES[i] {
            if p.pid == target_pid {
                if i == self_slot {
                    return u64::MAX;
                }
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
        if !proc::is_running_in_slot(slot) {
            break;
        }
        scheduler::yield_now();
        core::hint::spin_loop();
    }
    match proc::reap_slot(slot) {
        Some(code) => code as u64,
        None => u64::MAX,
    }
}

pub unsafe fn syscall_brk(addr: u64) -> u64 {
    let current_brk = proc::current_brk();
    if addr == 0 {
        return current_brk as u64;
    }
    let new_brk = addr as usize;
    if new_brk <= current_brk {
        proc::set_brk(new_brk);
        return new_brk as u64;
    }

    let mut page = (current_brk + 0xFFF) & !0xFFF;
    let end = (new_brk + 0xFFF) & !0xFFF;

    while page < end {
        let frame = pmm::alloc_frame();
        if frame == 0 {
            crate::dbg_log!("BRK", "OOM");
            return current_brk as u64;
        }
        paging::map_page_in(
            task::current_pml4(),
            page,
            frame,
            paging::MapFlags {
                writable: true,
                user: true,
                executable: false,
            },
        );
        core::ptr::write_bytes(paging::p2v(frame) as *mut u8, 0, 0x1000);
        page += 0x1000;
    }

    proc::set_brk(new_brk);
    new_brk as u64
}

pub unsafe fn syscall_boottime(buf_ptr: u64, buf_len: u64) -> u64 {
    let report = crate::bench::report();
    let bytes = report.as_bytes();
    let count = (buf_len as usize).min(bytes.len());
    if crate::usercopy::copy_to_user(buf_ptr, &bytes[..count]).is_err() {
        return u64::MAX;
    }
    count as u64
}

pub unsafe fn syscall_exit(code: u64) -> ! {
    crate::dbg_log!("SYSCALL", "exit({})", code);
    proc::exit(code as i64);
    if let Some(slot) = task::current_task_slot() {
        task::kill_task(slot);
    }
    scheduler::IN_SYSCALL.store(false, core::sync::atomic::Ordering::Relaxed);
    core::arch::asm!("sti");
    loop {
        core::arch::asm!("hlt");
    }
}
