/// process.rs — Single-process table.
///
/// Tracks the currently running userspace process.
/// One process at a time for now — extensible to a full table later.
use core::sync::atomic::{AtomicUsize, Ordering};

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
    pub task_slot: usize, // index into task::TASKS[]
    pub exit_code: i64,
}

// ---------------------------------------------------------------------------
// Global process slot
// ---------------------------------------------------------------------------

/// PID counter — incremented on each new process.
static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

/// The single current userspace process. `None` means no process running.
static mut CURRENT_PROCESS: Option<Process> = None;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Spawn a new process entry. Called by exec before creating the task.
/// Returns the assigned PID.
pub unsafe fn spawn(task_slot: usize) -> usize {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    CURRENT_PROCESS = Some(Process {
        pid,
        state: ProcessState::Running,
        task_slot,
        exit_code: 0,
    });
    crate::dbg_log!("PROCESS", "spawned pid={} task_slot={}", pid, task_slot);
    pid
}

/// Called by syscall 60 (exit) to mark the process as a zombie.
pub unsafe fn exit(code: i64) {
    if let Some(ref mut p) = CURRENT_PROCESS {
        crate::dbg_log!("PROCESS", "pid={} exited with code={}", p.pid, code);
        p.state = ProcessState::Zombie;
        p.exit_code = code;
    }
}

/// Returns true if a user process is currently alive (not zombie, not absent).
pub fn is_running() -> bool {
    unsafe {
        matches!(
            &CURRENT_PROCESS,
            Some(Process {
                state: ProcessState::Running,
                ..
            })
        )
    }
}

/// Reap a zombie process — clears the slot so a new process can run.
/// Returns the exit code, or None if no zombie exists.
pub unsafe fn reap() -> Option<i64> {
    if let Some(ref p) = CURRENT_PROCESS {
        if p.state == ProcessState::Zombie {
            let code = p.exit_code;
            CURRENT_PROCESS = None;
            crate::dbg_log!("PROCESS", "reaped zombie, exit_code={}", code);
            return Some(code);
        }
    }
    None
}

/// Returns the task slot of the current process, if any.
pub unsafe fn current_task_slot() -> Option<usize> {
    CURRENT_PROCESS.as_ref().map(|p| p.task_slot)
}

