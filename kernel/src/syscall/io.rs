use crate::drivers::vga;
use crate::proc::fd_io::{read_from_fd, write_to_fd};
use crate::proc::DescriptorInfo;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use crate::syscall::types::Termios;
use crate::syscall::types::WinSize;
use crate::syscall::types::{TCGETS, TIOCGWINSZ};

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
        SysError::BadFd,
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
        _ => result::err(SysError::Unsupported),
    }
}
