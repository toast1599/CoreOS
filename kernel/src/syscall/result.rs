pub const FAILURE: u64 = u64::MAX;

#[derive(Clone, Copy, Debug)]
pub enum SysError {
    BadFd,
    Fault,
    Invalid,
    NoEntry,
    Unsupported,
}

pub type SysResult<T = u64> = core::result::Result<T, SysError>;

pub fn ret(result: SysResult) -> u64 {
    match result {
        Ok(value) => value,
        Err(_) => FAILURE,
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
