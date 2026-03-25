use crate::proc;
use crate::proc::DescriptorInfo;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use crate::syscall::types::Stat;

const S_IFREG: u32 = 0o100000;
const S_IFCHR: u32 = 0o020000;
const S_IFIFO: u32 = 0o010000;
const DEFAULT_FILE_MODE: u32 = S_IFREG | 0o644;
const DEFAULT_CHAR_MODE: u32 = S_IFCHR | 0o666;
const DEFAULT_PIPE_MODE: u32 = S_IFIFO | 0o666;
const AT_FDCWD: i64 = -100;
const AT_REMOVEDIR: u64 = 0x200;
const PROC_SELF_EXE: &[u8] = b"/proc/self/exe";
const F_OK: u64 = 0;
const X_OK: u64 = 1;
const W_OK: u64 = 2;
const R_OK: u64 = 4;

fn stat_for_file_idx(file_idx: usize, size: i64) -> Stat {
    Stat {
        st_dev: 1,
        st_ino: (file_idx + 1) as u64,
        st_mode: DEFAULT_FILE_MODE,
        st_nlink: 1,
        st_uid: 0,
        st_gid: 0,
        st_rdev: 0,
        st_size: size,
        st_blksize: 4096,
        st_blocks: ((size as u64) + 511).div_ceil(512) as i64,
        st_atime: 0,
        st_mtime: 0,
        st_ctime: 0,
    }
}

unsafe fn copy_file_payload_args(args_ptr: u64) -> Option<(u64, usize)> {
    if !crate::usercopy::user_range_ok(args_ptr, 16) {
        return None;
    }
    let data_ptr = (args_ptr as *const u64).read();
    let data_len = (args_ptr as *const u64).add(1).read() as usize;
    if !crate::usercopy::user_range_ok(data_ptr, data_len) {
        return None;
    }
    Some((data_ptr, data_len))
}

fn path_is_root(path_ptr: u64, path_len: u64) -> bool {
    if path_len != 1 {
        return false;
    }
    let mut raw = [0u8; 1];
    unsafe { crate::usercopy::copy_from_user(&mut raw, path_ptr) }.is_ok() && raw[0] == b'/'
}

fn build_proc_self_exe_target() -> ([u8; helpers::MAX_PATH_LEN], usize) {
    let (exe_path, exe_len) = unsafe { crate::proc::current_exe_path() };
    let mut out = [0u8; helpers::MAX_PATH_LEN];
    if exe_len == 0 || exe_len + 1 > out.len() {
        return (out, 0);
    }
    out[0] = b'/';
    out[1..1 + exe_len].copy_from_slice(&exe_path[..exe_len]);
    (out, exe_len + 1)
}

unsafe fn readlink_common(path: &[u8], buf_ptr: u64, buf_len: u64) -> SysResult {
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, buf_len as usize), SysError::Fault)?;
    result::ensure(path == PROC_SELF_EXE, SysError::NoEntry)?;

    let (target, target_len) = build_proc_self_exe_target();
    result::ensure(target_len > 0, SysError::NoEntry)?;
    let count = core::cmp::min(target_len, buf_len as usize);
    result::ensure(crate::usercopy::copy_to_user(buf_ptr, &target[..count]).is_ok(), SysError::Fault)?;
    result::ok(count as u64)
}

pub unsafe fn open(path_ptr: u64, path_len: u64) -> u64 {
    result::ret(open_impl(path_ptr, path_len))
}

unsafe fn open_impl(path_ptr: u64, path_len: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(path_ptr, path_len), SysError::Fault)?;
    let file_idx = result::option(super::fs::fs_find_idx(&name_buf[..name_len]), SysError::NoEntry)?;
    result::ok(
        result::option(proc::open_file(file_idx), SysError::BadFd)? as u64,
    )
}

pub unsafe fn openat(dirfd: u64, path_ptr: u64, path_len: u64, _flags: u64) -> u64 {
    result::ret(openat_impl(dirfd, path_ptr, path_len))
}

unsafe fn openat_impl(dirfd: u64, path_ptr: u64, path_len: u64) -> SysResult {
    result::ensure(dirfd as i64 == AT_FDCWD, SysError::Unsupported)?;
    open_impl(path_ptr, path_len)
}

pub unsafe fn fsize(fd: u64) -> u64 {
    result::ret(fsize_impl(fd))
}

unsafe fn fsize_impl(fd: u64) -> SysResult {
    result::ok(result::option(proc::file_size(fd as usize), SysError::BadFd)? as u64)
}

pub unsafe fn fstat(fd: u64, stat_ptr: u64) -> u64 {
    result::ret(fstat_impl(fd, stat_ptr))
}

unsafe fn fstat_impl(fd: u64, stat_ptr: u64) -> SysResult {
    let stat = match result::option(proc::descriptor_info(fd as usize), SysError::BadFd)? {
        DescriptorInfo::Stdio { index } => Stat {
            st_dev: 0,
            st_ino: index as u64,
            st_mode: DEFAULT_CHAR_MODE,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 1,
            st_blocks: 0,
            st_atime: 0,
            st_mtime: 0,
            st_ctime: 0,
        },
        DescriptorInfo::File { file_idx, size } => stat_for_file_idx(file_idx, size as i64),
        DescriptorInfo::Pipe => Stat {
            st_dev: 2,
            st_ino: fd,
            st_mode: DEFAULT_PIPE_MODE,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 1024,
            st_blocks: 0,
            st_atime: 0,
            st_mtime: 0,
            st_ctime: 0,
        },
    };

    result::ensure(helpers::copy_struct_to_user(stat_ptr, &stat), SysError::Fault)?;
    result::ok(0u64)
}

pub unsafe fn fstatat(dirfd: u64, path_ptr: u64, path_len: u64, stat_ptr: u64) -> u64 {
    result::ret(fstatat_impl(dirfd, path_ptr, path_len, stat_ptr))
}

unsafe fn fstatat_impl(dirfd: u64, path_ptr: u64, path_len: u64, stat_ptr: u64) -> SysResult {
    result::ensure(dirfd as i64 == AT_FDCWD, SysError::Unsupported)?;
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(path_ptr, path_len), SysError::Fault)?;
    let file_idx = result::option(super::fs::fs_find_idx(&name_buf[..name_len]), SysError::NoEntry)?;
    let stat = stat_for_file_idx(file_idx, super::fs::fs_file_size(file_idx) as i64);
    result::ensure(helpers::copy_struct_to_user(stat_ptr, &stat), SysError::Fault)?;
    result::ok(0u64)
}

pub unsafe fn faccessat(dirfd: u64, path_ptr: u64, path_len: u64, mode: u64) -> u64 {
    result::ret(faccessat_impl(dirfd, path_ptr, path_len, mode))
}

unsafe fn faccessat_impl(dirfd: u64, path_ptr: u64, path_len: u64, mode: u64) -> SysResult {
    result::ensure(dirfd as i64 == AT_FDCWD, SysError::Unsupported)?;
    result::ensure(mode & !(R_OK | W_OK | X_OK) == 0 || mode == F_OK, SysError::Invalid)?;

    if path_is_root(path_ptr, path_len) {
        result::ensure(mode & W_OK == 0, SysError::Unsupported)?;
        return result::ok(0u64);
    }

    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(path_ptr, path_len), SysError::Fault)?;
    let _file_idx =
        result::option(super::fs::fs_find_idx(&name_buf[..name_len]), SysError::NoEntry)?;

    result::ensure(mode & X_OK == 0, SysError::Unsupported)?;
    result::ok(0u64)
}

pub unsafe fn lseek(fd: u64, offset: u64, whence: u64) -> u64 {
    result::ret(lseek_impl(fd, offset, whence))
}

unsafe fn lseek_impl(fd: u64, offset: u64, whence: u64) -> SysResult {
    result::ok(result::option(proc::seek(fd as usize, offset as i64, whence), SysError::BadFd)?)
}

pub unsafe fn truncate(path_ptr: u64, path_len: u64) -> u64 {
    result::ret(truncate_impl(path_ptr, path_len))
}

unsafe fn truncate_impl(path_ptr: u64, path_len: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(path_ptr, path_len), SysError::Fault)?;
    let file_idx =
        result::option(super::fs::fs_find_idx(&name_buf[..name_len]), SysError::NoEntry)?;
    result::ensure(super::fs::fs_resize(file_idx, 0), SysError::Invalid)?;
    result::ok(0u64)
}

pub unsafe fn ftruncate(fd: u64, len: u64) -> u64 {
    result::ret(ftruncate_impl(fd, len))
}

unsafe fn ftruncate_impl(fd: u64, len: u64) -> SysResult {
    let DescriptorInfo::File { file_idx, .. } =
        result::option(proc::descriptor_info(fd as usize), SysError::BadFd)?
    else {
        return result::err(SysError::Unsupported);
    };
    result::ensure(super::fs::fs_resize(file_idx, len as usize), SysError::Invalid)?;
    result::ok(0u64)
}

pub unsafe fn ls(buf_ptr: u64, buf_len: u64) -> u64 {
    result::ret(ls_impl(buf_ptr, buf_len))
}

unsafe fn ls_impl(buf_ptr: u64, buf_len: u64) -> SysResult {
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, buf_len as usize), SysError::Fault)?;
    let files = crate::vfs::list().unwrap_or_default();
    let buf = buf_ptr as *mut u8;
    let buf_len = buf_len as usize;
    let mut pos = 0usize;
    for file in files.iter() {
        for &ch in file.name.iter() {
            if pos + 1 >= buf_len {
                break;
            }
            buf.add(pos).write(ch as u8);
            pos += 1;
        }

        let size = file.data.len();
        for &b in b" (" {
            if pos + 1 >= buf_len {
                break;
            }
            buf.add(pos).write(b);
            pos += 1;
        }

        let mut digits = [0u8; 20];
        let mut digit_count = 0usize;
        let mut value = size;
        if value == 0 {
            digits[0] = b'0';
            digit_count = 1;
        } else {
            while value > 0 && digit_count < digits.len() {
                digits[digit_count] = b'0' + (value % 10) as u8;
                digit_count += 1;
                value /= 10;
            }
        }
        for i in (0..digit_count).rev() {
            if pos + 1 >= buf_len {
                break;
            }
            buf.add(pos).write(digits[i]);
            pos += 1;
        }

        for &b in b" bytes)\n" {
            if pos + 1 >= buf_len {
                break;
            }
            buf.add(pos).write(b);
            pos += 1;
        }
    }
    if pos < buf_len {
        buf.add(pos).write(0);
    }
    result::ok(pos as u64)
}

pub unsafe fn touch(name_ptr: u64, name_len: u64) -> u64 {
    result::ret(touch_impl(name_ptr, name_len))
}

unsafe fn touch_impl(name_ptr: u64, name_len: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(name_ptr, name_len), SysError::Fault)?;
    result::ensure(crate::vfs::create(&name_buf[..name_len]), SysError::Invalid)?;
    result::ok(0u64)
}

pub unsafe fn rm(name_ptr: u64, name_len: u64) -> u64 {
    result::ret(rm_impl(name_ptr, name_len))
}

unsafe fn rm_impl(name_ptr: u64, name_len: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(name_ptr, name_len), SysError::Fault)?;
    result::ensure(crate::vfs::remove(&name_buf[..name_len]), SysError::NoEntry)?;
    result::ok(0u64)
}

pub unsafe fn unlinkat(dirfd: u64, path_ptr: u64, path_len: u64, flags: u64) -> u64 {
    result::ret(unlinkat_impl(dirfd, path_ptr, path_len, flags))
}

unsafe fn unlinkat_impl(dirfd: u64, path_ptr: u64, path_len: u64, flags: u64) -> SysResult {
    result::ensure(dirfd as i64 == AT_FDCWD, SysError::Unsupported)?;
    result::ensure(flags == 0 || flags == AT_REMOVEDIR, SysError::Invalid)?;
    result::ensure(flags & AT_REMOVEDIR == 0, SysError::Unsupported)?;
    rm_impl(path_ptr, path_len)
}

pub unsafe fn readlink(path_ptr: u64, buf_ptr: u64, buf_len: u64) -> u64 {
    result::ret(readlink_impl(path_ptr, buf_ptr, buf_len))
}

unsafe fn readlink_impl(path_ptr: u64, buf_ptr: u64, buf_len: u64) -> SysResult {
    let (path, path_len) = result::option(helpers::copy_cstr_from_user(path_ptr), SysError::Fault)?;
    readlink_common(&path[..path_len], buf_ptr, buf_len)
}

pub unsafe fn readlinkat(dirfd: u64, path_ptr: u64, buf_ptr: u64, buf_len: u64) -> u64 {
    result::ret(readlinkat_impl(dirfd, path_ptr, buf_ptr, buf_len))
}

unsafe fn readlinkat_impl(dirfd: u64, path_ptr: u64, buf_ptr: u64, buf_len: u64) -> SysResult {
    result::ensure(dirfd as i64 == AT_FDCWD, SysError::Unsupported)?;
    let (path, path_len) = result::option(helpers::copy_cstr_from_user(path_ptr), SysError::Fault)?;
    readlink_common(&path[..path_len], buf_ptr, buf_len)
}

pub unsafe fn write_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    result::ret(write_file_impl(name_ptr, name_len, args_ptr))
}

unsafe fn write_file_impl(name_ptr: u64, name_len: u64, args_ptr: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(name_ptr, name_len), SysError::Fault)?;
    let (data_ptr, data_len) = result::option(copy_file_payload_args(args_ptr), SysError::Fault)?;

    let src = core::slice::from_raw_parts(data_ptr as *const u8, data_len);
    result::ensure(crate::vfs::replace_all(&name_buf[..name_len], src), SysError::NoEntry)?;
    result::ok(0u64)
}

pub unsafe fn push_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    result::ret(push_file_impl(name_ptr, name_len, args_ptr))
}

unsafe fn push_file_impl(name_ptr: u64, name_len: u64, args_ptr: u64) -> SysResult {
    let (name_buf, name_len) =
        result::option(helpers::copy_path_from_user(name_ptr, name_len), SysError::Fault)?;
    let (data_ptr, data_len) = result::option(copy_file_payload_args(args_ptr), SysError::Fault)?;

    let src = core::slice::from_raw_parts(data_ptr as *const u8, data_len);
    result::ensure(crate::vfs::append_all(&name_buf[..name_len], src), SysError::NoEntry)?;
    result::ok(0u64)
}
