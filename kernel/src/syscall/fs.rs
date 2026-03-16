use crate::proc;
use crate::drivers::vga;
use crate::drivers::serial;
use crate::hw::kbd_buffer;

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

pub unsafe fn fs_file_size(file_idx: usize) -> usize {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    match fs_guard.as_ref() {
        Some(fs) if file_idx < fs.files.len() => fs.files[file_idx].data.len(),
        _ => 0,
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

// ---------------------------------------------------------------------------
// syscall implementations
// ---------------------------------------------------------------------------

pub unsafe fn syscall_write(_fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let len = count as usize;
    if !crate::usercopy::user_range_ok(buf_ptr, len) {
        return u64::MAX;
    }
    let buf = buf_ptr as *const u8;
    for i in 0..len {
        let b = *buf.add(i);
        serial::write_byte(b);
        vga::console::write_byte_to_fb(b);
    }
    len as u64
}

pub unsafe fn syscall_read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let buf = buf_ptr as *mut u8;
    let count = count as usize;
    if count == 0 {
        return 0;
    }
    if !crate::usercopy::user_range_ok(buf_ptr, count) {
        return u64::MAX;
    }

    if fd == 0 {
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
        return count as u64;
    }

    let fd_usize = fd as usize;
    let of = match proc::get_fd_mut(fd_usize) {
        Some(f) => f,
        None => return u64::MAX,
    };
    let bytes_read = fs_read(of.file_idx, of.offset, buf, count);
    of.offset += bytes_read;
    bytes_read as u64
}

pub unsafe fn syscall_open(path_ptr: u64, path_len: u64) -> u64 {
    if path_len == 0 || path_len > 64 {
        return u64::MAX;
    }
    let mut raw = [0u8; 64];
    if crate::usercopy::copy_from_user(&mut raw[..path_len as usize], path_ptr).is_err() {
        return u64::MAX;
    }
    let mut name_buf = ['\0'; 64];
    for i in 0..path_len as usize {
        name_buf[i] = raw[i] as char;
    }
    let name = &name_buf[..path_len as usize];
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

pub unsafe fn syscall_fsize(fd: u64) -> u64 {
    let of = match proc::get_fd(fd as usize) {
        Some(f) => f,
        None => return u64::MAX,
    };
    fs_file_size(of.file_idx) as u64
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
        if pos < buf_len {
            buf.add(pos).write(0);
            pos += 1;
        }
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
