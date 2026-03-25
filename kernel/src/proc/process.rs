use core::sync::atomic::Ordering;

use super::{
    default_fd_flags, default_fds, task, Process, ProcessState, VmRegion, MMAP_BASE, NEXT_PID,
    PROCESSES,
};

#[allow(dead_code)]
pub unsafe fn spawn(task_slot: usize, pml4: usize) -> usize {
    spawn_named(task_slot, pml4, &[])
}

pub unsafe fn spawn_named(task_slot: usize, pml4: usize, name: &[char]) -> usize {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    let parent_pid = current_pid();
    let mut exe_path = [0u8; super::EXE_PATH_MAX];
    let exe_path_len = name.len().min(super::EXE_PATH_MAX);
    for (i, ch) in name.iter().take(exe_path_len).enumerate() {
        exe_path[i] = *ch as u8;
    }
    PROCESSES[task_slot] = Some(Process {
        pid,
        parent_pid,
        pgid: pid,
        sid: pid,
        state: ProcessState::Running,
        task_slot,
        exit_code: 0,
        uid: 0,
        euid: 0,
        gid: 0,
        egid: 0,
        umask: 0o022,
        clear_child_tid: 0,
        fs_base: 0,
        exe_path,
        exe_path_len,
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

pub unsafe fn current_ppid() -> usize {
    if let Some(slot) = task::current_task_slot() {
        if let Some(ref p) = PROCESSES[slot] {
            return p.parent_pid;
        }
    }
    0
}

pub unsafe fn active_process_count() -> usize {
    PROCESSES.iter().filter(|process| process.is_some()).count()
}

pub unsafe fn set_brk(new_brk: usize) {
    if let Some(slot) = task::current_task_slot() {
        if let Some(ref mut p) = PROCESSES[slot] {
            p.program_break = new_brk;
        }
    }
}

pub unsafe fn current_fs_base() -> u64 {
    current_process().map(|p| p.fs_base).unwrap_or(0)
}

pub unsafe fn current_exe_path() -> ([u8; super::EXE_PATH_MAX], usize) {
    match current_process() {
        Some(process) => (process.exe_path, process.exe_path_len),
        None => ([0; super::EXE_PATH_MAX], 0),
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

pub unsafe fn find_slot_by_pid(pid: usize) -> Option<usize> {
    PROCESSES
        .iter()
        .enumerate()
        .find_map(|(slot, process)| match process {
            Some(p) if p.pid == pid => Some(slot),
            _ => None,
        })
}
