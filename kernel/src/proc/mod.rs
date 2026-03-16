pub mod elf;
pub mod exec;
pub mod scheduler;
pub mod task;

use core::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// File descriptor table
// ---------------------------------------------------------------------------

/// Max file descriptors per process, including stdin/stdout/stderr.
pub const MAX_FDS: usize = 16;
pub const MAX_OPEN_FILES: usize = 32;
pub const MAX_PIPES: usize = 16;
pub const MAX_VMAS: usize = 32;
pub const MMAP_BASE: usize = 0x0000_0001_0000_0000;
pub const FD_CLOEXEC: u32 = 1;
const PIPE_CAPACITY: usize = 1024;

/// A shared open file description pointing into RamFS.
#[derive(Clone, Copy)]
pub struct OpenFile {
    /// Index into RamFS.files
    pub file_idx: usize,
    /// Read cursor (byte offset)
    pub offset: usize,
    /// Open-file status flags shared by dup'd descriptors.
    pub status_flags: u32,
    /// Number of descriptors referencing this open file description.
    pub refs: usize,
    pub in_use: bool,
}

impl OpenFile {
    pub const fn empty() -> Self {
        Self {
            file_idx: 0,
            offset: 0,
            status_flags: 0,
            refs: 0,
            in_use: false,
        }
    }
}

#[derive(Clone, Copy)]
pub enum FdTarget {
    Empty,
    Stdio(u8),
    Open(usize),
    PipeRead(usize),
    PipeWrite(usize),
}

#[derive(Clone, Copy)]
pub struct Pipe {
    pub buf: [u8; PIPE_CAPACITY],
    pub read_pos: usize,
    pub write_pos: usize,
    pub len: usize,
    pub read_refs: usize,
    pub write_refs: usize,
    pub read_flags: u32,
    pub write_flags: u32,
    pub in_use: bool,
}

impl Pipe {
    pub const fn empty() -> Self {
        Self {
            buf: [0; PIPE_CAPACITY],
            read_pos: 0,
            write_pos: 0,
            len: 0,
            read_refs: 0,
            write_refs: 0,
            read_flags: 0,
            write_flags: 0,
            in_use: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct VmRegion {
    pub start: usize,
    pub len: usize,
    pub prot: u32,
    #[allow(dead_code)]
    pub flags: u32,
    pub in_use: bool,
}

impl VmRegion {
    pub const fn empty() -> Self {
        Self {
            start: 0,
            len: 0,
            prot: 0,
            flags: 0,
            in_use: false,
        }
    }
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

#[allow(dead_code)]

pub struct Process {
    pub pid: usize,
    pub state: ProcessState,
    pub task_slot: usize,
    pub exit_code: i64,
    pub program_break: usize,
    pub next_mmap_base: usize,
    pub pml4: usize,
    /// Per-process file descriptor table. 0/1/2 start as stdin/stdout/stderr.
    pub fds: [FdTarget; MAX_FDS],
    pub fd_flags: [u32; MAX_FDS],
    pub vmas: [VmRegion; MAX_VMAS],
}

// ---------------------------------------------------------------------------
// Process table
// ---------------------------------------------------------------------------

static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

/// The process table. Indexed by task slot (matching Task array in task.rs).
pub static mut PROCESSES: [Option<Process>; 8] = [None, None, None, None, None, None, None, None];
static mut OPEN_FILES: [OpenFile; MAX_OPEN_FILES] = [OpenFile::empty(); MAX_OPEN_FILES];
static mut PIPES: [Pipe; MAX_PIPES] = [Pipe::empty(); MAX_PIPES];

fn default_fds() -> [FdTarget; MAX_FDS] {
    let mut fds = [FdTarget::Empty; MAX_FDS];
    fds[0] = FdTarget::Stdio(0);
    fds[1] = FdTarget::Stdio(1);
    fds[2] = FdTarget::Stdio(2);
    fds
}

fn default_fd_flags() -> [u32; MAX_FDS] {
    [0; MAX_FDS]
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Spawn a new process entry in the given task slot.
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
        vmas: [VmRegion::empty(); MAX_VMAS],
    });
    crate::dbg_log!("PROCESS", "spawned pid={} at slot={}", pid, task_slot);
    pid
}

pub unsafe fn fork_current(task_slot: usize, pml4: usize) -> usize {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    let parent = current_process().expect("fork_current without current process");
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
        state: ProcessState::Running,
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

fn overlaps(a_start: usize, a_len: usize, b_start: usize, b_len: usize) -> bool {
    let a_end = a_start.saturating_add(a_len);
    let b_end = b_start.saturating_add(b_len);
    a_start < b_end && b_start < a_end
}

pub unsafe fn region_conflicts(start: usize, len: usize) -> bool {
    let Some(p) = current_process() else {
        return true;
    };
    p.vmas
        .iter()
        .any(|v| v.in_use && overlaps(start, len, v.start, v.len))
}

pub unsafe fn alloc_vma(start: usize, len: usize, prot: u32, flags: u32) -> bool {
    let Some(p) = current_process_mut() else {
        return false;
    };
    for vma in &mut p.vmas {
        if !vma.in_use {
            *vma = VmRegion {
                start,
                len,
                prot,
                flags,
                in_use: true,
            };
            return true;
        }
    }
    false
}

pub unsafe fn find_vma_exact_mut(start: usize, len: usize) -> Option<&'static mut VmRegion> {
    let p = current_process_mut()?;
    p.vmas.iter_mut().find(|v| v.in_use && v.start == start && v.len == len)
}

pub unsafe fn reserve_mmap_base(len: usize) -> Option<usize> {
    let p = current_process_mut()?;
    let start = (p.next_mmap_base + 0xFFF) & !0xFFF;
    let end = start.checked_add(len)?;
    p.next_mmap_base = end;
    Some(start)
}

/// Called by syscall 60 (exit) to mark the current process as a zombie.
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
            release_fds(&p.fds);
            PROCESSES[slot] = None;
            crate::dbg_log!("PROCESS", "reaped slot={}, exit_code={}", slot, code);
            return Some(code);
        }
    }
    None
}

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

unsafe fn release_fds(fds: &[FdTarget; MAX_FDS]) {
    for &target in fds.iter() {
        release_fd_target(target);
    }
}

/// Allocate a new fd for the current process.
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
        buf: [0; PIPE_CAPACITY],
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
