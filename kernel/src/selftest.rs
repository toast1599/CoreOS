//! Lightweight kernel self-tests intended to run during boot.

pub fn run_boot_sanity_suite() {
    crate::serial_fmt!("[SELFTEST] begin boot sanity suite\n");
    crate::arch::context::validate_syscall_frame_layout();
    crate::fs::selftest_ramfs_basic();
    crate::proc::selftest_process_layout();
    crate::serial_fmt!("[SELFTEST] boot sanity suite passed\n");
}

