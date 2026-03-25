use alloc::vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use crate::syscall::types::{StackT, SysInfo};

/// uname structure
#[repr(C)]
#[derive(Debug)]
pub struct UtsName {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
    pub domainname: [u8; 65],
}

impl UtsName {
    fn new() -> Self {
        let mut uts = Self {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
        };

        // Fill in the fields
        let sysname = b"CoreOS";
        let nodename = b"localhost";
        let release = b"0.1.0";
        let version = b"#1 SMP Sat Mar 21 2026";
        let machine = b"x86_64";
        let domainname = b"(none)";

        uts.sysname[..sysname.len()].copy_from_slice(sysname);
        uts.nodename[..nodename.len()].copy_from_slice(nodename);
        uts.release[..release.len()].copy_from_slice(release);
        uts.version[..version.len()].copy_from_slice(version);
        uts.machine[..machine.len()].copy_from_slice(machine);
        uts.domainname[..domainname.len()].copy_from_slice(domainname);

        uts
    }
}

/// uname(buf) - Get system information
pub fn syscall_uname(buf: u64) -> u64 {
    result::ret(syscall_uname_impl(buf))
}

fn syscall_uname_impl(buf: u64) -> SysResult {
    result::ensure(buf != 0, SysError::Fault)?;
    let uts = UtsName::new();
    result::ensure(unsafe { helpers::copy_struct_to_user(buf, &uts) }, SysError::Fault)?;
    result::ok(0u64)
}

/// rlimit structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RLimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

const RLIM_INFINITY: u64 = !0;

// Resource limit types
const RLIMIT_NOFILE: i32 = 7; // Max number of open files
const RLIMIT_STACK: i32 = 3; // Stack size
const RLIMIT_AS: i32 = 9; // Address space
const RLIMIT_DATA: i32 = 2; // Data segment
const RLIMIT_CORE: i32 = 4; // Core file size
const ROOT_DIR: &[u8] = b"/\0";
const ARCH_SET_FS: u64 = 0x1002;
const ARCH_GET_FS: u64 = 0x1003;
const USER_TOP: u64 = 0x0000_8000_0000_0000;
const GRND_NONBLOCK: u64 = 0x0001;
const GRND_RANDOM: u64 = 0x0002;

static RNG_STATE: AtomicU64 = AtomicU64::new(0);

/// getrlimit(resource, rlim) - Get resource limits
fn syscall_getrlimit_impl(resource: i32, rlim: u64) -> SysResult {
    result::ensure(rlim != 0, SysError::Fault)?;
    // For now, return unlimited for everything
    let limit = match resource {
        RLIMIT_NOFILE => RLimit {
            rlim_cur: 1024,
            rlim_max: 4096,
        },
        RLIMIT_STACK => RLimit {
            rlim_cur: 8 * 1024 * 1024, // 8MB
            rlim_max: RLIM_INFINITY,
        },
        RLIMIT_AS | RLIMIT_DATA => RLimit {
            rlim_cur: RLIM_INFINITY,
            rlim_max: RLIM_INFINITY,
        },
        RLIMIT_CORE => RLimit {
            rlim_cur: 0,
            rlim_max: RLIM_INFINITY,
        },
        _ => RLimit {
            rlim_cur: RLIM_INFINITY,
            rlim_max: RLIM_INFINITY,
        },
    };
    result::ensure(unsafe { helpers::copy_struct_to_user(rlim, &limit) }, SysError::Fault)?;
    result::ok(0u64)
}

pub fn syscall_getrlimit(resource: i32, rlim: u64) -> u64 {
    result::ret(syscall_getrlimit_impl(resource, rlim))
}

/// setrlimit(resource, rlim) - Set resource limits (stub)
pub fn syscall_setrlimit(_resource: i32, _rlim: u64) -> u64 {
    result::ret(result::ok(0u64))
}

/// prlimit64(pid, resource, new_limit, old_limit) - Get/set resource limits
pub fn syscall_prlimit64(pid: i32, resource: i32, new_limit: u64, old_limit: u64) -> u64 {
    result::ret(syscall_prlimit64_impl(pid, resource, new_limit, old_limit))
}

fn syscall_prlimit64_impl(pid: i32, resource: i32, new_limit: u64, old_limit: u64) -> SysResult {
    result::ensure(
        pid == 0 || pid as usize == unsafe { crate::proc::current_pid() },
        SysError::NoEntry,
    )?;
    // Get old limit if requested
    if old_limit != 0 {
        syscall_getrlimit_impl(resource, old_limit)?;
    }

    // Set new limit if provided (stub - ignore it)
    if new_limit != 0 {
        // Would validate and set here, but we're stubbing
    }

    result::ok(0u64)
}

pub fn syscall_getcwd(buf: u64, size: u64) -> u64 {
    result::ret(syscall_getcwd_impl(buf, size))
}

fn syscall_getcwd_impl(buf: u64, size: u64) -> SysResult {
    result::ensure(buf != 0 && size >= ROOT_DIR.len() as u64, SysError::Fault)?;
    result::ensure(
        unsafe { crate::usercopy::copy_to_user(buf, ROOT_DIR) }.is_ok(),
        SysError::Fault,
    )?;
    result::ok(buf)
}

pub fn syscall_chdir(path_ptr: u64, path_len: u64) -> u64 {
    result::ret(syscall_chdir_impl(path_ptr, path_len))
}

fn syscall_chdir_impl(path_ptr: u64, path_len: u64) -> SysResult {
    if path_len == 1 {
        let mut raw = [0u8; 1];
        result::ensure(
            unsafe { crate::usercopy::copy_from_user(&mut raw, path_ptr) }.is_ok(),
            SysError::Fault,
        )?;
        result::ensure(matches!(raw[0], b'/' | b'.'), SysError::NoEntry)?;
        return result::ok(0u64);
    }

    let (name_buf, name_len) = result::option(
        unsafe { helpers::copy_path_from_user(path_ptr, path_len) },
        SysError::Fault,
    )?;
    result::ensure(name_len == 1 && name_buf[0] == '.', SysError::NoEntry)?;
    result::ok(0u64)
}

pub fn syscall_sigaltstack(ss_ptr: u64, old_ss_ptr: u64) -> u64 {
    result::ret(syscall_sigaltstack_impl(ss_ptr, old_ss_ptr))
}

fn syscall_sigaltstack_impl(_ss_ptr: u64, old_ss_ptr: u64) -> SysResult {
    if old_ss_ptr != 0 {
        let disabled = StackT {
            ss_sp: 0,
            ss_flags: 2,
            ss_size: 0,
        };
        result::ensure(
            unsafe { helpers::copy_struct_to_user(old_ss_ptr, &disabled) },
            SysError::Fault,
        )?;
    }
    result::ok(0u64)
}

pub fn syscall_sysinfo(info_ptr: u64) -> u64 {
    result::ret(syscall_sysinfo_impl(info_ptr))
}

fn syscall_sysinfo_impl(info_ptr: u64) -> SysResult {
    let info = SysInfo {
        uptime: crate::hw::pit::uptime_seconds() as i64,
        loads: [0; 3],
        totalram: crate::mem::pmm::total_bytes() as u64,
        freeram: crate::mem::pmm::free_bytes() as u64,
        sharedram: 0,
        bufferram: 0,
        totalswap: 0,
        freeswap: 0,
        procs: unsafe { crate::proc::active_process_count() as u16 },
        totalhigh: 0,
        freehigh: 0,
        mem_unit: 1,
        _pad: [0; 8],
    };
    result::ensure(
        unsafe { helpers::copy_struct_to_user(info_ptr, &info) },
        SysError::Fault,
    )?;
    result::ok(0u64)
}

pub fn syscall_arch_prctl(code: u64, addr: u64) -> u64 {
    result::ret(syscall_arch_prctl_impl(code, addr))
}

fn syscall_arch_prctl_impl(code: u64, addr: u64) -> SysResult {
    match code {
        ARCH_SET_FS => {
            result::ensure(addr < USER_TOP, SysError::Invalid)?;
            let process = result::option(unsafe { crate::proc::current_process_mut() }, SysError::Invalid)?;
            process.fs_base = addr;
            unsafe { crate::arch::amd64::cpu::write_fs_base(addr) };
            result::ok(0u64)
        }
        ARCH_GET_FS => {
            result::ensure(addr != 0, SysError::Fault)?;
            let fs_base = unsafe { crate::proc::current_fs_base() };
            result::ensure(unsafe { helpers::copy_struct_to_user(addr, &fs_base) }, SysError::Fault)?;
            result::ok(0u64)
        }
        _ => result::err(SysError::Unsupported),
    }
}

fn next_random_u64() -> u64 {
    let mut state = RNG_STATE.load(Ordering::Relaxed);
    if state == 0 {
        state = crate::hw::pit::ticks()
            ^ ((unsafe { crate::proc::current_pid() } as u64) << 32)
            ^ 0x9E37_79B9_7F4A_7C15;
    }
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    RNG_STATE.store(state, Ordering::Relaxed);
    state
}

pub fn syscall_getrandom(buf_ptr: u64, len: u64, flags: u64) -> u64 {
    result::ret(syscall_getrandom_impl(buf_ptr, len, flags))
}

fn syscall_getrandom_impl(buf_ptr: u64, len: u64, flags: u64) -> SysResult {
    result::ensure(flags & !(GRND_NONBLOCK | GRND_RANDOM) == 0, SysError::Invalid)?;
    let len = len as usize;
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, len), SysError::Fault)?;

    let mut bytes = vec![0u8; len];
    let mut remaining = 0u64;
    for byte in bytes.iter_mut() {
        if remaining == 0 {
            remaining = next_random_u64();
        }
        *byte = remaining as u8;
        remaining >>= 8;
    }

    result::ensure(unsafe { crate::usercopy::copy_to_user(buf_ptr, &bytes) }.is_ok(), SysError::Fault)?;
    result::ok(len as u64)
}
