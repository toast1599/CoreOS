pub unsafe fn read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    super::fs::syscall_read(fd, buf_ptr, count)
}

pub unsafe fn write(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    super::fs::syscall_write(fd, buf_ptr, count)
}

pub unsafe fn readv(fd: u64, iov_ptr: u64, iovcnt: u64) -> u64 {
    super::fs::syscall_readv(fd, iov_ptr, iovcnt)
}

pub unsafe fn writev(fd: u64, iov_ptr: u64, iovcnt: u64) -> u64 {
    super::fs::syscall_writev(fd, iov_ptr, iovcnt)
}

pub unsafe fn ioctl(fd: u64, req: u64, argp: u64) -> u64 {
    super::fs::syscall_ioctl(fd, req, argp)
}
