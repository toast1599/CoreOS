/// Verbose debug logging to COM1 serial.
/// Usage:  dbg_log!("TAG", "message {}", value);
#[macro_export]
macro_rules! dbg_log {
    ($tag:expr, $($arg:tt)*) => {{
        $crate::serial_fmt!("[{}] ", $tag);
        $crate::serial_fmt!($($arg)*);
        $crate::serial_fmt!("\n");
    }};
}

