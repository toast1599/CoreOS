pub unsafe fn brk(addr: u64) -> u64 {
    super::process::syscall_brk(addr)
}

pub unsafe fn mmap(args_ptr: u64) -> u64 {
    super::process::syscall_mmap(args_ptr)
}

pub unsafe fn mprotect(addr: u64, len: u64, prot: u64) -> u64 {
    super::process::syscall_mprotect(addr, len, prot)
}

pub unsafe fn munmap(addr: u64, len: u64) -> u64 {
    super::process::syscall_munmap(addr, len)
}
