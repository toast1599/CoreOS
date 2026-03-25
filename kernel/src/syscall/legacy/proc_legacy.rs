// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

use crate::proc::process;

/// getpid() - Get process ID
pub fn syscall_getpid() -> i64 {
    let current = process::current_task();
    let task = current.lock();
    task.pid as i64
}

/// getppid() - Get parent process ID
pub fn syscall_getppid() -> i64 {
    let current = process::current_task();
    let task = current.lock();
    task.parent_pid as i64
}

/// getuid() - Get real user ID
pub fn syscall_getuid() -> i64 {
    let current = process::current_task();
    let task = current.lock();
    task.uid as i64
}

/// geteuid() - Get effective user ID
pub fn syscall_geteuid() -> i64 {
    let current = process::current_task();
    let task = current.lock();
    task.euid as i64
}

/// getgid() - Get real group ID
pub fn syscall_getgid() -> i64 {
    let current = process::current_task();
    let task = current.lock();
    task.gid as i64
}

/// getegid() - Get effective group ID
pub fn syscall_getegid() -> i64 {
    let current = process::current_task();
    let task = current.lock();
    task.egid as i64
}

/// setuid(uid) - Set user ID (stub - always succeeds for root)
pub fn syscall_setuid(uid: u32) -> i64 {
    let current = process::current_task();
    let mut task = current.lock();

    // For now, always allow (we're running as root)
    task.uid = uid;
    task.euid = uid;
    0
}

/// setgid(gid) - Set group ID (stub - always succeeds for root)
pub fn syscall_setgid(gid: u32) -> i64 {
    let current = process::current_task();
    let mut task = current.lock();

    task.gid = gid;
    task.egid = gid;
    0
}

/// kill(pid, sig) - Send signal to process
/// Basic implementation: only handles SIGKILL (9) and SIGTERM (15)
pub fn syscall_kill(pid: i32, sig: i32) -> i64 {
    if pid <= 0 {
        return -22; // EINVAL - no broadcast/process groups yet
    }

    // Only handle termination signals for now
    if sig == 9 || sig == 15 {
        // SIGKILL or SIGTERM
        match process::kill_process(pid) {
            Ok(()) => 0,
            Err(_) => -3, // ESRCH - no such process
        }
    } else if sig == 0 {
        // sig 0 = check if process exists
        match process::find_task(pid) {
            Some(_) => 0,
            None => -3, // ESRCH
        }
    } else {
        // Other signals not implemented yet
        0 // Pretend it worked
    }
}

/// Signal action stubs
const EINVAL: i64 = -22;

/// rt_sigaction(sig, act, oldact, sigsetsize) - Stub
pub fn syscall_rt_sigaction(_sig: i32, _act: u64, _oldact: u64, _sigsetsize: u64) -> i64 {
    // For now, just return success
    // musl checks this but doesn't strictly require it to work
    0
}

/// rt_sigprocmask(how, set, oldset, sigsetsize) - Stub
pub fn syscall_rt_sigprocmask(_how: i32, _set: u64, _oldset: u64, _sigsetsize: u64) -> i64 {
    // Return success without doing anything
    // Signal masks are not implemented yet
    0
}
