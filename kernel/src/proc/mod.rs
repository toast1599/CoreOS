//! Process subsystem.
//!
//! The `proc` tree owns three related concerns:
//! - lifecycle and lookup for processes and threads
//! - scheduler task integration
//! - per-process resources such as file descriptors and VM regions
//!
//! This module is intentionally a thin facade. Concrete state lives in
//! [`state`], while the child modules implement specific behaviors.

pub mod elf;
pub mod exec;
mod fd;
pub mod fd_io;
mod process;
pub mod scheduler;
pub mod state;
pub mod task;
mod vm;

pub use fd::{
    close_descriptor, create_pipe_pair, descriptor_info, dup_exact, dup_min, file_size,
    fork_current, get_fd_flags, get_fd_target, get_status_flags, is_stdin, is_stdout_or_stderr,
    open_char_device, open_file, read_file, read_pipe, reap_slot, seek, set_cloexec,
    set_status_flags, with_fd_mut, write_file, write_pipe,
};
pub use process::{
    active_process_count, current_brk, current_exe_path, current_fs_base, current_pid,
    current_ppid, current_process, current_process_mut, current_thread, current_thread_mut,
    current_tid, exit, exit_thread, find_slot_by_pid, find_thread_slot_by_tid, is_running_in_slot,
    set_brk, spawn_named, spawn_thread_in_group, task_slot_reaped,
};
pub use state::{
    DescriptorInfo, FdTarget, OpenFile, Pipe, Process, ProcessState, Thread, ThreadState, VmRegion,
    EXE_PATH_MAX, FD_CLOEXEC, MAX_FDS, MAX_OPEN_FILES, MAX_PIPES, NEXT_PID, O_ACCMODE, O_APPEND,
    O_CREAT, O_EXCL, O_NONBLOCK, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY, PROCESSES, THREADS,
};
pub use vm::{alloc_vma, find_vma_exact_mut, region_conflicts, reserve_mmap_base};
