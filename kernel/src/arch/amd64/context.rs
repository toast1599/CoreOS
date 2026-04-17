//! Context-layout contracts shared across syscall and task switching code.
//!
//! This module intentionally centralizes register/frame layout assumptions so
//! refactors in low-level assembly have a single Rust-side contract to check.

use crate::syscall::types::SyscallFrame;

/// Number of 64-bit register slots captured by the syscall entry stub.
pub const SYSCALL_FRAME_QWORDS: usize = 15;
/// Size of the saved register frame in bytes.
pub const SYSCALL_FRAME_SIZE: usize = SYSCALL_FRAME_QWORDS * core::mem::size_of::<u64>();

/// Offsets (in bytes) for key fields used by trampoline / fork paths.
pub const SYSCALL_FRAME_RAX_OFFSET: usize = 11 * 8;
pub const SYSCALL_FRAME_USER_RSP_OFFSET: usize = 14 * 8; // r10 slot

#[inline]
pub fn validate_syscall_frame_layout() {
    // Keep as runtime assertions so failures remain visible in QEMU logs during
    // early bring-up.
    assert_eq!(
        core::mem::size_of::<SyscallFrame>(),
        SYSCALL_FRAME_SIZE,
        "SyscallFrame size changed; update assembly contracts"
    );
    assert_eq!(
        core::mem::offset_of!(SyscallFrame, rax),
        SYSCALL_FRAME_RAX_OFFSET,
        "SyscallFrame::rax offset changed; update assembly contracts"
    );
    assert_eq!(
        core::mem::offset_of!(SyscallFrame, r10),
        SYSCALL_FRAME_USER_RSP_OFFSET,
        "SyscallFrame::r10 offset changed; update assembly contracts"
    );
}

