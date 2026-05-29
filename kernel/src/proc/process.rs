use core::sync::atomic::Ordering;

use super::{task, Process, ProcessState, Thread, ThreadState, NEXT_PID, PROCESSES, THREADS};

#[allow(dead_code)]
pub unsafe fn spawn(task_slot: usize) -> usize {
    spawn_named(task_slot, &[])
}

/// Create a new thread-group leader in the given scheduler slot.
pub unsafe fn spawn_named(task_slot: usize, name: &[char]) -> usize {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    let parent_pid = current_pid();

    PROCESSES[task_slot] = Some(Process::new_leader(pid, parent_pid, name));
    THREADS[task_slot] = Some(Thread::new_leader(pid, parent_pid, task_slot, task_slot));

    crate::dbg_log!("PROCESS", "spawned pid={} at slot={}", pid, task_slot);
    pid
}

pub unsafe fn current_brk() -> usize {
    current_process().map(|p| p.program_break).unwrap_or(0)
}

pub unsafe fn current_pid() -> usize {
    current_process().map(|p| p.pid).unwrap_or(0)
}

pub unsafe fn current_tid() -> usize {
    current_thread().map(|t| t.tid).unwrap_or(0)
}

pub unsafe fn current_ppid() -> usize {
    current_process().map(|p| p.parent_pid).unwrap_or(0)
}

pub unsafe fn active_process_count() -> usize {
    PROCESSES.iter().filter(|process| process.is_some()).count()
}

pub unsafe fn set_brk(new_brk: usize) {
    if let Some(ref mut p) = current_process_mut() {
        p.program_break = new_brk;
    }
}

pub unsafe fn current_fs_base() -> u64 {
    current_thread().map(|t| t.fs_base).unwrap_or(0)
}

pub unsafe fn current_exe_path() -> ([u8; super::EXE_PATH_MAX], usize) {
    match current_process() {
        Some(process) => (process.exe_path, process.exe_path_len),
        None => ([0; super::EXE_PATH_MAX], 0),
    }
}

pub unsafe fn current_thread_mut() -> Option<&'static mut Thread> {
    if let Some(slot) = task::current_task_slot() {
        return THREADS[slot].as_mut();
    }
    None
}

pub unsafe fn current_thread() -> Option<&'static Thread> {
    if let Some(slot) = task::current_task_slot() {
        return THREADS[slot].as_ref();
    }
    None
}

pub unsafe fn current_process_mut() -> Option<&'static mut Process> {
    let group_slot = current_thread()?.group_slot;
    PROCESSES[group_slot].as_mut()
}

pub unsafe fn current_process() -> Option<&'static Process> {
    let group_slot = current_thread()?.group_slot;
    PROCESSES[group_slot].as_ref()
}

/// Exit the current thread and mark the process zombie if this was its last thread.
pub unsafe fn exit(code: i64) {
    let _ = exit_thread(code, false);
}

/// Exit the current thread and optionally tear down the whole thread group.
pub unsafe fn exit_thread(code: i64, whole_group: bool) -> bool {
    let Some(thread) = current_thread_mut() else {
        return false;
    };
    let task_slot = thread.task_slot;
    let group_slot = thread.group_slot;
    thread.state = ThreadState::Zombie;

    let Some(process) = PROCESSES[group_slot].as_mut() else {
        return false;
    };
    crate::dbg_log!(
        "PROCESS",
        "thread slot={} tid={} group_pid={} exit={} whole_group={}",
        task_slot,
        thread.tid,
        process.pid,
        code,
        whole_group as u8
    );

    if whole_group {
        process.state = ProcessState::Zombie;
        process.exit_code = code;
        process.thread_count = 0;
        for (slot, maybe_thread) in THREADS.iter_mut().enumerate() {
            let Some(group_thread) = maybe_thread.as_mut() else {
                continue;
            };
            if group_thread.group_slot != group_slot {
                continue;
            }
            group_thread.state = ThreadState::Zombie;
            if slot != task_slot {
                super::task::kill_task(slot);
            }
        }
        return true;
    }

    if process.thread_count > 0 {
        process.thread_count -= 1;
    }
    if process.thread_count == 0 {
        process.state = ProcessState::Zombie;
        process.exit_code = code;
        return true;
    }
    false
}

pub unsafe fn task_slot_reaped(slot: usize) {
    let Some(thread) = THREADS[slot].as_ref() else {
        return;
    };
    if thread.group_slot != slot {
        crate::dbg_log!(
            "PROCESS",
            "dropping thread slot={} tid={} group_slot={}",
            slot,
            thread.tid,
            thread.group_slot
        );
        THREADS[slot] = None;
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

pub unsafe fn find_thread_slot_by_tid(tid: usize) -> Option<usize> {
    THREADS
        .iter()
        .enumerate()
        .find_map(|(slot, thread)| match thread {
            Some(t) if t.tid == tid => Some(slot),
            _ => None,
        })
}

pub unsafe fn spawn_thread_in_group(task_slot: usize, clear_child_tid: u64, fs_base: u64) -> usize {
    let tid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    let current = current_thread().expect("spawn_thread_in_group without current thread");
    let group_slot = current.group_slot;
    let process = PROCESSES[group_slot]
        .as_mut()
        .expect("spawn_thread_in_group without process");
    process.thread_count += 1;
    THREADS[task_slot] = Some(Thread::new_group_member(
        tid,
        current.tid,
        group_slot,
        task_slot,
        clear_child_tid,
        fs_base,
        current.sig_mask,
    ));
    tid
}
