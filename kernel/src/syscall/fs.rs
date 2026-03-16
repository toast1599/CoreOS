use crate::proc;
use crate::proc::FdTarget;
use crate::drivers::vga;
use crate::drivers::serial;
use crate::hw::kbd_buffer;

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
struct WinSize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

#[repr(C)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Iovec {
    iov_base: u64,
    iov_len: u64,
}

const SEEK_SET: u64 = 0;
const SEEK_CUR: u64 = 1;
const SEEK_END: u64 = 2;
const S_IFREG: u32 = 0o100000;
const S_IFCHR: u32 = 0o020000;
const S_IFIFO: u32 = 0o010000;
const DEFAULT_FILE_MODE: u32 = S_IFREG | 0o644;
const DEFAULT_CHAR_MODE: u32 = S_IFCHR | 0o666;
const DEFAULT_PIPE_MODE: u32 = S_IFIFO | 0o666;
const AT_FDCWD: i64 = -100;
const MAX_IOV: usize = 16;
const TCGETS: u64 = 0x5401;
const TIOCGWINSZ: u64 = 0x5413;

// ---------------------------------------------------------------------------
// Filesystem helpers
// ---------------------------------------------------------------------------

pub unsafe fn fs_find_idx(name: &[char]) -> Option<usize> {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    fs_guard
        .as_ref()?
        .files
        .iter()
        .position(|f| f.name.as_slice() == name)
}

unsafe fn user_path_to_name(path_ptr: u64, path_len: u64) -> Option<([char; 64], usize)> {
    if path_len == 0 || path_len > 64 {
        return None;
    }
    let mut raw = [0u8; 64];
    crate::usercopy::copy_from_user(&mut raw[..path_len as usize], path_ptr).ok()?;

    let mut start = 0usize;
    while start < path_len as usize && raw[start] == b'/' {
        start += 1;
    }
    if start >= path_len as usize {
        return None;
    }

    let mut name_buf = ['\0'; 64];
    let mut out_len = 0usize;
    for &b in &raw[start..path_len as usize] {
        if b == b'/' || out_len >= name_buf.len() {
            return None;
        }
        name_buf[out_len] = b as char;
        out_len += 1;
    }
    if out_len == 0 {
        None
    } else {
        Some((name_buf, out_len))
    }
}

pub unsafe fn fs_file_size(file_idx: usize) -> usize {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    match fs_guard.as_ref() {
        Some(fs) if file_idx < fs.files.len() => fs.files[file_idx].data.len(),
        _ => 0,
    }
}

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

pub unsafe fn fs_read(file_idx: usize, offset: usize, buf: *mut u8, count: usize) -> usize {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    let fs = match fs_guard.as_ref() {
        Some(f) => f,
        None => return 0,
    };
    if file_idx >= fs.files.len() {
        return 0;
    }
    let data = &fs.files[file_idx].data;
    let available = data.len().saturating_sub(offset);
    let to_read = count.min(available);
    if to_read == 0 {
        return 0;
    }
    core::ptr::copy_nonoverlapping(data[offset..].as_ptr(), buf, to_read);
    to_read
}

pub unsafe fn fs_clone_by_name(name: &[char]) -> Option<alloc::vec::Vec<u8>> {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    let fs = fs_guard.as_ref()?;
    let file = fs.files.iter().find(|f| f.name.as_slice() == name)?;
    Some(file.data.clone())
}

unsafe fn write_to_fd(fd: usize, buf: *const u8, len: usize) -> Option<usize> {
    match proc::get_fd_target(fd) {
        Some(FdTarget::Stdio(1)) | Some(FdTarget::Stdio(2)) => {
            for i in 0..len {
                let b = *buf.add(i);
                serial::write_byte(b);
                vga::console::write_byte_to_fb(b);
            }
            Some(len)
        }
        Some(FdTarget::PipeWrite(_)) => {
            let (pipe, is_read_end) = proc::get_pipe_mut(fd)?;
            if is_read_end {
                return None;
            }
            if pipe.read_refs == 0 {
                return Some(0);
            }
            let mut written = 0usize;
            while written < len && pipe.len < 1024 {
                pipe.buf[pipe.write_pos] = *buf.add(written);
                pipe.write_pos = (pipe.write_pos + 1) % 1024;
                pipe.len += 1;
                written += 1;
            }
            Some(written)
        }
        _ => None,
    }
}

unsafe fn read_from_fd(fd: usize, buf: *mut u8, count: usize) -> Option<usize> {
    if count == 0 {
        return Some(0);
    }

    if matches!(proc::get_fd_target(fd), Some(FdTarget::Stdio(0))) {
        for i in 0..count {
            let c = loop {
                crate::proc::scheduler::IN_SYSCALL.store(false, core::sync::atomic::Ordering::Relaxed);
                core::arch::asm!("sti", options(nostack, nomem));
                for _ in 0..2000 {
                    core::hint::spin_loop();
                }
                core::arch::asm!("cli", options(nostack, nomem));
                crate::proc::scheduler::IN_SYSCALL.store(true, core::sync::atomic::Ordering::Relaxed);

                if let Some(c) = kbd_buffer::KEYBUF.pop() {
                    break c;
                }
            };
            buf.add(i).write(c as u8);
        }
        return Some(count);
    }

    if matches!(proc::get_fd_target(fd), Some(FdTarget::PipeRead(_))) {
        loop {
            let peer_closed = proc::pipe_peer_closed(fd)?;
            let (pipe, is_read_end) = proc::get_pipe_mut(fd)?;
            if !is_read_end {
                return None;
            }
            if pipe.len > 0 {
                let mut read = 0usize;
                while read < count && pipe.len > 0 {
                    buf.add(read).write(pipe.buf[pipe.read_pos]);
                    pipe.read_pos = (pipe.read_pos + 1) % 1024;
                    pipe.len -= 1;
                    read += 1;
                }
                return Some(read);
            }
            if peer_closed {
                return Some(0);
            }

            crate::proc::scheduler::IN_SYSCALL.store(false, core::sync::atomic::Ordering::Relaxed);
            core::arch::asm!("sti", options(nostack, nomem));
            for _ in 0..2000 {
                core::hint::spin_loop();
            }
            core::arch::asm!("cli", options(nostack, nomem));
            crate::proc::scheduler::IN_SYSCALL.store(true, core::sync::atomic::Ordering::Relaxed);
        }
    }

    let of = proc::get_fd_mut(fd)?;
    let bytes_read = fs_read(of.file_idx, of.offset, buf, count);
    of.offset += bytes_read;
    Some(bytes_read)
}

// ---------------------------------------------------------------------------
// syscall implementations
// ---------------------------------------------------------------------------

pub unsafe fn syscall_write(_fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let len = count as usize;
    if !crate::usercopy::user_range_ok(buf_ptr, len) {
        return u64::MAX;
    }
    match write_to_fd(_fd as usize, buf_ptr as *const u8, len) {
        Some(written) => written as u64,
        None => u64::MAX,
    }
}

pub unsafe fn syscall_read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let count = count as usize;
    if !crate::usercopy::user_range_ok(buf_ptr, count) {
        return u64::MAX;
    }
    match read_from_fd(fd as usize, buf_ptr as *mut u8, count) {
        Some(read) => read as u64,
        None => u64::MAX,
    }
}

pub unsafe fn syscall_writev(fd: u64, iov_ptr: u64, iovcnt: u64) -> u64 {
    let count = iovcnt as usize;
    if count == 0 || count > MAX_IOV {
        return u64::MAX;
    }
    let bytes_len = count * core::mem::size_of::<Iovec>();
    if !crate::usercopy::user_range_ok(iov_ptr, bytes_len) {
        return u64::MAX;
    }

    let mut iovs = [Iovec {
        iov_base: 0,
        iov_len: 0,
    }; MAX_IOV];
    let raw = core::slice::from_raw_parts_mut(iovs.as_mut_ptr().cast::<u8>(), bytes_len);
    if crate::usercopy::copy_from_user(raw, iov_ptr).is_err() {
        return u64::MAX;
    }

    let mut total = 0usize;
    for iov in &iovs[..count] {
        let len = iov.iov_len as usize;
        if !crate::usercopy::user_range_ok(iov.iov_base, len) {
            return u64::MAX;
        }
        match write_to_fd(fd as usize, iov.iov_base as *const u8, len) {
            Some(written) => {
                total = total.saturating_add(written);
                if written != len {
                    break;
                }
            }
            None => return u64::MAX,
        }
    }
    total as u64
}

pub unsafe fn syscall_readv(fd: u64, iov_ptr: u64, iovcnt: u64) -> u64 {
    let count = iovcnt as usize;
    if count == 0 || count > MAX_IOV {
        return u64::MAX;
    }
    let bytes_len = count * core::mem::size_of::<Iovec>();
    if !crate::usercopy::user_range_ok(iov_ptr, bytes_len) {
        return u64::MAX;
    }

    let mut iovs = [Iovec {
        iov_base: 0,
        iov_len: 0,
    }; MAX_IOV];
    let raw = core::slice::from_raw_parts_mut(iovs.as_mut_ptr().cast::<u8>(), bytes_len);
    if crate::usercopy::copy_from_user(raw, iov_ptr).is_err() {
        return u64::MAX;
    }

    let mut total = 0usize;
    for iov in &iovs[..count] {
        let len = iov.iov_len as usize;
        if !crate::usercopy::user_range_ok(iov.iov_base, len) {
            return u64::MAX;
        }
        match read_from_fd(fd as usize, iov.iov_base as *mut u8, len) {
            Some(read) => {
                total = total.saturating_add(read);
                if read != len {
                    break;
                }
            }
            None => return u64::MAX,
        }
    }
    total as u64
}

pub unsafe fn syscall_open(path_ptr: u64, path_len: u64) -> u64 {
    let (name_buf, name_len) = match user_path_to_name(path_ptr, path_len) {
        Some(v) => v,
        None => return u64::MAX,
    };
    let name = &name_buf[..name_len];
    let file_idx = match fs_find_idx(name) {
        Some(i) => i,
        None => return u64::MAX,
    };
    let fd = proc::alloc_fd(file_idx);
    if fd < 0 {
        u64::MAX
    } else {
        fd as u64
    }
}

pub unsafe fn syscall_openat(dirfd: u64, path_ptr: u64, path_len: u64, _flags: u64) -> u64 {
    if dirfd as i64 != AT_FDCWD {
        return u64::MAX;
    }
    syscall_open(path_ptr, path_len)
}

pub unsafe fn syscall_fsize(fd: u64) -> u64 {
    let of = match proc::get_fd(fd as usize) {
        Some(f) => f,
        None => return u64::MAX,
    };
    fs_file_size(of.file_idx) as u64
}

pub unsafe fn syscall_fstat(fd: u64, stat_ptr: u64) -> u64 {
    if !crate::usercopy::user_range_ok(stat_ptr, core::mem::size_of::<Stat>()) {
        return u64::MAX;
    }

    let stat = match proc::get_fd_target(fd as usize) {
        Some(FdTarget::Stdio(n)) => Stat {
            st_dev: 0,
            st_ino: n as u64,
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
        Some(FdTarget::Open(_)) => {
            let of = match proc::get_fd(fd as usize) {
                Some(f) => f,
                None => return u64::MAX,
            };
            let size = fs_file_size(of.file_idx) as i64;
            stat_for_file_idx(of.file_idx, size)
        }
        Some(FdTarget::PipeRead(_)) | Some(FdTarget::PipeWrite(_)) => Stat {
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
        _ => return u64::MAX,
    };

    let bytes = core::slice::from_raw_parts(
        (&stat as *const Stat).cast::<u8>(),
        core::mem::size_of::<Stat>(),
    );
    if crate::usercopy::copy_to_user(stat_ptr, bytes).is_err() {
        return u64::MAX;
    }
    0
}

pub unsafe fn syscall_fstatat(dirfd: u64, path_ptr: u64, path_len: u64, stat_ptr: u64) -> u64 {
    if dirfd as i64 != AT_FDCWD {
        return u64::MAX;
    }
    if !crate::usercopy::user_range_ok(stat_ptr, core::mem::size_of::<Stat>()) {
        return u64::MAX;
    }
    let (name_buf, name_len) = match user_path_to_name(path_ptr, path_len) {
        Some(v) => v,
        None => return u64::MAX,
    };
    let file_idx = match fs_find_idx(&name_buf[..name_len]) {
        Some(i) => i,
        None => return u64::MAX,
    };
    let stat = stat_for_file_idx(file_idx, fs_file_size(file_idx) as i64);
    let bytes = core::slice::from_raw_parts(
        (&stat as *const Stat).cast::<u8>(),
        core::mem::size_of::<Stat>(),
    );
    if crate::usercopy::copy_to_user(stat_ptr, bytes).is_err() {
        return u64::MAX;
    }
    0
}

pub unsafe fn syscall_lseek(fd: u64, offset: u64, whence: u64) -> u64 {
    if !matches!(proc::get_fd_target(fd as usize), Some(FdTarget::Open(_))) {
        return u64::MAX;
    }

    let of = match proc::get_fd_mut(fd as usize) {
        Some(f) => f,
        None => return u64::MAX,
    };
    let file_size = fs_file_size(of.file_idx) as i64;
    let cur = of.offset as i64;
    let off = offset as i64;

    let new_off = match whence {
        SEEK_SET => off,
        SEEK_CUR => cur.saturating_add(off),
        SEEK_END => file_size.saturating_add(off),
        _ => return u64::MAX,
    };

    if new_off < 0 {
        return u64::MAX;
    }

    of.offset = new_off as usize;
    new_off as u64
}

pub unsafe fn syscall_ioctl(fd: u64, req: u64, argp: u64) -> u64 {
    if !matches!(proc::get_fd_target(fd as usize), Some(FdTarget::Stdio(_))) {
        return u64::MAX;
    }

    match req {
        TCGETS => {
            if !crate::usercopy::user_range_ok(argp, core::mem::size_of::<Termios>()) {
                return u64::MAX;
            }
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
            let bytes = core::slice::from_raw_parts(
                (&termios as *const Termios).cast::<u8>(),
                core::mem::size_of::<Termios>(),
            );
            if crate::usercopy::copy_to_user(argp, bytes).is_err() {
                return u64::MAX;
            }
            0
        }
        TIOCGWINSZ => {
            if !crate::usercopy::user_range_ok(argp, core::mem::size_of::<WinSize>()) {
                return u64::MAX;
            }
            let (rows, cols, xpixel, ypixel) = vga::console::tty_winsize();
            let winsz = WinSize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: xpixel,
                ws_ypixel: ypixel,
            };
            let bytes = core::slice::from_raw_parts(
                (&winsz as *const WinSize).cast::<u8>(),
                core::mem::size_of::<WinSize>(),
            );
            if crate::usercopy::copy_to_user(argp, bytes).is_err() {
                return u64::MAX;
            }
            0
        }
        _ => u64::MAX,
    }
}

pub unsafe fn syscall_ls(buf_ptr: u64, buf_len: u64) -> u64 {
    if !crate::usercopy::user_range_ok(buf_ptr, buf_len as usize) {
        return u64::MAX;
    }
    let fs_guard = crate::fs::FILESYSTEM.lock();
    let fs = match fs_guard.as_ref() {
        Some(f) => f,
        None => return 0,
    };
    let buf = buf_ptr as *mut u8;
    let buf_len = buf_len as usize;
    let mut pos = 0usize;
    for file in fs.files.iter() {
        for &ch in file.name.iter() {
            if pos + 1 >= buf_len {
                break;
            }
            buf.add(pos).write(ch as u8);
            pos += 1;
        }

        let size = file.data.len();
        let suffix = b" (";
        for &b in suffix {
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

        let tail = b" bytes)\n";
        for &b in tail {
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
    pos as u64
}

pub unsafe fn syscall_touch(name_ptr: u64, name_len: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    let mut raw = [0u8; 64];
    if crate::usercopy::copy_from_user(&mut raw[..name_len as usize], name_ptr).is_err() {
        return u64::MAX;
    }
    let mut name_buf = ['\0'; 64];
    for i in 0..name_len as usize {
        name_buf[i] = raw[i] as char;
    }
    let name = &name_buf[..name_len as usize];
    let mut fs_guard = crate::fs::FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => {
            if fs.create(name) {
                0
            } else {
                u64::MAX
            }
        }
        None => u64::MAX,
    }
}

pub unsafe fn syscall_rm(name_ptr: u64, name_len: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    let mut raw = [0u8; 64];
    if crate::usercopy::copy_from_user(&mut raw[..name_len as usize], name_ptr).is_err() {
        return u64::MAX;
    }
    let mut name_buf = ['\0'; 64];
    for i in 0..name_len as usize {
        name_buf[i] = raw[i] as char;
    }
    let name = &name_buf[..name_len as usize];
    let mut fs_guard = crate::fs::FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => {
            if fs.remove(name) {
                0
            } else {
                u64::MAX
            }
        }
        None => u64::MAX,
    }
}

pub unsafe fn syscall_write_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    if !crate::usercopy::user_range_ok(args_ptr, 16) {
        return u64::MAX;
    }
    let data_ptr = (args_ptr as *const u64).read();
    let data_len = (args_ptr as *const u64).add(1).read() as usize;
    let mut raw = [0u8; 64];
    if crate::usercopy::copy_from_user(&mut raw[..name_len as usize], name_ptr).is_err() {
        return u64::MAX;
    }
    if !crate::usercopy::user_range_ok(data_ptr, data_len) {
        return u64::MAX;
    }
    let mut name_buf = ['\0'; 64];
    for i in 0..name_len as usize {
        name_buf[i] = raw[i] as char;
    }
    let name = &name_buf[..name_len as usize];
    let mut fs_guard = crate::fs::FILESYSTEM.lock();
    let fs = match fs_guard.as_mut() {
        Some(f) => f,
        None => return u64::MAX,
    };
    let file = match fs.find_mut(name) {
        Some(f) => f,
        None => return u64::MAX,
    };
    file.data.clear();
    let src = core::slice::from_raw_parts(data_ptr as *const u8, data_len);
    file.data.extend_from_slice(src);
    0
}

pub unsafe fn syscall_push_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    if name_len == 0 || name_len > 64 {
        return u64::MAX;
    }
    if !crate::usercopy::user_range_ok(args_ptr, 16) {
        return u64::MAX;
    }
    let data_ptr = (args_ptr as *const u64).read();
    let data_len = (args_ptr as *const u64).add(1).read() as usize;
    let mut raw = [0u8; 64];
    if crate::usercopy::copy_from_user(&mut raw[..name_len as usize], name_ptr).is_err() {
        return u64::MAX;
    }
    if !crate::usercopy::user_range_ok(data_ptr, data_len) {
        return u64::MAX;
    }
    let mut name_buf = ['\0'; 64];
    for i in 0..name_len as usize {
        name_buf[i] = raw[i] as char;
    }
    let name = &name_buf[..name_len as usize];
    let mut fs_guard = crate::fs::FILESYSTEM.lock();
    let fs = match fs_guard.as_mut() {
        Some(f) => f,
        None => return u64::MAX,
    };
    let file = match fs.find_mut(name) {
        Some(f) => f,
        None => return u64::MAX,
    };
    let src = core::slice::from_raw_parts(data_ptr as *const u8, data_len);
    file.data.extend_from_slice(src);
    0
}
