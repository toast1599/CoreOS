use super::{
    task, FdTarget, OpenFile, Pipe, Process, MAX_FDS, MAX_OPEN_FILES, MAX_PIPES, OPEN_FILES,
    PIPES, PROCESSES, FD_CLOEXEC,
};

unsafe fn alloc_open_file(file_idx: usize) -> Option<usize> {
    for (i, of) in OPEN_FILES.iter_mut().enumerate() {
        if !of.in_use {
            *of = OpenFile {
                file_idx,
                offset: 0,
                status_flags: 0,
                refs: 1,
                in_use: true,
            };
            return Some(i);
        }
    }
    None
}

unsafe fn retain_fd_target(target: FdTarget) -> bool {
    match target {
        FdTarget::Empty => false,
        FdTarget::Stdio(_) => true,
        FdTarget::Open(open_idx) => {
            if open_idx >= MAX_OPEN_FILES || !OPEN_FILES[open_idx].in_use {
                return false;
            }
            OPEN_FILES[open_idx].refs += 1;
            true
        }
        FdTarget::PipeRead(pipe_idx) => {
            if pipe_idx >= MAX_PIPES || !PIPES[pipe_idx].in_use {
                return false;
            }
            PIPES[pipe_idx].read_refs += 1;
            true
        }
        FdTarget::PipeWrite(pipe_idx) => {
            if pipe_idx >= MAX_PIPES || !PIPES[pipe_idx].in_use {
                return false;
            }
            PIPES[pipe_idx].write_refs += 1;
            true
        }
    }
}

unsafe fn release_fd_target(target: FdTarget) {
    match target {
        FdTarget::Open(open_idx) => {
            if open_idx < MAX_OPEN_FILES && OPEN_FILES[open_idx].in_use {
                let of = &mut OPEN_FILES[open_idx];
                if of.refs > 1 {
                    of.refs -= 1;
                } else {
                    *of = OpenFile::empty();
                }
            }
        }
        FdTarget::PipeRead(pipe_idx) => {
            if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use {
                let pipe = &mut PIPES[pipe_idx];
                if pipe.read_refs > 0 {
                    pipe.read_refs -= 1;
                }
                if pipe.read_refs == 0 && pipe.write_refs == 0 {
                    *pipe = Pipe::empty();
                }
            }
        }
        FdTarget::PipeWrite(pipe_idx) => {
            if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use {
                let pipe = &mut PIPES[pipe_idx];
                if pipe.write_refs > 0 {
                    pipe.write_refs -= 1;
                }
                if pipe.read_refs == 0 && pipe.write_refs == 0 {
                    *pipe = Pipe::empty();
                }
            }
        }
        _ => {}
    }
}

pub unsafe fn release_fds(fds: &[FdTarget; MAX_FDS]) {
    for &target in fds.iter() {
        release_fd_target(target);
    }
}

pub unsafe fn fork_current(task_slot: usize, pml4: usize) -> usize {
    let pid = super::NEXT_PID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let parent = super::current_process().expect("fork_current without current process");
    let fds = parent.fds;
    for target in fds {
        match target {
            FdTarget::Open(open_idx) => {
                if open_idx < MAX_OPEN_FILES && OPEN_FILES[open_idx].in_use {
                    OPEN_FILES[open_idx].refs += 1;
                }
            }
            FdTarget::PipeRead(pipe_idx) => {
                if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use {
                    PIPES[pipe_idx].read_refs += 1;
                }
            }
            FdTarget::PipeWrite(pipe_idx) => {
                if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use {
                    PIPES[pipe_idx].write_refs += 1;
                }
            }
            _ => {}
        }
    }
    PROCESSES[task_slot] = Some(Process {
        pid,
        state: super::ProcessState::Running,
        task_slot,
        exit_code: 0,
        program_break: parent.program_break,
        next_mmap_base: parent.next_mmap_base,
        pml4,
        fds,
        fd_flags: parent.fd_flags,
        vmas: parent.vmas,
    });
    crate::dbg_log!("PROCESS", "forked pid={} at slot={}", pid, task_slot);
    pid
}

pub unsafe fn reap_slot(slot: usize) -> Option<i64> {
    if let Some(ref p) = PROCESSES[slot] {
        if p.state == super::ProcessState::Zombie {
            let code = p.exit_code;
            release_fds(&p.fds);
            PROCESSES[slot] = None;
            crate::dbg_log!("PROCESS", "reaped slot={}, exit_code={}", slot, code);
            return Some(code);
        }
    }
    None
}

pub unsafe fn alloc_fd(file_idx: usize) -> i64 {
    let slot = match task::current_task_slot() {
        Some(s) => s,
        None => return -1,
    };
    let open_idx = match alloc_open_file(file_idx) {
        Some(i) => i,
        None => return -1,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return -1,
    };
    for fd in 3..MAX_FDS {
        if matches!(p.fds[fd], FdTarget::Empty) {
            p.fds[fd] = FdTarget::Open(open_idx);
            p.fd_flags[fd] = 0;
            return fd as i64;
        }
    }
    release_fd_target(FdTarget::Open(open_idx));
    -1
}

pub unsafe fn get_fd_target(fd: usize) -> Option<FdTarget> {
    let slot = task::current_task_slot()?;
    let p = PROCESSES[slot].as_ref()?;
    if fd >= MAX_FDS {
        return None;
    }
    match p.fds[fd] {
        FdTarget::Empty => None,
        target => Some(target),
    }
}

pub unsafe fn get_fd(fd: usize) -> Option<&'static OpenFile> {
    match get_fd_target(fd)? {
        FdTarget::Open(open_idx) if open_idx < MAX_OPEN_FILES && OPEN_FILES[open_idx].in_use => {
            Some(&OPEN_FILES[open_idx])
        }
        _ => None,
    }
}

pub unsafe fn get_fd_mut(fd: usize) -> Option<&'static mut OpenFile> {
    match get_fd_target(fd)? {
        FdTarget::Open(open_idx) if open_idx < MAX_OPEN_FILES && OPEN_FILES[open_idx].in_use => {
            Some(&mut OPEN_FILES[open_idx])
        }
        _ => None,
    }
}

pub unsafe fn get_pipe_mut(fd: usize) -> Option<(&'static mut Pipe, bool)> {
    match get_fd_target(fd)? {
        FdTarget::PipeRead(pipe_idx) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            Some((&mut PIPES[pipe_idx], true))
        }
        FdTarget::PipeWrite(pipe_idx) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            Some((&mut PIPES[pipe_idx], false))
        }
        _ => None,
    }
}

pub unsafe fn pipe_peer_closed(fd: usize) -> Option<bool> {
    match get_fd_target(fd)? {
        FdTarget::PipeRead(pipe_idx) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            Some(PIPES[pipe_idx].write_refs == 0)
        }
        FdTarget::PipeWrite(pipe_idx) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            Some(PIPES[pipe_idx].read_refs == 0)
        }
        _ => None,
    }
}

pub unsafe fn close_fd(fd: usize) -> bool {
    let slot = match task::current_task_slot() {
        Some(s) => s,
        None => return false,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return false,
    };
    if fd >= MAX_FDS {
        return false;
    }
    let target = p.fds[fd];
    if matches!(target, FdTarget::Empty) {
        return false;
    }
    p.fds[fd] = FdTarget::Empty;
    p.fd_flags[fd] = 0;
    release_fd_target(target);
    true
}

pub unsafe fn dup_fd(old_fd: usize, min_fd: usize, new_fd: Option<usize>, cloexec: bool) -> i64 {
    let slot = match task::current_task_slot() {
        Some(s) => s,
        None => return -1,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return -1,
    };
    if old_fd >= MAX_FDS {
        return -1;
    }
    let target = p.fds[old_fd];
    if matches!(target, FdTarget::Empty) {
        return -1;
    }

    let dest_fd = match new_fd {
        Some(fd) => {
            if fd >= MAX_FDS {
                return -1;
            }
            if fd == old_fd {
                return fd as i64;
            }
            fd
        }
        None => match p
            .fds
            .iter()
            .enumerate()
            .skip(min_fd)
            .find(|(_, entry)| matches!(entry, FdTarget::Empty))
            .map(|(fd, _)| fd)
        {
            Some(fd) => fd,
            None => return -1,
        },
    };

    if !matches!(p.fds[dest_fd], FdTarget::Empty) {
        let old_target = p.fds[dest_fd];
        p.fds[dest_fd] = FdTarget::Empty;
        release_fd_target(old_target);
    }
    if !retain_fd_target(target) {
        return -1;
    }
    p.fds[dest_fd] = target;
    p.fd_flags[dest_fd] = if cloexec { FD_CLOEXEC } else { 0 };
    dest_fd as i64
}

pub unsafe fn get_fd_flags(fd: usize) -> Option<u32> {
    let slot = task::current_task_slot()?;
    let p = PROCESSES[slot].as_ref()?;
    if fd >= MAX_FDS || matches!(p.fds[fd], FdTarget::Empty) {
        return None;
    }
    Some(p.fd_flags[fd])
}

pub unsafe fn set_fd_flags(fd: usize, flags: u32) -> bool {
    let slot = match task::current_task_slot() {
        Some(s) => s,
        None => return false,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return false,
    };
    if fd >= MAX_FDS || matches!(p.fds[fd], FdTarget::Empty) {
        return false;
    }
    p.fd_flags[fd] = flags;
    true
}

pub unsafe fn get_status_flags(fd: usize) -> Option<u32> {
    match get_fd_target(fd)? {
        FdTarget::Stdio(_) => Some(0),
        FdTarget::Open(open_idx) if open_idx < MAX_OPEN_FILES && OPEN_FILES[open_idx].in_use => {
            Some(OPEN_FILES[open_idx].status_flags)
        }
        FdTarget::PipeRead(pipe_idx) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            Some(PIPES[pipe_idx].read_flags)
        }
        FdTarget::PipeWrite(pipe_idx) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            Some(PIPES[pipe_idx].write_flags)
        }
        _ => None,
    }
}

pub unsafe fn set_status_flags(fd: usize, flags: u32) -> bool {
    match get_fd_target(fd) {
        Some(FdTarget::Stdio(_)) => true,
        Some(FdTarget::Open(open_idx))
            if open_idx < MAX_OPEN_FILES && OPEN_FILES[open_idx].in_use =>
        {
            OPEN_FILES[open_idx].status_flags = flags;
            true
        }
        Some(FdTarget::PipeRead(pipe_idx)) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            PIPES[pipe_idx].read_flags = flags;
            true
        }
        Some(FdTarget::PipeWrite(pipe_idx)) if pipe_idx < MAX_PIPES && PIPES[pipe_idx].in_use => {
            PIPES[pipe_idx].write_flags = flags;
            true
        }
        _ => false,
    }
}

unsafe fn alloc_specific_fd(target: FdTarget, fd_flags: u32) -> i64 {
    let slot = match task::current_task_slot() {
        Some(s) => s,
        None => return -1,
    };
    let p = match PROCESSES[slot].as_mut() {
        Some(p) => p,
        None => return -1,
    };
    for fd in 3..MAX_FDS {
        if matches!(p.fds[fd], FdTarget::Empty) {
            p.fds[fd] = target;
            p.fd_flags[fd] = fd_flags;
            return fd as i64;
        }
    }
    -1
}

pub unsafe fn alloc_pipe() -> Option<(usize, usize)> {
    let pipe_idx = PIPES.iter().position(|p| !p.in_use)?;
    PIPES[pipe_idx] = Pipe {
        buf: [0; super::PIPE_CAPACITY],
        read_pos: 0,
        write_pos: 0,
        len: 0,
        read_refs: 1,
        write_refs: 1,
        read_flags: 0,
        write_flags: 0,
        in_use: true,
    };

    let read_fd = alloc_specific_fd(FdTarget::PipeRead(pipe_idx), 0);
    if read_fd < 0 {
        PIPES[pipe_idx] = Pipe::empty();
        return None;
    }
    let write_fd = alloc_specific_fd(FdTarget::PipeWrite(pipe_idx), 0);
    if write_fd < 0 {
        let _ = close_fd(read_fd as usize);
        PIPES[pipe_idx] = Pipe::empty();
        return None;
    }
    Some((read_fd as usize, write_fd as usize))
}
