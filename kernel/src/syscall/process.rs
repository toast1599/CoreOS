use crate::arch::paging;
use crate::proc;
use crate::proc::scheduler;
use crate::proc::task;
use crate::syscall::nr;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use crate::syscall::types::{SigAction, SigSet, SyscallFrame, TimeSpec};

const O_CLOEXEC: u64 = 0o2000000;
const O_APPEND: u64 = crate::proc::O_APPEND as u64;
const O_NONBLOCK: u64 = crate::proc::O_NONBLOCK as u64;
const O_RDONLY: u64 = crate::proc::O_RDONLY as u64;
const F_DUPFD: u64 = 0;
const F_GETFD: u64 = 1;
const F_SETFD: u64 = 2;
const F_GETFL: u64 = 3;
const F_SETFL: u64 = 4;
const F_DUPFD_CLOEXEC: u64 = 1030;
const FCNTL_STATUS_MASK: u32 = (O_APPEND | O_NONBLOCK) as u32;
const SIGSET_SIZE: u64 = core::mem::size_of::<SigSet>() as u64;
const SA_RESTORER: u64 = 0x0400_0000;
const SA_ONSTACK: u64 = 0x0800_0000;
const UMASK_MASK: u32 = 0o777;
const SIG_BLOCK: u64 = 0;
const SIG_UNBLOCK: u64 = 1;
const SIG_SETMASK: u64 = 2;
const SIGKILL: usize = 9;
const SIGCHLD: usize = 17;
const SS_DISABLE: i32 = 2;
const CLONE_VM: u64 = 0x0000_0100;
const CLONE_FS: u64 = 0x0000_0200;
const CLONE_FILES: u64 = 0x0000_0400;
const CLONE_SIGHAND: u64 = 0x0000_0800;
const CLONE_PIDFD: u64 = 0x0000_1000;
const CLONE_PTRACE: u64 = 0x0000_2000;
const CLONE_VFORK: u64 = 0x0000_4000;
const CLONE_PARENT: u64 = 0x0000_8000;
const CLONE_THREAD: u64 = 0x0001_0000;
const CLONE_NEWNS: u64 = 0x0002_0000;
const CLONE_SYSVSEM: u64 = 0x0004_0000;
const CLONE_SETTLS: u64 = 0x0008_0000;
const CLONE_PARENT_SETTID: u64 = 0x0010_0000;
const CLONE_CHILD_CLEARTID: u64 = 0x0020_0000;
const CLONE_DETACHED: u64 = 0x0040_0000;
const CLONE_UNTRACED: u64 = 0x0080_0000;
const CLONE_CHILD_SETTID: u64 = 0x0100_0000;
const CLONE_NEWCGROUP: u64 = 0x0200_0000;
const CLONE_NEWUTS: u64 = 0x0400_0000;
const CLONE_NEWIPC: u64 = 0x0800_0000;
const CLONE_NEWUSER: u64 = 0x1000_0000;
const CLONE_NEWPID: u64 = 0x2000_0000;
const CLONE_NEWNET: u64 = 0x4000_0000;
const CLONE_IO: u64 = 0x8000_0000;
const CLONE_SIGNAL_MASK: u64 = 0xff;
const CLONE_THREAD_FLAGS: u64 =
    CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD;
const CLONE_SUPPORTED_MASK: u64 =
    CLONE_CHILD_CLEARTID | CLONE_CHILD_SETTID | CLONE_SETTLS | CLONE_PARENT_SETTID;
const FUTEX_WAIT: u64 = 0;
const FUTEX_WAKE: u64 = 1;
const FUTEX_PRIVATE_FLAG: u64 = 128;
const FUTEX_CMD_MASK: u64 = !(FUTEX_PRIVATE_FLAG);
const FUTEX_OWNER_DIED: u32 = 0x4000_0000;
const FUTEX_TID_MASK: u32 = 0x3fff_ffff;
const MAX_FUTEX_WAITERS: usize = 16;
const ROBUST_LIST_LIMIT: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
struct RobustList {
    next: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RobustListHead {
    list: RobustList,
    futex_offset: i64,
    list_op_pending: u64,
}

#[derive(Clone, Copy)]
struct FutexWaiter {
    in_use: bool,
    addr: u64,
}

impl FutexWaiter {
    const fn empty() -> Self {
        Self {
            in_use: false,
            addr: 0,
        }
    }
}

static mut FUTEX_WAITERS: [FutexWaiter; MAX_FUTEX_WAITERS] =
    [FutexWaiter::empty(); MAX_FUTEX_WAITERS];

fn sig_bit(sig: usize) -> (usize, u64) {
    let idx = (sig - 1) / 64;
    let bit = 1u64 << ((sig - 1) % 64);
    (idx, bit)
}

fn sigset_contains(set: &SigSet, sig: usize) -> bool {
    let (idx, bit) = sig_bit(sig);
    (set.bits[idx] & bit) != 0
}

fn sigset_add(set: &mut SigSet, sig: usize) {
    let (idx, bit) = sig_bit(sig);
    set.bits[idx] |= bit;
}

fn sigset_remove(set: &mut SigSet, sig: usize) {
    let (idx, bit) = sig_bit(sig);
    set.bits[idx] &= !bit;
}

fn sanitize_sigmask(set: &mut SigSet) {
    sigset_remove(set, SIGKILL);
}

unsafe fn queue_signal(slot: usize, sig: usize) -> bool {
    let Some(process) = proc::PROCESSES[slot].as_mut() else {
        return false;
    };
    sigset_add(&mut process.sig_pending, sig);
    true
}

unsafe fn queue_thread_signal(slot: usize, sig: usize) -> bool {
    let Some(thread) = proc::THREADS[slot].as_mut() else {
        return false;
    };
    sigset_add(&mut thread.sig_pending, sig);
    true
}

unsafe fn notify_parent_exit(parent_pid: usize) {
    if parent_pid == 0 {
        return;
    }
    if let Some(parent_slot) = proc::find_slot_by_pid(parent_pid) {
        let _ = queue_signal(parent_slot, SIGCHLD);
    }
}

fn signal_default_ignored(sig: usize) -> bool {
    matches!(sig, SIGCHLD)
}

fn signal_default_terminates(sig: usize) -> bool {
    !signal_default_ignored(sig)
}

unsafe fn terminate_process_group(group_slot: usize, sig: usize) {
    let parent_pid = match proc::PROCESSES[group_slot].as_ref() {
        Some(process) => process.parent_pid,
        None => 0,
    };
    if let Some(process) = proc::PROCESSES[group_slot].as_mut() {
        process.state = proc::ProcessState::Zombie;
        process.exit_code = 128 + sig as i64;
        process.thread_count = 0;
    }
    for (slot, maybe_thread) in proc::THREADS.iter_mut().enumerate() {
        let Some(thread) = maybe_thread.as_mut() else {
            continue;
        };
        if thread.group_slot != group_slot {
            continue;
        }
        thread.state = proc::ThreadState::Zombie;
        task::kill_task(slot);
    }
    notify_parent_exit(parent_pid);
}

unsafe fn clone_tls_from_frame(frame_ptr: u64) -> SysResult<u64> {
    Ok(current_frame(frame_ptr)?.r8)
}

unsafe fn current_frame<'a>(frame_ptr: u64) -> SysResult<&'a mut SyscallFrame> {
    result::ensure(frame_ptr != 0, SysError::Invalid)?;
    Ok(&mut *(frame_ptr as *mut SyscallFrame))
}

unsafe fn deliver_pending_signal(frame_ptr: u64) -> SysResult {
    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    let thread = result::option(proc::current_thread_mut(), SysError::Invalid)?;
    if thread.in_signal_handler {
        return result::ok(0u64);
    }

    let mut chosen = None;
    for sig in 1..65usize {
        if sigset_contains(&thread.sig_pending, sig) && !sigset_contains(&thread.sig_mask, sig) {
            chosen = Some((sig, true));
            break;
        }
        if sigset_contains(&process.sig_pending, sig) && !sigset_contains(&thread.sig_mask, sig) {
            chosen = Some((sig, false));
            break;
        }
    }
    let Some((sig, thread_directed)) = chosen else {
        return result::ok(0u64);
    };
    if thread_directed {
        sigset_remove(&mut thread.sig_pending, sig);
    } else {
        sigset_remove(&mut process.sig_pending, sig);
    }
    let action = process.sig_handlers[sig];
    if action.handler == 0 {
        if signal_default_ignored(sig) {
            return result::ok(0u64);
        }
        if signal_default_terminates(sig) {
            terminate_process_group(thread.group_slot, sig);
        }
        return result::ok(0u64);
    }

    result::ensure(action.restorer != 0, SysError::Invalid)?;
    let frame = current_frame(frame_ptr)?;
    let mut user_rsp = frame.r10;
    if (action.flags & SA_ONSTACK) != 0 && (thread.sig_altstack.ss_flags & SS_DISABLE) == 0 {
        user_rsp = thread
            .sig_altstack
            .ss_sp
            .checked_add(thread.sig_altstack.ss_size as u64)
            .ok_or(SysError::Range)?;
        thread.on_altstack = true;
    } else {
        thread.on_altstack = false;
    }

    let signal_frame_ptr = user_rsp
        .checked_sub(8 + core::mem::size_of::<SyscallFrame>() as u64)
        .ok_or(SysError::Fault)?;
    let restorer_ptr = signal_frame_ptr;
    let saved_frame_ptr = signal_frame_ptr + 8;
    result::ensure(
        crate::usercopy::user_range_ok(signal_frame_ptr, 8 + core::mem::size_of::<SyscallFrame>()),
        SysError::Fault,
    )?;
    result::ensure(
        helpers::copy_struct_to_user(restorer_ptr, &action.restorer),
        SysError::Fault,
    )?;
    result::ensure(helpers::copy_struct_to_user(saved_frame_ptr, &*frame), SysError::Fault)?;

    let mut combined_mask = thread.sig_mask;
    sigset_add(&mut combined_mask, sig);
    for (dst, src) in combined_mask.bits.iter_mut().zip(action.mask.bits.iter()) {
        *dst |= *src;
    }
    sanitize_sigmask(&mut combined_mask);
    thread.saved_sig_mask = thread.sig_mask;
    thread.sig_mask = combined_mask;
    thread.in_signal_handler = true;

    frame.rdi = sig as u64;
    frame.rcx = action.handler;
    frame.r10 = restorer_ptr;
    result::ok(0u64)
}

pub unsafe fn finish_syscall(num: u64, frame_ptr: u64, ret: u64) -> u64 {
    if let Ok(frame) = current_frame(frame_ptr) {
        frame.rax = ret;
    }
    if num == nr::RT_SIGRETURN {
        return current_frame(frame_ptr).map(|frame| frame.rax).unwrap_or(ret);
    }
    let _ = deliver_pending_signal(frame_ptr);
    current_frame(frame_ptr).map(|frame| frame.rax).unwrap_or(ret)
}

unsafe fn futex_wait(uaddr: u64, expected: u32, timeout_ptr: u64) -> SysResult {
    let current: u32 = result::option(helpers::copy_struct_from_user(uaddr), SysError::Fault)?;
    if current != expected {
        return result::err(SysError::Again);
    }

    let timeout_ticks = if timeout_ptr == 0 {
        None
    } else {
        let timeout: TimeSpec =
            result::option(helpers::copy_struct_from_user(timeout_ptr), SysError::Fault)?;
        result::ensure(timeout.tv_sec >= 0 && timeout.tv_nsec >= 0, SysError::Invalid)?;
        let total_ns = (timeout.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(timeout.tv_nsec as u64);
        Some(total_ns.div_ceil(10_000_000))
    };

    let slot = result::option(
        FUTEX_WAITERS.iter().position(|waiter| !waiter.in_use),
        SysError::NoMem,
    )?;
    FUTEX_WAITERS[slot] = FutexWaiter {
        in_use: true,
        addr: uaddr,
    };

    let start = crate::hw::pit::ticks();
    loop {
        if !FUTEX_WAITERS[slot].in_use {
            return result::ok(0u64);
        }
        let current: u32 =
            result::option(helpers::copy_struct_from_user(uaddr), SysError::Fault)?;
        if current != expected {
            FUTEX_WAITERS[slot] = FutexWaiter::empty();
            return result::err(SysError::Again);
        }
        if let Some(limit) = timeout_ticks {
            if crate::hw::pit::ticks().saturating_sub(start) >= limit {
                FUTEX_WAITERS[slot] = FutexWaiter::empty();
                return result::err(SysError::TimedOut);
            }
        }
        scheduler::yield_now();
    }
}

unsafe fn futex_wake(uaddr: u64, count: usize) -> SysResult {
    let mut woke = 0usize;
    for waiter in FUTEX_WAITERS.iter_mut() {
        if !waiter.in_use || waiter.addr != uaddr {
            continue;
        }
        waiter.in_use = false;
        woke += 1;
        if woke == count {
            break;
        }
    }
    result::ok(woke as u64)
}

unsafe fn wake_robust_futex(uaddr: u64, tid: u32) {
    if !crate::usercopy::user_range_ok(uaddr, core::mem::size_of::<u32>()) {
        return;
    }
    let Some(word): Option<u32> = helpers::copy_struct_from_user(uaddr) else {
        return;
    };
    if (word & FUTEX_TID_MASK) != tid {
        return;
    }
    let updated = (word & !FUTEX_TID_MASK) | FUTEX_OWNER_DIED;
    let _ = helpers::copy_struct_to_user(uaddr, &updated);
    let _ = futex_wake(uaddr, 1);
}

unsafe fn release_robust_list() {
    let Some(thread) = proc::current_thread() else {
        return;
    };
    if thread.robust_list_head == 0 || thread.robust_list_len != core::mem::size_of::<RobustListHead>() {
        return;
    }
    let tid = thread.tid as u32;
    let head_ptr = thread.robust_list_head;
    let Some(head): Option<RobustListHead> = helpers::copy_struct_from_user(head_ptr) else {
        return;
    };

    let mut node_ptr = head.list.next;
    let mut seen = 0usize;
    while node_ptr != 0 && node_ptr != head_ptr && seen < ROBUST_LIST_LIMIT {
        let futex_addr = if head.futex_offset >= 0 {
            node_ptr.saturating_add(head.futex_offset as u64)
        } else {
            node_ptr.saturating_sub((-head.futex_offset) as u64)
        };
        wake_robust_futex(futex_addr, tid);
        let Some(node): Option<RobustList> = helpers::copy_struct_from_user(node_ptr) else {
            break;
        };
        node_ptr = node.next;
        seen += 1;
    }

    if head.list_op_pending != 0 {
        let futex_addr = if head.futex_offset >= 0 {
            head.list_op_pending.saturating_add(head.futex_offset as u64)
        } else {
            head.list_op_pending
                .saturating_sub((-head.futex_offset) as u64)
        };
        wake_robust_futex(futex_addr, tid);
    }
}

pub unsafe fn syscall_exec(path_ptr: u64, argv_ptr: u64, envp_ptr: u64) -> u64 {
    result::ret(syscall_exec_impl(path_ptr, argv_ptr, envp_ptr))
}

unsafe fn syscall_exec_impl(path_ptr: u64, _argv_ptr: u64, _envp_ptr: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_cstr_from_user(path_ptr), SysError::Fault)?;
    let elf_bytes = match crate::vfs::clone_bytes(&name_buf[..name_len]) {
        Some(v) => v,
        None => {
            crate::dbg_log!("SYSCALL", "exec: file not found");
            return result::err(SysError::NoEntry);
        }
    };
    let (pid, _slot) = crate::proc::exec::exec_as_task(elf_bytes.as_slice(), &name_buf[..name_len]);
    result::ensure(pid != 0, SysError::Invalid)?;
    result::ok(pid as u64)
}

pub unsafe fn syscall_close(fd: u64) -> u64 {
    result::ret(syscall_close_impl(fd))
}

unsafe fn syscall_close_impl(fd: u64) -> SysResult {
    result::ensure(proc::close_descriptor(fd as usize), SysError::BadFd)?;
    result::ok(0u64)
}

pub unsafe fn syscall_getpid() -> u64 {
    proc::current_pid() as u64
}

pub unsafe fn syscall_getppid() -> u64 {
    proc::current_ppid() as u64
}

pub unsafe fn syscall_gettid() -> u64 {
    proc::current_tid() as u64
}

pub unsafe fn syscall_getuid() -> u64 {
    proc::current_process().map(|p| p.uid as u64).unwrap_or(0)
}

pub unsafe fn syscall_geteuid() -> u64 {
    proc::current_process().map(|p| p.euid as u64).unwrap_or(0)
}

pub unsafe fn syscall_getgid() -> u64 {
    proc::current_process().map(|p| p.gid as u64).unwrap_or(0)
}

pub unsafe fn syscall_getegid() -> u64 {
    proc::current_process().map(|p| p.egid as u64).unwrap_or(0)
}

pub unsafe fn syscall_setuid(uid: u64) -> u64 {
    result::ret(syscall_setuid_impl(uid))
}

unsafe fn syscall_setuid_impl(uid: u64) -> SysResult {
    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    process.uid = uid as u32;
    process.euid = uid as u32;
    result::ok(0u64)
}

pub unsafe fn syscall_setgid(gid: u64) -> u64 {
    result::ret(syscall_setgid_impl(gid))
}

unsafe fn syscall_setgid_impl(gid: u64) -> SysResult {
    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    process.gid = gid as u32;
    process.egid = gid as u32;
    result::ok(0u64)
}

pub unsafe fn syscall_set_tid_address(tidptr: u64) -> u64 {
    result::ret(syscall_set_tid_address_impl(tidptr))
}

unsafe fn syscall_set_tid_address_impl(tidptr: u64) -> SysResult {
    let thread = result::option(proc::current_thread_mut(), SysError::Invalid)?;
    thread.clear_child_tid = tidptr;
    result::ok(proc::current_tid() as u64)
}

pub unsafe fn syscall_kill(pid: u64, sig: u64) -> u64 {
    result::ret(syscall_kill_impl(pid, sig))
}

unsafe fn syscall_kill_impl(pid: u64, sig: u64) -> SysResult {
    let slot = result::option(proc::find_slot_by_pid(pid as usize), SysError::NoEntry)?;
    if sig == 0 {
        return result::ok(0u64);
    }
    result::ensure(sig < 65, SysError::Invalid)?;
    if sig as usize == SIGKILL {
        terminate_process_group(slot, SIGKILL);
        return result::ok(0u64);
    }
    result::ensure(queue_signal(slot, sig as usize), SysError::NoEntry)?;
    result::ok(0u64)
}

pub unsafe fn syscall_tkill(tid: u64, sig: u64) -> u64 {
    result::ret(syscall_tkill_impl(tid, sig))
}

unsafe fn syscall_tkill_impl(tid: u64, sig: u64) -> SysResult {
    result::ensure(tid != 0, SysError::Invalid)?;
    result::ensure(sig < 65, SysError::Invalid)?;
    let slot = result::option(proc::find_thread_slot_by_tid(tid as usize), SysError::NoEntry)?;
    if sig == 0 {
        return result::ok(0u64);
    }
    if sig as usize == SIGKILL {
        let group_slot = result::option(proc::THREADS[slot].as_ref(), SysError::NoEntry)?.group_slot;
        terminate_process_group(group_slot, SIGKILL);
        return result::ok(0u64);
    }
    result::ensure(queue_thread_signal(slot, sig as usize), SysError::NoEntry)?;
    result::ok(0u64)
}

pub unsafe fn syscall_tgkill(tgid: u64, tid: u64, sig: u64) -> u64 {
    result::ret(syscall_tgkill_impl(tgid, tid, sig))
}

unsafe fn syscall_tgkill_impl(tgid: u64, tid: u64, sig: u64) -> SysResult {
    result::ensure(tgid != 0 && tid != 0, SysError::Invalid)?;
    let slot = result::option(proc::find_thread_slot_by_tid(tid as usize), SysError::NoEntry)?;
    let thread = result::option(proc::THREADS[slot].as_ref(), SysError::NoEntry)?;
    let process = result::option(proc::PROCESSES[thread.group_slot].as_ref(), SysError::NoEntry)?;
    result::ensure(process.pid == tgid as usize, SysError::NoEntry)?;
    syscall_tkill_impl(tid, sig)
}

pub unsafe fn syscall_getpgrp() -> u64 {
    proc::current_process().map(|p| p.pgid as u64).unwrap_or(0)
}

pub unsafe fn syscall_getpgid(pid: u64) -> u64 {
    result::ret(syscall_getpgid_impl(pid))
}

unsafe fn syscall_getpgid_impl(pid: u64) -> SysResult {
    if pid == 0 {
        return result::ok(proc::current_process().map(|p| p.pgid as u64).unwrap_or(0));
    }
    let slot = result::option(proc::find_slot_by_pid(pid as usize), SysError::NoEntry)?;
    let process = result::option(proc::PROCESSES[slot].as_ref(), SysError::NoEntry)?;
    result::ok(process.pgid as u64)
}

pub unsafe fn syscall_getsid(pid: u64) -> u64 {
    result::ret(syscall_getsid_impl(pid))
}

unsafe fn syscall_getsid_impl(pid: u64) -> SysResult {
    if pid == 0 {
        return result::ok(proc::current_process().map(|p| p.sid as u64).unwrap_or(0));
    }
    let slot = result::option(proc::find_slot_by_pid(pid as usize), SysError::NoEntry)?;
    let process = result::option(proc::PROCESSES[slot].as_ref(), SysError::NoEntry)?;
    result::ok(process.sid as u64)
}

pub unsafe fn syscall_setpgid(pid: u64, pgid: u64) -> u64 {
    result::ret(syscall_setpgid_impl(pid, pgid))
}

unsafe fn syscall_setpgid_impl(pid: u64, pgid: u64) -> SysResult {
    let current_pid = proc::current_pid();
    let target_pid = if pid == 0 { current_pid } else { pid as usize };
    result::ensure(target_pid == current_pid, SysError::Unsupported)?;

    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    let new_pgid = if pgid == 0 { target_pid } else { pgid as usize };
    result::ensure(new_pgid != 0, SysError::Invalid)?;
    result::ensure(
        new_pgid == target_pid || new_pgid == process.pgid,
        SysError::Unsupported,
    )?;
    process.pgid = new_pgid;
    result::ok(0u64)
}

pub unsafe fn syscall_umask(mask: u64) -> u64 {
    result::ret(syscall_umask_impl(mask))
}

unsafe fn syscall_umask_impl(mask: u64) -> SysResult {
    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    let old = process.umask;
    process.umask = (mask as u32) & UMASK_MASK;
    result::ok(old as u64)
}

pub unsafe fn syscall_getresuid(ruid_ptr: u64, euid_ptr: u64, suid_ptr: u64) -> u64 {
    result::ret(syscall_getresuid_impl(ruid_ptr, euid_ptr, suid_ptr))
}

unsafe fn syscall_getresuid_impl(ruid_ptr: u64, euid_ptr: u64, suid_ptr: u64) -> SysResult {
    let process = result::option(proc::current_process(), SysError::Invalid)?;
    result::ensure(
        helpers::copy_struct_to_user(ruid_ptr, &process.uid),
        SysError::Fault,
    )?;
    result::ensure(
        helpers::copy_struct_to_user(euid_ptr, &process.euid),
        SysError::Fault,
    )?;
    result::ensure(
        helpers::copy_struct_to_user(suid_ptr, &process.euid),
        SysError::Fault,
    )?;
    result::ok(0u64)
}

pub unsafe fn syscall_getresgid(rgid_ptr: u64, egid_ptr: u64, sgid_ptr: u64) -> u64 {
    result::ret(syscall_getresgid_impl(rgid_ptr, egid_ptr, sgid_ptr))
}

unsafe fn syscall_getresgid_impl(rgid_ptr: u64, egid_ptr: u64, sgid_ptr: u64) -> SysResult {
    let process = result::option(proc::current_process(), SysError::Invalid)?;
    result::ensure(
        helpers::copy_struct_to_user(rgid_ptr, &process.gid),
        SysError::Fault,
    )?;
    result::ensure(
        helpers::copy_struct_to_user(egid_ptr, &process.egid),
        SysError::Fault,
    )?;
    result::ensure(
        helpers::copy_struct_to_user(sgid_ptr, &process.egid),
        SysError::Fault,
    )?;
    result::ok(0u64)
}

pub unsafe fn syscall_rt_sigaction(sig: u64, act: u64, oldact: u64, sigsetsize: u64) -> u64 {
    result::ret(syscall_rt_sigaction_impl(sig, act, oldact, sigsetsize))
}

unsafe fn syscall_rt_sigaction_impl(sig: u64, act: u64, oldact: u64, sigsetsize: u64) -> SysResult {
    result::ensure(sig > 0 && sig < 65, SysError::Invalid)?;
    result::ensure(sigsetsize == SIGSET_SIZE, SysError::Invalid)?;
    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    if oldact != 0 {
        let current = process.sig_handlers[sig as usize];
        result::ensure(
            helpers::copy_struct_to_user(oldact, &current),
            SysError::Fault,
        )?;
    }
    if act != 0 {
        let new_action: SigAction =
            result::option(helpers::copy_struct_from_user(act), SysError::Fault)?;
        result::ensure(
            (new_action.flags & !(SA_RESTORER | SA_ONSTACK)) == 0,
            SysError::Unsupported,
        )?;
        if new_action.handler != 0 {
            result::ensure((new_action.flags & SA_RESTORER) != 0, SysError::Invalid)?;
            result::ensure(new_action.restorer != 0, SysError::Invalid)?;
        }
        process.sig_handlers[sig as usize] = new_action;
    }
    result::ok(0u64)
}

pub unsafe fn syscall_rt_sigprocmask(how: u64, set: u64, oldset: u64, sigsetsize: u64) -> u64 {
    result::ret(syscall_rt_sigprocmask_impl(how, set, oldset, sigsetsize))
}

unsafe fn syscall_rt_sigprocmask_impl(how: u64, set: u64, oldset: u64, sigsetsize: u64) -> SysResult {
    result::ensure(sigsetsize == SIGSET_SIZE, SysError::Invalid)?;
    result::ensure(matches!(how, SIG_BLOCK | SIG_UNBLOCK | SIG_SETMASK), SysError::Invalid)?;
    let thread = result::option(proc::current_thread_mut(), SysError::Invalid)?;
    if oldset != 0 {
        result::ensure(helpers::copy_struct_to_user(oldset, &thread.sig_mask), SysError::Fault)?;
    }
    if set != 0 {
        let mut new_set: SigSet = result::option(helpers::copy_struct_from_user(set), SysError::Fault)?;
        sanitize_sigmask(&mut new_set);
        match how {
            SIG_BLOCK => {
                for (dst, src) in thread.sig_mask.bits.iter_mut().zip(new_set.bits.iter()) {
                    *dst |= *src;
                }
            }
            SIG_UNBLOCK => {
                for (dst, src) in thread.sig_mask.bits.iter_mut().zip(new_set.bits.iter()) {
                    *dst &= !*src;
                }
            }
            SIG_SETMASK => thread.sig_mask = new_set,
            _ => return result::err(SysError::Invalid),
        }
        sanitize_sigmask(&mut thread.sig_mask);
    }
    result::ok(0u64)
}

pub unsafe fn syscall_rt_sigreturn(frame_ptr: u64) -> u64 {
    result::ret(syscall_rt_sigreturn_impl(frame_ptr))
}

unsafe fn syscall_rt_sigreturn_impl(frame_ptr: u64) -> SysResult {
    let frame = current_frame(frame_ptr)?;
    let saved_frame: SyscallFrame =
        result::option(helpers::copy_struct_from_user(frame.r10), SysError::Fault)?;
    *frame = saved_frame;
    let thread = result::option(proc::current_thread_mut(), SysError::Invalid)?;
    thread.sig_mask = thread.saved_sig_mask;
    thread.in_signal_handler = false;
    thread.on_altstack = false;
    result::ok(frame.rax)
}

pub unsafe fn syscall_fork(frame_ptr: u64) -> u64 {
    result::ret(syscall_fork_impl(frame_ptr))
}

unsafe fn syscall_fork_impl(frame_ptr: u64) -> SysResult {
    result::ensure(frame_ptr != 0, SysError::Invalid)?;

    let child_pml4 = paging::clone_user_address_space(task::current_pml4());
    let child_slot = result::option(
        task::spawn_forked_task(frame_ptr as *const u8, child_pml4),
        SysError::Invalid,
    )?;
    let child_pid = proc::fork_current(child_slot, child_pml4);
    result::ok(child_pid as u64)
}

pub unsafe fn syscall_clone(
    flags: u64,
    child_stack: u64,
    parent_tid_ptr: u64,
    child_tid_ptr: u64,
    frame_ptr: u64,
) -> u64 {
    result::ret(syscall_clone_impl(
        flags,
        child_stack,
        parent_tid_ptr,
        child_tid_ptr,
        frame_ptr,
    ))
}

unsafe fn syscall_clone_impl(
    flags: u64,
    child_stack: u64,
    parent_tid_ptr: u64,
    child_tid_ptr: u64,
    frame_ptr: u64,
) -> SysResult {
    let unsupported = flags & !(CLONE_SIGNAL_MASK | CLONE_SUPPORTED_MASK | CLONE_THREAD_FLAGS);
    result::ensure(unsupported == 0, SysError::Unsupported)?;
    result::ensure(
        (flags & (CLONE_VFORK
            | CLONE_PARENT
            | CLONE_PIDFD
            | CLONE_PTRACE
            | CLONE_NEWNS
            | CLONE_SYSVSEM
            | CLONE_DETACHED
            | CLONE_UNTRACED
            | CLONE_NEWCGROUP
            | CLONE_NEWUTS
            | CLONE_NEWIPC
            | CLONE_NEWUSER
            | CLONE_NEWPID
            | CLONE_NEWNET
            | CLONE_IO))
            == 0,
        SysError::Unsupported,
    )?;
    let exit_signal = (flags & CLONE_SIGNAL_MASK) as usize;
    let thread_clone = (flags & CLONE_THREAD) != 0;
    if thread_clone {
        result::ensure((flags & CLONE_THREAD_FLAGS) == CLONE_THREAD_FLAGS, SysError::Invalid)?;
        result::ensure(exit_signal == 0, SysError::Unsupported)?;
    } else {
        result::ensure(exit_signal == 0 || exit_signal == SIGCHLD, SysError::Unsupported)?;
    }

    let child_pml4 = if thread_clone {
        task::current_pml4()
    } else {
        paging::clone_user_address_space(task::current_pml4())
    };
    let child_slot = result::option(
        task::spawn_forked_task(frame_ptr as *const u8, child_pml4),
        SysError::Invalid,
    )?;
    let child_id = if thread_clone {
        let tls = if (flags & CLONE_SETTLS) != 0 {
            clone_tls_from_frame(frame_ptr)?
        } else {
            proc::current_fs_base()
        };
        proc::spawn_thread_in_group(
            child_slot,
            if (flags & CLONE_CHILD_CLEARTID) != 0 || (flags & CLONE_CHILD_SETTID) != 0 {
                child_tid_ptr
            } else {
                0
            },
            tls,
        )
    } else {
        proc::fork_current(child_slot, child_pml4)
    };
    if child_stack != 0 {
        let child_task = result::option(task::task_frame_mut(child_slot), SysError::Invalid)?;
        child_task.r10 = child_stack;
    }
    if (flags & CLONE_CHILD_SETTID) != 0 {
        let thread = result::option(proc::THREADS[child_slot].as_mut(), SysError::Invalid)?;
        thread.clear_child_tid = child_tid_ptr;
        result::ensure(
            helpers::copy_struct_to_user(child_tid_ptr, &(child_id as i32)),
            SysError::Fault,
        )?;
    }
    if (flags & CLONE_CHILD_CLEARTID) != 0 {
        let thread = result::option(proc::THREADS[child_slot].as_mut(), SysError::Invalid)?;
        thread.clear_child_tid = child_tid_ptr;
    }
    if parent_tid_ptr != 0 {
        result::ensure(
            helpers::copy_struct_to_user(parent_tid_ptr, &(child_id as i32)),
            SysError::Fault,
        )?;
    }
    result::ok(child_id as u64)
}

pub unsafe fn syscall_futex(
    uaddr: u64,
    futex_op: u64,
    val: u64,
    timeout_ptr: u64,
    _frame_ptr: u64,
) -> u64 {
    result::ret(syscall_futex_impl(uaddr, futex_op, val, timeout_ptr))
}

unsafe fn syscall_futex_impl(uaddr: u64, futex_op: u64, val: u64, timeout_ptr: u64) -> SysResult {
    result::ensure(crate::usercopy::user_range_ok(uaddr, core::mem::size_of::<u32>()), SysError::Fault)?;
    let op = futex_op & FUTEX_CMD_MASK;
    match op {
        FUTEX_WAIT => futex_wait(uaddr, val as u32, timeout_ptr),
        FUTEX_WAKE => futex_wake(uaddr, val as usize),
        _ => result::err(SysError::Unsupported),
    }
}

pub unsafe fn syscall_set_robust_list(head: u64, len: u64) -> u64 {
    result::ret(syscall_set_robust_list_impl(head, len))
}

unsafe fn syscall_set_robust_list_impl(head: u64, len: u64) -> SysResult {
    result::ensure(head != 0, SysError::Fault)?;
    result::ensure(
        len as usize == core::mem::size_of::<RobustListHead>(),
        SysError::Invalid,
    )?;
    result::ensure(
        crate::usercopy::user_range_ok(head, len as usize),
        SysError::Fault,
    )?;
    let thread = result::option(proc::current_thread_mut(), SysError::Invalid)?;
    thread.robust_list_head = head;
    thread.robust_list_len = len as usize;
    result::ok(0u64)
}

pub unsafe fn syscall_get_robust_list(pid: u64, head_ptr: u64, len_ptr: u64) -> u64 {
    result::ret(syscall_get_robust_list_impl(pid, head_ptr, len_ptr))
}

unsafe fn syscall_get_robust_list_impl(pid: u64, head_ptr: u64, len_ptr: u64) -> SysResult {
    result::ensure(head_ptr != 0 && len_ptr != 0, SysError::Fault)?;
    let slot = if pid == 0 {
        result::option(task::current_task_slot(), SysError::Invalid)?
    } else {
        result::option(proc::find_thread_slot_by_tid(pid as usize), SysError::NoEntry)?
    };
    let thread = result::option(proc::THREADS[slot].as_ref(), SysError::NoEntry)?;
    result::ensure(
        helpers::copy_struct_to_user(head_ptr, &thread.robust_list_head),
        SysError::Fault,
    )?;
    result::ensure(
        helpers::copy_struct_to_user(len_ptr, &thread.robust_list_len),
        SysError::Fault,
    )?;
    result::ok(0u64)
}

pub unsafe fn syscall_dup(oldfd: u64) -> u64 {
    result::ret(syscall_dup_impl(oldfd))
}

unsafe fn syscall_dup_impl(oldfd: u64) -> SysResult {
    result::ok(result::option(proc::dup_min(oldfd as usize, 0, false), SysError::BadFd)? as u64)
}

pub unsafe fn syscall_dup3(oldfd: u64, newfd: u64, flags: u64) -> u64 {
    result::ret(syscall_dup3_impl(oldfd, newfd, flags))
}

unsafe fn syscall_dup3_impl(oldfd: u64, newfd: u64, flags: u64) -> SysResult {
    result::ensure(flags & !O_CLOEXEC == 0, SysError::Invalid)?;
    result::ensure(oldfd != newfd, SysError::Invalid)?;
    let cloexec = (flags & O_CLOEXEC) != 0;
    let fd = proc::dup_exact(oldfd as usize, newfd as usize, cloexec);
    result::ok(result::option(fd, SysError::BadFd)? as u64)
}

pub unsafe fn syscall_fcntl(fd: u64, cmd: u64, arg: u64) -> u64 {
    result::ret(syscall_fcntl_impl(fd, cmd, arg))
}

unsafe fn syscall_fcntl_impl(fd: u64, cmd: u64, arg: u64) -> SysResult {
    match cmd {
        F_DUPFD => {
            result::ok(result::option(proc::dup_min(fd as usize, arg as usize, false), SysError::BadFd)? as u64)
        }
        F_DUPFD_CLOEXEC => {
            result::ok(result::option(proc::dup_min(fd as usize, arg as usize, true), SysError::BadFd)? as u64)
        }
        F_GETFD => result::ok(
            result::option(proc::get_fd_flags(fd as usize), SysError::BadFd)? as u64,
        ),
        F_SETFD => {
            let cloexec = (arg as u32 & proc::FD_CLOEXEC) != 0;
            result::ensure(proc::set_cloexec(fd as usize, cloexec), SysError::BadFd)?;
            result::ok(0u64)
        }
        F_GETFL => result::ok(
            (result::option(proc::get_status_flags(fd as usize), SysError::BadFd)? as u64)
                | O_RDONLY,
        ),
        F_SETFL => {
            let flags = (arg as u32) & FCNTL_STATUS_MASK;
            result::ensure(proc::set_status_flags(fd as usize, flags), SysError::BadFd)?;
            result::ok(0u64)
        }
        _ => result::err(SysError::Unsupported),
    }
}

pub unsafe fn syscall_pipe(pipefd_ptr: u64) -> u64 {
    result::ret(syscall_pipe_impl(pipefd_ptr))
}

unsafe fn syscall_pipe_impl(pipefd_ptr: u64) -> SysResult {
    result::ensure(
        crate::usercopy::user_range_ok(pipefd_ptr, core::mem::size_of::<[i32; 2]>()),
        SysError::Fault,
    )?;
    let (read_fd, write_fd) = result::option(proc::create_pipe_pair(), SysError::Invalid)?;
    let pipefds = [read_fd as i32, write_fd as i32];
    let bytes = core::slice::from_raw_parts(
        (&pipefds as *const [i32; 2]).cast::<u8>(),
        core::mem::size_of::<[i32; 2]>(),
    );
    if crate::usercopy::copy_to_user(pipefd_ptr, bytes).is_err() {
        let _ = proc::close_descriptor(read_fd);
        let _ = proc::close_descriptor(write_fd);
        return result::err(SysError::Fault);
    }
    result::ok(0u64)
}

pub unsafe fn syscall_pipe2(pipefd_ptr: u64, flags: u64) -> u64 {
    result::ret(syscall_pipe2_impl(pipefd_ptr, flags))
}

unsafe fn syscall_pipe2_impl(pipefd_ptr: u64, flags: u64) -> SysResult {
    result::ensure(flags & !(O_CLOEXEC | O_NONBLOCK) == 0, SysError::Invalid)?;
    syscall_pipe_impl(pipefd_ptr)?;

    let mut pipefds = [0i32; 2];
    let raw = core::slice::from_raw_parts_mut(
        pipefds.as_mut_ptr().cast::<u8>(),
        core::mem::size_of::<[i32; 2]>(),
    );
    result::ensure(
        crate::usercopy::copy_from_user(raw, pipefd_ptr).is_ok(),
        SysError::Fault,
    )?;

    for &fd in &pipefds {
        let fd = fd as usize;
        if (flags & O_CLOEXEC) != 0 {
            result::ensure(proc::set_cloexec(fd, true), SysError::BadFd)?;
        }
        if (flags & O_NONBLOCK) != 0 {
            result::ensure(proc::set_status_flags(fd, O_NONBLOCK as u32), SysError::BadFd)?;
        }
    }
    result::ok(0u64)
}

pub unsafe fn syscall_waitpid(pid: u64) -> u64 {
    result::ret(syscall_waitpid_impl(pid))
}

unsafe fn syscall_waitpid_impl(pid: u64) -> SysResult {
    let target_pid = pid as usize;
    result::ensure(target_pid > 0, SysError::Invalid)?;
    let self_slot = result::option(task::current_task_slot(), SysError::Invalid)?;
    let slot = result::option(proc::find_slot_by_pid(target_pid), SysError::NoEntry)?;
    result::ensure(slot != self_slot, SysError::Invalid)?;
    let self_pid = proc::current_pid();
    let target = result::option(proc::PROCESSES[slot].as_ref(), SysError::NoEntry)?;
    result::ensure(target.parent_pid == self_pid, SysError::Child)?;

    loop {
        if !proc::is_running_in_slot(slot) {
            break;
        }
        scheduler::yield_now();
        core::hint::spin_loop();
    }
    result::ok(result::option(proc::reap_slot(slot), SysError::Invalid)? as u64)
}

unsafe fn clear_child_tid_and_wake() {
    release_robust_list();
    let Some(thread) = proc::current_thread_mut() else {
        return;
    };
    let clear_child_tid = thread.clear_child_tid;
    if clear_child_tid == 0 {
        return;
    }
    let zero = 0u32;
    let _ = helpers::copy_struct_to_user(clear_child_tid, &zero);
    let _ = futex_wake(clear_child_tid, 1);
}

pub unsafe fn syscall_exit(code: u64) -> ! {
    crate::dbg_log!("SYSCALL", "exit({})", code);
    let parent_pid = proc::current_ppid();
    clear_child_tid_and_wake();
    let group_dead = proc::exit_thread(code as i64, false);
    if group_dead {
        notify_parent_exit(parent_pid);
    }
    if let Some(slot) = task::current_task_slot() {
        task::kill_task(slot);
    }
    scheduler::IN_SYSCALL.store(false, core::sync::atomic::Ordering::Relaxed);
    core::arch::asm!("sti");
    loop {
        core::arch::asm!("hlt");
    }
}

pub unsafe fn syscall_exit_group(code: u64) -> ! {
    crate::dbg_log!("SYSCALL", "exit_group({})", code);
    let parent_pid = proc::current_ppid();
    clear_child_tid_and_wake();
    let _ = proc::exit_thread(code as i64, true);
    notify_parent_exit(parent_pid);
    if let Some(slot) = task::current_task_slot() {
        task::kill_task(slot);
    }
    scheduler::IN_SYSCALL.store(false, core::sync::atomic::Ordering::Relaxed);
    core::arch::asm!("sti");
    loop {
        core::arch::asm!("hlt");
    }
}
