//! Simple kernel debug logging for COM1 serial.
//!
//! Usage:
//!     dbg_log!("TASK", "spawned task {}", id);

/// Compile-time switch to enable/disable debug logs.
pub const DEBUG_ENABLED: bool = true;

/// Internal logging macro.
/// Do not call directly outside the crate.
#[macro_export]
macro_rules! dbg_log {
    ($tag:expr, $($arg:tt)*) => {{
        if $crate::debug::DEBUG_ENABLED {
            $crate::serial_fmt!(
                "[{}][{}:{}] {}\n",
                $tag,
                core::file!(),
                core::line!(),
                format_args!($($arg)*)
            );
        }
    }};
}

