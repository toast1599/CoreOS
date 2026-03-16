pub mod elf;
pub mod exec;
mod fd;
mod process;
pub mod scheduler;
pub mod task;
mod vm;

use core::sync::atomic::AtomicUsize;

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

pub use fd::{
    alloc_fd, alloc_pipe, close_fd, dup_fd, fork_current, get_fd, get_fd_flags, get_fd_mut,
    get_fd_target, get_pipe_mut, get_status_flags, pipe_peer_closed, reap_slot, set_fd_flags,
    set_status_flags,
};
pub use process::{
    current_brk, current_pid, current_process, current_process_mut, exit, is_running_in_slot,
    set_brk, spawn,
};
pub use vm::{alloc_vma, find_vma_exact_mut, region_conflicts, reserve_mmap_base};
