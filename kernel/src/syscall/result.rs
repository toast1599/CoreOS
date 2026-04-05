#[derive(Clone, Copy, Debug)]
pub enum SysError {
    Again,
    BadFd,
    Child,
    Fault,
    Invalid,
    NoEntry,
    NoMem,
    NoSys,
    AddrFamilyNotSupported,
    Access,
    NotSeekable,
    NotTty,
    Range,
    TimedOut,
    Unsupported,
}

pub type SysResult<T = u64> = core::result::Result<T, SysError>;

pub fn errno(error: SysError) -> u64 {
    let code = match error {
        SysError::BadFd => 9,
        SysError::Again => 11,
        SysError::Child => 10,
        SysError::Fault => 14,
        SysError::Invalid => 22,
        SysError::NoEntry => 2,
        SysError::NoMem => 12,
        SysError::NoSys => 38,
        SysError::AddrFamilyNotSupported => 97,
        SysError::Access => 13,
        SysError::NotSeekable => 29,
        SysError::NotTty => 25,
        SysError::Range => 34,
        SysError::TimedOut => 110,
        SysError::Unsupported => 95,
    };
    (-(code as i64)) as u64
}

pub fn ret(result: SysResult) -> u64 {
    match result {
        Ok(value) => value,
        Err(error) => errno(error),
    }
}

pub fn ok(value: impl Into<u64>) -> SysResult {
    Ok(value.into())
}

pub fn err(error: SysError) -> SysResult {
    Err(error)
}

pub fn ensure(condition: bool, error: SysError) -> SysResult<()> {
    if condition {
        Ok(())
    } else {
        Err(error)
    }
}

pub fn option<T>(value: Option<T>, error: SysError) -> SysResult<T> {
    value.ok_or(error)
}
