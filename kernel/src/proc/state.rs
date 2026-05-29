//! Core process-subsystem state.
//!
//! This module owns the process/thread data model plus the global tables that
//! back scheduling, file descriptor management, and virtual memory bookkeeping.

use crate::syscall::types::{SigAction, SigSet, StackT};
use core::sync::atomic::AtomicUsize;
use spin::Mutex;

/// Max file descriptors per process, including stdin/stdout/stderr.
pub const MAX_FDS: usize = 16;
pub const MAX_OPEN_FILES: usize = 32;
pub const MAX_PIPES: usize = 16;
pub const MAX_VMAS: usize = 32;
pub const EXE_PATH_MAX: usize = 64;
pub const MMAP_BASE: usize = 0x0000_0001_0000_0000;
pub const FD_CLOEXEC: u32 = 1;
pub const O_ACCMODE: u32 = 0o3;
pub const O_RDONLY: u32 = 0o0;
pub const O_WRONLY: u32 = 0o1;
pub const O_RDWR: u32 = 0o2;
pub const O_CREAT: u32 = 0o100;
pub const O_EXCL: u32 = 0o200;
pub const O_TRUNC: u32 = 0o1000;
pub const O_APPEND: u32 = 0o2000;
pub const O_NONBLOCK: u32 = 0o4000;
pub(super) const PIPE_CAPACITY: usize = 1024;

/// A shared open file description pointing into RamFS.
#[derive(Clone, Copy)]
pub struct OpenFile {
    /// Index into `RamFS.files`.
    pub file_idx: usize,
    /// Read/write cursor in bytes.
    pub offset: usize,
    /// Open-file status flags shared by duplicated descriptors.
    pub status_flags: u32,
    /// Number of file descriptors that reference this open-file description.
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

    pub const fn new(file_idx: usize, status_flags: u32) -> Self {
        Self {
            file_idx,
            offset: 0,
            status_flags,
            refs: 1,
            in_use: true,
        }
    }
}

#[derive(Clone, Copy)]
pub enum FdTarget {
    Empty,
    Stdio(u8),
    Tty,
    Null,
    Zero,
    Open(usize),
    PipeRead(usize),
    PipeWrite(usize),
}

#[derive(Clone, Copy)]
pub enum DescriptorInfo {
    Stdio { index: u8 },
    CharDevice,
    File { file_idx: usize, size: usize },
    Pipe,
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

    pub const fn new() -> Self {
        Self {
            buf: [0; PIPE_CAPACITY],
            read_pos: 0,
            write_pos: 0,
            len: 0,
            read_refs: 1,
            write_refs: 1,
            read_flags: 0,
            write_flags: 0,
            in_use: true,
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

    pub const fn new(start: usize, len: usize, prot: u32, flags: u32) -> Self {
        Self {
            start,
            len,
            prot,
            flags,
            in_use: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ProcessState {
    /// Process is alive and scheduled.
    Running,
    /// Process exited and is waiting to be reaped.
    Zombie,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ThreadState {
    Running,
    Zombie,
}

#[derive(Clone, Copy)]
pub struct Thread {
    pub tid: usize,
    #[allow(dead_code)]
    pub parent_tid: usize,
    pub group_slot: usize,
    pub task_slot: usize,
    pub state: ThreadState,
    pub clear_child_tid: u64,
    pub fs_base: u64,
    pub sig_pending: SigSet,
    pub sig_mask: SigSet,
    pub saved_sig_mask: SigSet,
    pub sig_altstack: StackT,
    pub in_signal_handler: bool,
    pub on_altstack: bool,
    pub robust_list_head: u64,
    pub robust_list_len: usize,
}

impl Thread {
    /// Build the initial thread record for a new process leader.
    pub const fn new_leader(
        tid: usize,
        parent_tid: usize,
        group_slot: usize,
        task_slot: usize,
    ) -> Self {
        Self {
            tid,
            parent_tid,
            group_slot,
            task_slot,
            state: ThreadState::Running,
            clear_child_tid: 0,
            fs_base: 0,
            sig_pending: SigSet::empty(),
            sig_mask: SigSet::empty(),
            saved_sig_mask: SigSet::empty(),
            sig_altstack: StackT::disabled(),
            in_signal_handler: false,
            on_altstack: false,
            robust_list_head: 0,
            robust_list_len: 0,
        }
    }

    /// Build a thread that joins an existing thread group.
    pub const fn new_group_member(
        tid: usize,
        parent_tid: usize,
        group_slot: usize,
        task_slot: usize,
        clear_child_tid: u64,
        fs_base: u64,
        sig_mask: SigSet,
    ) -> Self {
        Self {
            tid,
            parent_tid,
            group_slot,
            task_slot,
            state: ThreadState::Running,
            clear_child_tid,
            fs_base,
            sig_pending: SigSet::empty(),
            sig_mask,
            saved_sig_mask: SigSet::empty(),
            sig_altstack: StackT::disabled(),
            in_signal_handler: false,
            on_altstack: false,
            robust_list_head: 0,
            robust_list_len: 0,
        }
    }
}

pub struct Process {
    pub pid: usize,
    pub parent_pid: usize,
    pub pgid: usize,
    pub sid: usize,
    pub state: ProcessState,
    pub thread_count: usize,
    pub exit_code: i64,
    pub uid: u32,
    pub euid: u32,
    pub gid: u32,
    pub egid: u32,
    pub umask: u32,
    pub exe_path: [u8; EXE_PATH_MAX],
    pub exe_path_len: usize,
    pub program_break: usize,
    pub next_mmap_base: usize,
    pub sig_handlers: [SigAction; 65],
    pub sig_pending: SigSet,
    /// Per-process file descriptor table. Descriptors `0..=2` start as stdio.
    pub fds: [FdTarget; MAX_FDS],
    pub fd_flags: [u32; MAX_FDS],
    pub vmas: [VmRegion; MAX_VMAS],
}

impl Process {
    /// Build the first process record for a newly spawned thread group.
    pub fn new_leader(pid: usize, parent_pid: usize, name: &[char]) -> Self {
        let mut exe_path = [0u8; EXE_PATH_MAX];
        let exe_path_len = name.len().min(EXE_PATH_MAX);
        for (idx, ch) in name.iter().take(exe_path_len).enumerate() {
            exe_path[idx] = *ch as u8;
        }

        Self {
            pid,
            parent_pid,
            pgid: pid,
            sid: pid,
            state: ProcessState::Running,
            thread_count: 1,
            exit_code: 0,
            uid: 0,
            euid: 0,
            gid: 0,
            egid: 0,
            umask: 0o022,
            exe_path,
            exe_path_len,
            program_break: 0x4000_0000,
            next_mmap_base: MMAP_BASE,
            sig_handlers: [SigAction::empty(); 65],
            sig_pending: SigSet::empty(),
            fds: default_fds(),
            fd_flags: default_fd_flags(),
            vmas: [VmRegion::empty(); MAX_VMAS],
        }
    }

    /// Clone the process-visible state needed by `fork`.
    pub fn fork_from(parent: &Self, pid: usize) -> Self {
        Self {
            pid,
            parent_pid: parent.pid,
            pgid: parent.pgid,
            sid: parent.sid,
            state: ProcessState::Running,
            thread_count: 1,
            exit_code: 0,
            uid: parent.uid,
            euid: parent.euid,
            gid: parent.gid,
            egid: parent.egid,
            umask: parent.umask,
            exe_path: parent.exe_path,
            exe_path_len: parent.exe_path_len,
            program_break: parent.program_break,
            next_mmap_base: parent.next_mmap_base,
            sig_handlers: parent.sig_handlers,
            sig_pending: SigSet::empty(),
            fds: parent.fds,
            fd_flags: parent.fd_flags,
            vmas: parent.vmas,
        }
    }
}

/// Monotonic identifier allocator shared by processes and threads.
pub static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

/// Process records indexed by thread-group leader task slot.
///
/// Non-leader threads point back to this slot through `Thread::group_slot`.
pub static mut PROCESSES: [Option<Process>; 8] = [None, None, None, None, None, None, None, None];

/// Thread records indexed by scheduler task slot.
pub static mut THREADS: [Option<Thread>; 8] = [None, None, None, None, None, None, None, None];

/// Shared open-file descriptions. File descriptors store indices into this table.
pub(super) static OPEN_FILES: Mutex<[OpenFile; MAX_OPEN_FILES]> =
    Mutex::new([OpenFile::empty(); MAX_OPEN_FILES]);

/// Pipe ring buffers. Pipe file descriptors store indices into this table.
pub(super) static mut PIPES: [Pipe; MAX_PIPES] = [Pipe::empty(); MAX_PIPES];

const fn default_fds() -> [FdTarget; MAX_FDS] {
    let mut fds = [FdTarget::Empty; MAX_FDS];
    fds[0] = FdTarget::Stdio(0);
    fds[1] = FdTarget::Stdio(1);
    fds[2] = FdTarget::Stdio(2);
    fds
}

const fn default_fd_flags() -> [u32; MAX_FDS] {
    [0; MAX_FDS]
}
