use crate::arch::paging;
use crate::proc;
use crate::proc::scheduler;
use crate::proc::task;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use crate::syscall::types::{SigAction, SigSet};
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
const SIGSET_SIZE: u64 = core::mem::size_of::<SigSet>() as u64;
const SA_RESTORER: u64 = 0x0400_0000;
const UMASK_MASK: u32 = 0o777;

pub unsafe fn syscall_exec(path_ptr: u64, path_len: u64) -> u64 {
    result::ret(syscall_exec_impl(path_ptr, path_len))
}

unsafe fn syscall_exec_impl(path_ptr: u64, path_len: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(path_ptr, path_len), SysError::Fault)?;
    let elf_bytes = match crate::vfs::clone_bytes(&name_buf[..name_len]) {
        Some(v) => v,
        None => {
            crate::dbg_log!("SYSCALL", "exec: file not found");
            return result::err(SysError::NoEntry);
        }
    };
    let (pid, _slot) = crate::proc::exec::exec_as_task(elf_bytes.as_slice());
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
    proc::current_pid() as u64
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
    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    process.clear_child_tid = tidptr;
    result::ok(proc::current_pid() as u64)
}

pub unsafe fn syscall_kill(pid: u64, sig: u64) -> u64 {
    result::ret(syscall_kill_impl(pid, sig))
}

unsafe fn syscall_kill_impl(pid: u64, sig: u64) -> SysResult {
    let slot = result::option(proc::find_slot_by_pid(pid as usize), SysError::NoEntry)?;
    if sig == 0 {
        return result::ok(0u64);
    }
    result::ensure(matches!(sig, 9 | 15), SysError::Unsupported)?;

    if let Some(process) = proc::current_process_mut() {
        if process.pid == pid as usize {
            process.state = proc::ProcessState::Zombie;
            process.exit_code = 128 + sig as i64;
            task::kill_task(slot);
            return result::ok(0u64);
        }
    }

    result::err(SysError::Unsupported)
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
    if act != 0 {
        let new_action: SigAction =
            result::option(helpers::copy_struct_from_user(act), SysError::Fault)?;
        result::ensure(
            (new_action.flags & !SA_RESTORER) == 0,
            SysError::Unsupported,
        )?;
    }
    if oldact != 0 {
        let current = SigAction {
            handler: 0,
            flags: 0,
            restorer: 0,
            mask: SigSet { bits: [0; 16] },
        };
        result::ensure(
            helpers::copy_struct_to_user(oldact, &current),
            SysError::Fault,
        )?;
    }
    result::ok(0u64)
}

pub unsafe fn syscall_rt_sigprocmask(how: u64, set: u64, oldset: u64, sigsetsize: u64) -> u64 {
    result::ret(syscall_rt_sigprocmask_impl(how, set, oldset, sigsetsize))
}

unsafe fn syscall_rt_sigprocmask_impl(how: u64, set: u64, oldset: u64, sigsetsize: u64) -> SysResult {
    result::ensure(sigsetsize == SIGSET_SIZE, SysError::Invalid)?;
    result::ensure(how <= 2, SysError::Invalid)?;
    if set != 0 {
        let _: SigSet = result::option(helpers::copy_struct_from_user(set), SysError::Fault)?;
    }
    if oldset != 0 {
        let empty = SigSet { bits: [0; 16] };
        result::ensure(helpers::copy_struct_to_user(oldset, &empty), SysError::Fault)?;
    }
    result::ok(0u64)
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
    let cloexec = (flags & O_CLOEXEC) != 0;
    let fd = if oldfd == newfd {
        if !proc::fd_exists(oldfd as usize) {
            None
        } else if !proc::set_cloexec(oldfd as usize, cloexec) {
            None
        } else {
            Some(oldfd as usize)
        }
    } else {
        proc::dup_exact(oldfd as usize, newfd as usize, cloexec)
    };
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
    let self_slot = result::option(task::current_task_slot(), SysError::Invalid)?;
    let slot = result::option(proc::find_slot_by_pid(target_pid), SysError::NoEntry)?;
    result::ensure(slot != self_slot, SysError::Invalid)?;

    loop {
        if !proc::is_running_in_slot(slot) {
            break;
        }
        scheduler::yield_now();
        core::hint::spin_loop();
    }
    result::ok(result::option(proc::reap_slot(slot), SysError::Invalid)? as u64)
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
