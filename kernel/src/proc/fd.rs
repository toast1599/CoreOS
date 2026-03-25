#[path = "fd_open.rs"]
mod fd_open;
#[path = "fd_pipe.rs"]
mod fd_pipe;

use super::{
    task, DescriptorInfo, FdTarget, OpenFile, Process, MAX_FDS, MAX_OPEN_FILES, MAX_PIPES,
    OPEN_FILES, PIPES, PROCESSES, FD_CLOEXEC,
};

unsafe fn alloc_open_file(file_idx: usize) -> Option<usize> {
    fd_open::alloc(file_idx)
}

unsafe fn retain_fd_target(target: FdTarget) -> bool {
    match target {
        FdTarget::Empty => false,
        FdTarget::Stdio(_) => true,
        FdTarget::Open(open_idx) => fd_open::retain(open_idx),
        FdTarget::PipeRead(pipe_idx) => fd_pipe::retain_read(pipe_idx),
        FdTarget::PipeWrite(pipe_idx) => fd_pipe::retain_write(pipe_idx),
    }
}

unsafe fn release_fd_target(target: FdTarget) {
    match target {
        FdTarget::Open(open_idx) => fd_open::release(open_idx),
        FdTarget::PipeRead(pipe_idx) => fd_pipe::release_read(pipe_idx),
        FdTarget::PipeWrite(pipe_idx) => fd_pipe::release_write(pipe_idx),
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
        parent_pid: parent.pid,
        pgid: parent.pgid,
        sid: parent.sid,
        state: super::ProcessState::Running,
        task_slot,
        exit_code: 0,
        uid: parent.uid,
        euid: parent.euid,
        gid: parent.gid,
        egid: parent.egid,
        umask: parent.umask,
        clear_child_tid: 0,
        fs_base: parent.fs_base,
        exe_path: parent.exe_path,
        exe_path_len: parent.exe_path_len,
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

pub unsafe fn open_file(file_idx: usize) -> Option<usize> {
    let fd = alloc_fd(file_idx);
    if fd < 0 { None } else { Some(fd as usize) }
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

pub unsafe fn descriptor_info(fd: usize) -> Option<DescriptorInfo> {
    match get_fd_target(fd)? {
        FdTarget::Stdio(index) => Some(DescriptorInfo::Stdio { index }),
        FdTarget::Open(open_idx) => fd_open::descriptor_info(open_idx),
        FdTarget::PipeRead(_) | FdTarget::PipeWrite(_) => Some(DescriptorInfo::Pipe),
        _ => None,
    }
}

pub unsafe fn get_fd_mut(fd: usize) -> Option<&'static mut OpenFile> {
    match get_fd_target(fd)? {
        FdTarget::Open(open_idx) => fd_open::get_mut(open_idx),
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

pub unsafe fn close_descriptor(fd: usize) -> bool {
    close_fd(fd)
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

pub unsafe fn dup_min(old_fd: usize, min_fd: usize, cloexec: bool) -> Option<usize> {
    let fd = dup_fd(old_fd, min_fd, None, cloexec);
    if fd < 0 { None } else { Some(fd as usize) }
}

pub unsafe fn dup_exact(old_fd: usize, new_fd: usize, cloexec: bool) -> Option<usize> {
    let fd = dup_fd(old_fd, 0, Some(new_fd), cloexec);
    if fd < 0 { None } else { Some(fd as usize) }
}

pub unsafe fn fd_exists(fd: usize) -> bool {
    get_fd_target(fd).is_some()
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

pub unsafe fn set_cloexec(fd: usize, cloexec: bool) -> bool {
    let flags = if cloexec { FD_CLOEXEC } else { 0 };
    set_fd_flags(fd, flags)
}

pub unsafe fn get_status_flags(fd: usize) -> Option<u32> {
    match get_fd_target(fd)? {
        FdTarget::Stdio(_) => Some(0),
        FdTarget::Open(open_idx) => fd_open::status_flags(open_idx),
        FdTarget::PipeRead(_) | FdTarget::PipeWrite(_) => fd_pipe::status_flags(fd),
        _ => None,
    }
}

pub unsafe fn file_size(fd: usize) -> Option<usize> {
    match descriptor_info(fd)? {
        DescriptorInfo::File { size, .. } => Some(size),
        _ => None,
    }
}

pub unsafe fn is_stdin(fd: usize) -> bool {
    matches!(descriptor_info(fd), Some(DescriptorInfo::Stdio { index: 0 }))
}

pub unsafe fn is_stdout_or_stderr(fd: usize) -> bool {
    matches!(
        descriptor_info(fd),
        Some(DescriptorInfo::Stdio { index: 1 | 2 })
    )
}

pub unsafe fn seek(fd: usize, offset: i64, whence: u64) -> Option<u64> {
    const SEEK_SET: u64 = 0;
    const SEEK_CUR: u64 = 1;
    const SEEK_END: u64 = 2;

    let of = get_fd_mut(fd)?;
    let file_size = crate::syscall::fs::fs_file_size(of.file_idx) as i64;
    let cur = of.offset as i64;

    let new_off = match whence {
        SEEK_SET => offset,
        SEEK_CUR => cur.saturating_add(offset),
        SEEK_END => file_size.saturating_add(offset),
        _ => return None,
    };
    if new_off < 0 {
        return None;
    }

    of.offset = new_off as usize;
    Some(new_off as u64)
}

pub unsafe fn set_status_flags(fd: usize, flags: u32) -> bool {
    match get_fd_target(fd) {
        Some(FdTarget::Stdio(_)) => true,
        Some(FdTarget::Open(open_idx)) => fd_open::set_status_flags(open_idx, flags),
        Some(FdTarget::PipeRead(_)) | Some(FdTarget::PipeWrite(_)) => {
            fd_pipe::set_status_flags(fd, flags)
        }
        _ => false,
    }
}

pub unsafe fn read_file(fd: usize, buf: *mut u8, count: usize) -> Option<usize> {
    let of = get_fd_mut(fd)?;
    let bytes_read = crate::syscall::fs::fs_read(of.file_idx, of.offset, buf, count);
    of.offset += bytes_read;
    Some(bytes_read)
}

pub unsafe fn write_pipe(fd: usize, buf: *const u8, len: usize) -> Option<usize> {
    fd_pipe::write(fd, buf, len)
}

pub unsafe fn read_pipe(fd: usize, buf: *mut u8, count: usize) -> Option<usize> {
    fd_pipe::read(fd, buf, count)
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
    let pipe_idx = fd_pipe::alloc()?;

    let read_fd = alloc_specific_fd(FdTarget::PipeRead(pipe_idx), 0);
    if read_fd < 0 {
        fd_pipe::clear(pipe_idx);
        return None;
    }
    let write_fd = alloc_specific_fd(FdTarget::PipeWrite(pipe_idx), 0);
    if write_fd < 0 {
        let _ = close_fd(read_fd as usize);
        fd_pipe::clear(pipe_idx);
        return None;
    }
    Some((read_fd as usize, write_fd as usize))
}

pub unsafe fn create_pipe_pair() -> Option<(usize, usize)> {
    alloc_pipe()
}
