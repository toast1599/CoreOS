pub unsafe fn open(path_ptr: u64, path_len: u64) -> u64 {
    super::fs::syscall_open(path_ptr, path_len)
}

pub unsafe fn openat(dirfd: u64, path_ptr: u64, path_len: u64, flags: u64) -> u64 {
    super::fs::syscall_openat(dirfd, path_ptr, path_len, flags)
}

pub unsafe fn fsize(fd: u64) -> u64 {
    super::fs::syscall_fsize(fd)
}

pub unsafe fn fstat(fd: u64, stat_ptr: u64) -> u64 {
    super::fs::syscall_fstat(fd, stat_ptr)
}

pub unsafe fn fstatat(dirfd: u64, path_ptr: u64, path_len: u64, stat_ptr: u64) -> u64 {
    super::fs::syscall_fstatat(dirfd, path_ptr, path_len, stat_ptr)
}

pub unsafe fn lseek(fd: u64, offset: u64, whence: u64) -> u64 {
    super::fs::syscall_lseek(fd, offset, whence)
}

pub unsafe fn ls(buf_ptr: u64, buf_len: u64) -> u64 {
    super::fs::syscall_ls(buf_ptr, buf_len)
}

pub unsafe fn touch(name_ptr: u64, name_len: u64) -> u64 {
    super::fs::syscall_touch(name_ptr, name_len)
}

pub unsafe fn rm(name_ptr: u64, name_len: u64) -> u64 {
    super::fs::syscall_rm(name_ptr, name_len)
}

pub unsafe fn write_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    super::fs::syscall_write_file(name_ptr, name_len, args_ptr)
}

pub unsafe fn push_file(name_ptr: u64, name_len: u64, args_ptr: u64) -> u64 {
    super::fs::syscall_push_file(name_ptr, name_len, args_ptr)
}
