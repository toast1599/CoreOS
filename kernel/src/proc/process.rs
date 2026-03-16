use core::sync::atomic::Ordering;

use super::{
    default_fd_flags, default_fds, task, Process, ProcessState, VmRegion, MMAP_BASE, NEXT_PID,
    PROCESSES,
};

pub unsafe fn spawn(task_slot: usize, pml4: usize) -> usize {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    PROCESSES[task_slot] = Some(Process {
        pid,
        state: ProcessState::Running,
        task_slot,
        exit_code: 0,
        program_break: 0x4000_0000,
        next_mmap_base: MMAP_BASE,
        pml4,
        fds: default_fds(),
        fd_flags: default_fd_flags(),
        vmas: [VmRegion::empty(); super::MAX_VMAS],
    });
    crate::dbg_log!("PROCESS", "spawned pid={} at slot={}", pid, task_slot);
    pid
}

pub unsafe fn current_brk() -> usize {
    if let Some(slot) = task::current_task_slot() {
        if let Some(ref p) = PROCESSES[slot] {
            return p.program_break;
        }
    }
    0
}

pub unsafe fn current_pid() -> usize {
    if let Some(slot) = task::current_task_slot() {
        if let Some(ref p) = PROCESSES[slot] {
            return p.pid;
        }
    }
    0
}

pub unsafe fn set_brk(new_brk: usize) {
    if let Some(slot) = task::current_task_slot() {
        if let Some(ref mut p) = PROCESSES[slot] {
            p.program_break = new_brk;
        }
    }
}

pub unsafe fn current_process_mut() -> Option<&'static mut Process> {
    let slot = task::current_task_slot()?;
    PROCESSES[slot].as_mut()
}

pub unsafe fn current_process() -> Option<&'static Process> {
    let slot = task::current_task_slot()?;
    PROCESSES[slot].as_ref()
}

pub unsafe fn exit(code: i64) {
    if let Some(slot) = task::current_task_slot() {
        if let Some(ref mut p) = PROCESSES[slot] {
            crate::dbg_log!(
                "PROCESS",
                "slot={} (pid={}) exited with code={}",
                slot,
                p.pid,
                code
            );
            p.state = ProcessState::Zombie;
            p.exit_code = code;
        }
    }
}

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
