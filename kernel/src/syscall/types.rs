pub const TCGETS: u64 = 0x5401;
pub const TIOCGWINSZ: u64 = 0x5413;

#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_mtime: i64,
    pub st_ctime: i64,
}

#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[repr(C)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; 32],
    pub c_ispeed: u32,
    pub c_ospeed: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Iovec {
    pub iov_base: u64,
    pub iov_len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SigSet {
    pub bits: [u64; 16],
}

impl SigSet {
    pub const fn empty() -> Self {
        Self { bits: [0; 16] }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SigAction {
    pub handler: u64,
    pub flags: u64,
    pub restorer: u64,
    pub mask: SigSet,
}

impl SigAction {
    pub const fn empty() -> Self {
        Self {
            handler: 0,
            flags: 0,
            restorer: 0,
            mask: SigSet::empty(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StackT {
    pub ss_sp: u64,
    pub ss_flags: i32,
    pub ss_size: usize,
}

impl StackT {
    pub const fn disabled() -> Self {
        Self {
            ss_sp: 0,
            ss_flags: 2,
            ss_size: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SysInfo {
    pub uptime: i64,
    pub loads: [u64; 3],
    pub totalram: u64,
    pub freeram: u64,
    pub sharedram: u64,
    pub bufferram: u64,
    pub totalswap: u64,
    pub freeswap: u64,
    pub procs: u16,
    pub totalhigh: u64,
    pub freehigh: u64,
    pub mem_unit: u32,
    pub _pad: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallFrame {
    pub rbp: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rdx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rcx: u64,
    pub r11: u64,
    pub r10: u64,
}
