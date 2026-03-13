/// process.rs — Single-process table.
///
/// Tracks the currently running userspace process.
/// One process at a time for now — extensible to a full table later.
use core::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// File descriptor table
// ---------------------------------------------------------------------------

/// Max open files per process.
pub const MAX_FDS: usize = 8;

/// An open file descriptor pointing into RamFS.
#[derive(Clone)]
pub struct OpenFile {
    /// Index into RamFS.files
    pub file_idx: usize,
    /// Read cursor (byte offset)
    pub offset: usize,
}

// ---------------------------------------------------------------------------
// Process state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ProcessState {
    /// Process is alive and scheduled.
    Running,
    /// Process called exit() — waiting for shell to reap.
    Zombie,
}

// ---------------------------------------------------------------------------
// Process descriptor
// ---------------------------------------------------------------------------

pub struct Process {
    pub pid: usize,
    pub state: ProcessState,
    pub task_slot: usize,
    pub exit_code: i64,
    pub program_break: usize,
    /// Per-process file descriptor table. fd 0/1/2 are stdin/stdout/stderr
    /// (handled specially in syscall.rs); fds 3+ index into this table as
    /// `fds[fd - 3]`.
    pub fds: [Option<OpenFile>; MAX_FDS],
}

// ---------------------------------------------------------------------------
// Process table
// ---------------------------------------------------------------------------

static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

/// The process table. Indexed by task slot (matching Task array in task.rs).
pub static mut PROCESSES: [Option<Process>; 8] = [None, None, None, None, None, None, None, None];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Spawn a new process entry in the given task slot.
pub unsafe fn spawn(task_slot: usize) -> usize {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    PROCESSES[task_slot] = Some(Process {
        pid,
        state: ProcessState::Running,
        task_slot,
        exit_code: 0,
        program_break: 0x4000_0000,
        fds: [None, None, None, None, None, None, None, None],
    });
    crate::dbg_log!("PROCESS", "spawned pid={} at slot={}", pid, task_slot);
    pid
}

pub unsafe fn current_brk() -> usize {
    if let Some(slot) = crate::task::current_task_slot() {
        if let Some(ref p) = PROCESSES[slot] {
            return p.program_break;
        }
    }
    0
}

pub unsafe fn set_brk(new_brk: usize) {
    if let Some(slot) = crate::task::current_task_slot() {
        if let Some(ref mut p) = PROCESSES[slot] {
            p.program_break = new_brk;
        }
    }
}

/// Called by syscall 60 (exit) to mark the current process as a zombie.
pub unsafe fn exit(code: i64) {
    if let Some(slot) = crate::task::current_task_slot() {
        if let Some(ref mut p) = PROCESSES[slot] {
            crate::dbg_log!("PROCESS", "slot={} (pid={}) exited with code={}", slot, p.pid, code);
            p.state = ProcessState::Zombie;
            p.exit_code = code;
        }
    }
}

/// Returns true if a user process is alive in the given slot.
pub fn is_running_in_slot(slot: usize) -> bool {
    unsafe {
        matches!(
            &PROCESSES[slot],
            Some(Process {
                state: ProcessState::Running,
                ..
            })
        )
    }
}

/// Reap a zombie process in a specific slot.
pub unsafe fn reap_slot(slot: usize) -> Option<i64> {
    if let Some(ref p) = PROCESSES[slot] {
        if p.state == ProcessState::Zombie {
            let code = p.exit_code;
            PROCESSES[slot] = None;
            crate::dbg_log!("PROCESS", "reaped slot={}, exit_code={}", slot, code);
            return Some(code);
        }
    }
    None
}

/// Allocate a new fd for the current process.
pub unsafe fn alloc_fd(file_idx: usize) -> i64 {
    let slot = match crate::task::current_task_slot() {
        Some(s) => s,
        None => return -1,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return -1,
    };
    for (i, slot_fd) in p.fds.iter_mut().enumerate() {
        if slot_fd.is_none() {
            *slot_fd = Some(OpenFile {
                file_idx,
                offset: 0,
            });
            let fd = (i + 3) as i64;
            return fd;
        }
    }
    -1
}

pub unsafe fn get_fd(fd: usize) -> Option<&'static OpenFile> {
    if fd < 3 { return None; }
    let slot = crate::task::current_task_slot()?;
    let p = PROCESSES[slot].as_ref()?;
    let idx = fd - 3;
    if idx >= MAX_FDS { return None; }
    p.fds[idx].as_ref()
}

pub unsafe fn get_fd_mut(fd: usize) -> Option<&'static mut OpenFile> {
    if fd < 3 { return None; }
    let slot = crate::task::current_task_slot()?;
    let p = PROCESSES[slot].as_mut()?;
    let idx = fd - 3;
    if idx >= MAX_FDS { return None; }
    p.fds[idx].as_mut()
}

pub unsafe fn close_fd(fd: usize) -> bool {
    if fd < 3 { return false; }
    let slot = match crate::task::current_task_slot() {
        Some(s) => s,
        None => return false,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return false,
    };
    let idx = fd - 3;
    if idx >= MAX_FDS { return false; }
    if p.fds[idx].is_some() {
        p.fds[idx] = None;
        true
    } else {
        false
    }
}

