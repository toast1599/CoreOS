use crate::drivers::vga;
use crate::proc::fd_io::{read_from_fd, write_to_fd};
use crate::proc::DescriptorInfo;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use crate::syscall::types::Termios;
use crate::syscall::types::WinSize;
use crate::syscall::types::{TCGETS, TIOCGWINSZ};

const O_ACCMODE: u32 = crate::proc::O_ACCMODE;
const O_RDONLY: u32 = crate::proc::O_RDONLY;
const O_WRONLY: u32 = crate::proc::O_WRONLY;

// ---------------------------------------------------------------------------
// syscall implementations
// ---------------------------------------------------------------------------

pub unsafe fn write(_fd: u64, buf_ptr: u64, count: u64) -> u64 {
    result::ret(write_impl(_fd, buf_ptr, count))
}

unsafe fn write_impl(fd: u64, buf_ptr: u64, count: u64) -> SysResult {
    let len = count as usize;
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, len), SysError::Fault)?;
    result::ok(
        result::option(write_to_fd(fd as usize, buf_ptr as *const u8, len), SysError::BadFd)?
            as u64,
    )
}

pub unsafe fn read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    result::ret(read_impl(fd, buf_ptr, count))
}

unsafe fn read_impl(fd: u64, buf_ptr: u64, count: u64) -> SysResult {
    let count = count as usize;
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, count), SysError::Fault)?;
    result::ok(
        result::option(read_from_fd(fd as usize, buf_ptr as *mut u8, count), SysError::BadFd)?
            as u64,
    )
}

pub unsafe fn writev(fd: u64, iov_ptr: u64, iovcnt: u64) -> u64 {
    result::ret(writev_impl(fd, iov_ptr, iovcnt))
}

unsafe fn writev_impl(fd: u64, iov_ptr: u64, iovcnt: u64) -> SysResult {
    let (iovs, count) =
        result::option(helpers::copy_iovecs_from_user(iov_ptr, iovcnt), SysError::Fault)?;

    let mut total = 0usize;
    for iov in &iovs[..count] {
        let len = iov.iov_len as usize;
        result::ensure(crate::usercopy::user_range_ok(iov.iov_base, len), SysError::Fault)?;
        match write_to_fd(fd as usize, iov.iov_base as *const u8, len) {
            Some(written) => {
                total = total.saturating_add(written);
                if written != len {
                    break;
                }
            }
            None => return result::err(SysError::BadFd),
        }
    }
    result::ok(total as u64)
}

pub unsafe fn readv(fd: u64, iov_ptr: u64, iovcnt: u64) -> u64 {
    result::ret(readv_impl(fd, iov_ptr, iovcnt))
}

unsafe fn readv_impl(fd: u64, iov_ptr: u64, iovcnt: u64) -> SysResult {
    let (iovs, count) =
        result::option(helpers::copy_iovecs_from_user(iov_ptr, iovcnt), SysError::Fault)?;

    let mut total = 0usize;
    for iov in &iovs[..count] {
        let len = iov.iov_len as usize;
        result::ensure(crate::usercopy::user_range_ok(iov.iov_base, len), SysError::Fault)?;
        match read_from_fd(fd as usize, iov.iov_base as *mut u8, len) {
            Some(read) => {
                total = total.saturating_add(read);
                if read != len {
                    break;
                }
            }
            None => return result::err(SysError::BadFd),
        }
    }
    result::ok(total as u64)
}

pub unsafe fn ioctl(fd: u64, req: u64, argp: u64) -> u64 {
    result::ret(ioctl_impl(fd, req, argp))
}

unsafe fn ioctl_impl(fd: u64, req: u64, argp: u64) -> SysResult {
    result::ensure(
        matches!(
            crate::proc::descriptor_info(fd as usize),
            Some(DescriptorInfo::Stdio { .. })
        ),
        SysError::NotTty,
    )?;

    match req {
        TCGETS => {
            result::ensure(
                crate::usercopy::user_range_ok(argp, core::mem::size_of::<Termios>()),
                SysError::Fault,
            )?;
            let termios = Termios {
                c_iflag: 0,
                c_oflag: 0,
                c_cflag: 0,
                c_lflag: 0,
                c_line: 0,
                c_cc: [0; 32],
                c_ispeed: 0,
                c_ospeed: 0,
            };
            result::ensure(
                helpers::copy_struct_to_user(argp, &termios),
                SysError::Fault,
            )?;
            result::ok(0u64)
        }
        TIOCGWINSZ => {
            result::ensure(
                crate::usercopy::user_range_ok(argp, core::mem::size_of::<WinSize>()),
                SysError::Fault,
            )?;
            let (rows, cols, xpixel, ypixel) = vga::console::tty_winsize();
            let winsz = WinSize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: xpixel,
                ws_ypixel: ypixel,
            };
            result::ensure(helpers::copy_struct_to_user(argp, &winsz), SysError::Fault)?;
            result::ok(0u64)
        }
        _ => result::err(SysError::NotTty),
    }
}

pub unsafe fn pread64(fd: u64, buf_ptr: u64, count: u64, offset: u64) -> u64 {
    result::ret(pread64_impl(fd, buf_ptr, count, offset))
}

unsafe fn pread64_impl(fd: u64, buf_ptr: u64, count: u64, offset: u64) -> SysResult {
    let len = count as usize;
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, len), SysError::Fault)?;
    let status = result::option(crate::proc::get_status_flags(fd as usize), SysError::BadFd)?;
    result::ensure((status & O_ACCMODE) != O_WRONLY, SysError::BadFd)?;
    let DescriptorInfo::File { file_idx, .. } =
        result::option(crate::proc::descriptor_info(fd as usize), SysError::BadFd)?
    else {
        return result::err(SysError::NotSeekable);
    };

    let bytes = result::option(crate::syscall::fs::fs_read_to_vec(file_idx, offset as usize, len), SysError::BadFd)?;
    result::ensure(crate::usercopy::copy_to_user(buf_ptr, bytes.as_slice()).is_ok(), SysError::Fault)?;
    result::ok(bytes.len() as u64)
}

pub unsafe fn pwrite64(fd: u64, buf_ptr: u64, count: u64, offset: u64) -> u64 {
    result::ret(pwrite64_impl(fd, buf_ptr, count, offset))
}

unsafe fn pwrite64_impl(fd: u64, buf_ptr: u64, count: u64, offset: u64) -> SysResult {
    let len = count as usize;
    result::ensure(crate::usercopy::user_range_ok(buf_ptr, len), SysError::Fault)?;
    let status = result::option(crate::proc::get_status_flags(fd as usize), SysError::BadFd)?;
    result::ensure((status & O_ACCMODE) != O_RDONLY, SysError::BadFd)?;
    let DescriptorInfo::File { file_idx, .. } =
        result::option(crate::proc::descriptor_info(fd as usize), SysError::BadFd)?
    else {
        return result::err(SysError::NotSeekable);
    };

    let mut bytes = alloc::vec![0u8; len];
    result::ensure(crate::usercopy::copy_from_user(&mut bytes, buf_ptr).is_ok(), SysError::Fault)?;
    result::ensure(crate::syscall::fs::fs_write_at(file_idx, offset as usize, bytes.as_slice()), SysError::BadFd)?;
    result::ok(len as u64)
}

pub unsafe fn preadv(fd: u64, iov_ptr: u64, iovcnt: u64, offset: u64) -> u64 {
    result::ret(preadv_impl(fd, iov_ptr, iovcnt, offset))
}

unsafe fn preadv_impl(fd: u64, iov_ptr: u64, iovcnt: u64, offset: u64) -> SysResult {
    let (iovs, count) =
        result::option(helpers::copy_iovecs_from_user(iov_ptr, iovcnt), SysError::Fault)?;
    let mut total = 0usize;
    let mut cur_off = offset;
    for iov in &iovs[..count] {
        let len = iov.iov_len as usize;
        result::ensure(crate::usercopy::user_range_ok(iov.iov_base, len), SysError::Fault)?;
        let bytes = pread64_impl(fd, iov.iov_base, iov.iov_len, cur_off)? as usize;
        total = total.saturating_add(bytes);
        cur_off = cur_off.saturating_add(bytes as u64);
        if bytes != len {
            break;
        }
    }
    result::ok(total as u64)
}

pub unsafe fn pwritev(fd: u64, iov_ptr: u64, iovcnt: u64, offset: u64) -> u64 {
    result::ret(pwritev_impl(fd, iov_ptr, iovcnt, offset))
}

unsafe fn pwritev_impl(fd: u64, iov_ptr: u64, iovcnt: u64, offset: u64) -> SysResult {
    let (iovs, count) =
        result::option(helpers::copy_iovecs_from_user(iov_ptr, iovcnt), SysError::Fault)?;
    let mut total = 0usize;
    let mut cur_off = offset;
    for iov in &iovs[..count] {
        let written = pwrite64_impl(fd, iov.iov_base, iov.iov_len, cur_off)?;
        let written = written as usize;
        total = total.saturating_add(written);
        cur_off = cur_off.saturating_add(written as u64);
        if written != iov.iov_len as usize {
            break;
        }
    }
    result::ok(total as u64)
}

pub unsafe fn sendfile(out_fd: u64, in_fd: u64, offset_ptr: u64, count: u64) -> u64 {
    result::ret(sendfile_impl(out_fd, in_fd, offset_ptr, count))
}

unsafe fn sendfile_impl(out_fd: u64, in_fd: u64, offset_ptr: u64, count: u64) -> SysResult {
    let count = count as usize;
    let status = result::option(crate::proc::get_status_flags(in_fd as usize), SysError::BadFd)?;
    result::ensure((status & O_ACCMODE) != O_WRONLY, SysError::BadFd)?;
    let DescriptorInfo::File { file_idx, .. } =
        result::option(crate::proc::descriptor_info(in_fd as usize), SysError::BadFd)?
    else {
        return result::err(SysError::NotSeekable);
    };

    let start_offset = if offset_ptr != 0 {
        let offset: i64 = result::option(helpers::copy_struct_from_user(offset_ptr), SysError::Fault)?;
        result::ensure(offset >= 0, SysError::Invalid)?;
        offset as usize
    } else {
        let of = result::option(crate::proc::get_fd_mut(in_fd as usize), SysError::BadFd)?;
        of.offset
    };

    let bytes = result::option(crate::syscall::fs::fs_read_to_vec(file_idx, start_offset, count), SysError::BadFd)?;
    let written = result::option(write_to_fd(out_fd as usize, bytes.as_ptr(), bytes.len()), SysError::BadFd)?;

    if offset_ptr != 0 {
        let new_offset = (start_offset + written) as i64;
        result::ensure(helpers::copy_struct_to_user(offset_ptr, &new_offset), SysError::Fault)?;
    } else {
        let of = result::option(crate::proc::get_fd_mut(in_fd as usize), SysError::BadFd)?;
        of.offset = start_offset + written;
    }

    result::ok(written as u64)
}
