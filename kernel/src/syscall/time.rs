use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};

const CLOCK_REALTIME: u64 = 0;
const CLOCK_MONOTONIC: u64 = 1;
const NANOS_PER_SEC: u64 = 1_000_000_000;
const PIT_HZ: u64 = 100;
const TIMER_ABSTIME: u64 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Timeval {
    tv_sec: i64,
    tv_usec: i64,
}

pub unsafe fn nanosleep(req_ptr: u64, rem_ptr: u64) -> u64 {
    result::ret(nanosleep_impl(req_ptr, rem_ptr))
}

unsafe fn nanosleep_impl(req_ptr: u64, rem_ptr: u64) -> SysResult {
    let req: Timespec = result::option(helpers::copy_struct_from_user(req_ptr), SysError::Fault)?;
    sleep_timespec(req, rem_ptr)
}

unsafe fn sleep_timespec(req: Timespec, rem_ptr: u64) -> SysResult {
    result::ensure(
        req.tv_sec >= 0 && req.tv_nsec >= 0 && req.tv_nsec < NANOS_PER_SEC as i64,
        SysError::Invalid,
    )?;

    let total_ns = (req.tv_sec as u64)
        .saturating_mul(NANOS_PER_SEC)
        .saturating_add(req.tv_nsec as u64);
    let ticks = total_ns.div_ceil(NANOS_PER_SEC / PIT_HZ);
    crate::hw::pit::sleep_yield(ticks);

    if rem_ptr != 0 {
        let rem = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        result::ensure(helpers::copy_struct_to_user(rem_ptr, &rem), SysError::Fault)?;
    }

    result::ok(0u64)
}

pub unsafe fn clock_gettime(clockid: u64, tp_ptr: u64) -> u64 {
    result::ret(clock_gettime_impl(clockid, tp_ptr))
}

unsafe fn clock_gettime_impl(clockid: u64, tp_ptr: u64) -> SysResult {
    result::ensure(
        matches!(clockid, CLOCK_REALTIME | CLOCK_MONOTONIC),
        SysError::Invalid,
    )?;

    let ticks = crate::hw::pit::ticks();
    let ts = Timespec {
        tv_sec: (ticks / PIT_HZ) as i64,
        tv_nsec: ((ticks % PIT_HZ) * (NANOS_PER_SEC / PIT_HZ)) as i64,
    };

    result::ensure(helpers::copy_struct_to_user(tp_ptr, &ts), SysError::Fault)?;
    result::ok(0u64)
}

pub unsafe fn gettimeofday(tv_ptr: u64, tz_ptr: u64) -> u64 {
    result::ret(gettimeofday_impl(tv_ptr, tz_ptr))
}

unsafe fn gettimeofday_impl(tv_ptr: u64, tz_ptr: u64) -> SysResult {
    result::ensure(tz_ptr == 0, SysError::Unsupported)?;
    let ticks = crate::hw::pit::ticks();
    let tv = Timeval {
        tv_sec: (ticks / PIT_HZ) as i64,
        tv_usec: ((ticks % PIT_HZ) * (1_000_000 / PIT_HZ)) as i64,
    };
    result::ensure(helpers::copy_struct_to_user(tv_ptr, &tv), SysError::Fault)?;
    result::ok(0u64)
}

pub unsafe fn clock_nanosleep(clockid: u64, flags: u64, req_ptr: u64, rem_ptr: u64) -> u64 {
    result::ret(clock_nanosleep_impl(clockid, flags, req_ptr, rem_ptr))
}

unsafe fn clock_nanosleep_impl(clockid: u64, flags: u64, req_ptr: u64, rem_ptr: u64) -> SysResult {
    result::ensure(
        matches!(clockid, CLOCK_REALTIME | CLOCK_MONOTONIC),
        SysError::Invalid,
    )?;

    if flags == 0 {
        return nanosleep_impl(req_ptr, rem_ptr);
    }

    result::ensure(flags == TIMER_ABSTIME, SysError::Unsupported)?;
    let req: Timespec = result::option(helpers::copy_struct_from_user(req_ptr), SysError::Fault)?;
    result::ensure(
        req.tv_sec >= 0 && req.tv_nsec >= 0 && req.tv_nsec < NANOS_PER_SEC as i64,
        SysError::Invalid,
    )?;

    let ticks = crate::hw::pit::ticks();
    let now = Timespec {
        tv_sec: (ticks / PIT_HZ) as i64,
        tv_nsec: ((ticks % PIT_HZ) * (NANOS_PER_SEC / PIT_HZ)) as i64,
    };

    let req_ns = (req.tv_sec as i128) * (NANOS_PER_SEC as i128) + (req.tv_nsec as i128);
    let now_ns = (now.tv_sec as i128) * (NANOS_PER_SEC as i128) + (now.tv_nsec as i128);
    if req_ns <= now_ns {
        if rem_ptr != 0 {
            let rem = Timespec { tv_sec: 0, tv_nsec: 0 };
            result::ensure(helpers::copy_struct_to_user(rem_ptr, &rem), SysError::Fault)?;
        }
        return result::ok(0u64);
    }

    let delta_ns = (req_ns - now_ns) as u64;
    let rel = Timespec {
        tv_sec: (delta_ns / NANOS_PER_SEC) as i64,
        tv_nsec: (delta_ns % NANOS_PER_SEC) as i64,
    };
    sleep_timespec(rel, rem_ptr)
}

pub unsafe fn boottime(buf_ptr: u64, buf_len: u64) -> u64 {
    result::ret(boottime_impl(buf_ptr, buf_len))
}

unsafe fn boottime_impl(buf_ptr: u64, buf_len: u64) -> SysResult {
    let report = crate::bench::report();
    let bytes = report.as_bytes();
    let count = (buf_len as usize).min(bytes.len());
    result::ensure(
        crate::usercopy::copy_to_user(buf_ptr, &bytes[..count]).is_ok(),
        SysError::Fault,
    )?;
    result::ok(count as u64)
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
