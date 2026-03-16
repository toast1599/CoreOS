pub unsafe fn nanosleep(req_ptr: u64, rem_ptr: u64) -> u64 {
    super::process::syscall_nanosleep(req_ptr, rem_ptr)
}

pub unsafe fn clock_gettime(clockid: u64, tp_ptr: u64) -> u64 {
    super::process::syscall_clock_gettime(clockid, tp_ptr)
}

pub unsafe fn boottime(buf_ptr: u64, buf_len: u64) -> u64 {
    super::process::syscall_boottime(buf_ptr, buf_len)
}

pub fn uptime_seconds() -> u64 {
    crate::hw::pit::uptime_seconds()
}

pub fn ticks() -> u64 {
    crate::hw::pit::ticks()
}

pub fn sleep_ticks(ticks: u64) -> u64 {
    crate::hw::pit::sleep_yield(ticks);
    0
}
