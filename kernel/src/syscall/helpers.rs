use alloc::vec;

use crate::syscall::types::Iovec;

pub const MAX_PATH_LEN: usize = 64;
pub const MAX_IOV: usize = 16;

pub unsafe fn copy_path_from_user(path_ptr: u64, path_len: u64) -> Option<([char; MAX_PATH_LEN], usize)> {
    if path_len == 0 || path_len as usize > MAX_PATH_LEN {
        return None;
    }

    let mut raw = [0u8; MAX_PATH_LEN];
    crate::usercopy::copy_from_user(&mut raw[..path_len as usize], path_ptr).ok()?;

    let mut start = 0usize;
    while start < path_len as usize && raw[start] == b'/' {
        start += 1;
    }
    if start >= path_len as usize {
        return None;
    }

    let mut name_buf = ['\0'; MAX_PATH_LEN];
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

pub unsafe fn copy_struct_from_user<T: Copy>(user_ptr: u64) -> Option<T> {
    if !crate::usercopy::user_range_ok(user_ptr, core::mem::size_of::<T>()) {
        return None;
    }

    let mut raw = vec![0u8; core::mem::size_of::<T>()];
    crate::usercopy::copy_from_user(&mut raw, user_ptr).ok()?;
    Some(core::ptr::read_unaligned(raw.as_ptr().cast::<T>()))
}

pub unsafe fn copy_struct_to_user<T>(user_ptr: u64, value: &T) -> bool {
    if !crate::usercopy::user_range_ok(user_ptr, core::mem::size_of::<T>()) {
        return false;
    }

    let bytes = core::slice::from_raw_parts(
        (value as *const T).cast::<u8>(),
        core::mem::size_of::<T>(),
    );
    crate::usercopy::copy_to_user(user_ptr, bytes).is_ok()
}

pub unsafe fn copy_iovecs_from_user(iov_ptr: u64, iovcnt: u64) -> Option<([Iovec; MAX_IOV], usize)> {
    let count = iovcnt as usize;
    if count == 0 || count > MAX_IOV {
        return None;
    }

    let bytes_len = count * core::mem::size_of::<Iovec>();
    if !crate::usercopy::user_range_ok(iov_ptr, bytes_len) {
        return None;
    }

    let mut iovs = [Iovec {
        iov_base: 0,
        iov_len: 0,
    }; MAX_IOV];
    let raw = core::slice::from_raw_parts_mut(iovs.as_mut_ptr().cast::<u8>(), bytes_len);
    crate::usercopy::copy_from_user(raw, iov_ptr).ok()?;
    Some((iovs, count))
}
