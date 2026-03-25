// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

#![feature(alloc_error_handler)]
#![allow(static_mut_refs)]
#![no_main]
#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

// --- Architecture ---
mod arch;
mod boot;
mod drivers;
mod hw; // Centralized hardware (PIC, PIT, PS2, RTC, etc.)
mod mem;
mod sync;
mod syscall;
mod usercopy;

// --- Memory & Execution ---
mod proc;

// --- Filesystem & UI ---
mod bench;
mod debug;
mod fs;
mod kernel_boot;
mod panic;
mod shell;
mod vfs;

use crate::mem::heap::SlabAllocator;

// ---------------------------------------------------------------------------
// Global allocator
// ---------------------------------------------------------------------------

#[global_allocator]
static ALLOCATOR: SlabAllocator = SlabAllocator;

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    crate::serial_fmt!(
        "[ALLOC ERROR] size={} align={}\n",
        layout.size(),
        layout.align()
    );
    loop {}
}

// ---------------------------------------------------------------------------
// Demo background task
// ---------------------------------------------------------------------------

fn demo_task() {
    loop {
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

// ---------------------------------------------------------------------------
// Kernel entry point
// ---------------------------------------------------------------------------

#[export_name = "_start"]
#[link_section = ".text._start"]
pub unsafe extern "win64" fn _start(boot_info_phys: *const boot::CoreOS_BootInfo) -> ! {
    kernel_boot::zero_bss();

    drivers::serial::write_str("kernel start\n");
    crate::serial_fmt!("boot_info_phys={:p}\n", boot_info_phys);

    extern "C" {
        static __stack_top: u8;
    }
    core::arch::asm!(
        "lea rsp, [{stack}]",
        stack = sym __stack_top,
        options(nostack, nomem)
    );
    drivers::serial::write_str("stack ok\n");
    let stack_top = &raw const __stack_top as usize;

    let boot_info = kernel_boot::boot_kernel(boot_info_phys, stack_top, demo_task);

    shell::ui::run_shell(boot_info);
}
