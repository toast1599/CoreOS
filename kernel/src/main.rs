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
mod panic;
mod shell;

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
    // -----------------------------------------------------------------------
    // -1. Zero BSS (Must happen first for serial_fmt! to work)
    // -----------------------------------------------------------------------
    {
        extern "C" {
            static mut __bss_start: u8;
            static mut __bss_end: u8;
        }
        let bss_start = &raw mut __bss_start as *mut u8;
        let bss_end = &raw mut __bss_end as *mut u8;
        core::ptr::write_bytes(bss_start, 0, bss_end as usize - bss_start as usize);
    }

    // -----------------------------------------------------------------------
    // 1. Initial Serial & Memory
    // -----------------------------------------------------------------------
    drivers::serial::write_str("kernel start\n");
    crate::serial_fmt!("boot_info_phys={:p}\n", boot_info_phys);

    // -----------------------------------------------------------------------
    // 0. High-Half Boot Info Conversion
    // -----------------------------------------------------------------------
    // The bootloader preserves the UEFI identity map, so we can use boot_info_phys directly first.
    static mut BOOT_INFO_DATA: boot::CoreOS_BootInfo = boot::CoreOS_BootInfo {
        fb_base: 0,
        fb_size: 0,
        width: 0,
        height: 0,
        pitch: 0,
        mmap: [boot::MemMapEntry {
            physical_start: 0,
            num_pages: 0,
            mem_type: 0,
            _pad: 0,
        }; 256],
        mmap_count: 0,
        _pad: 0,
        user_elf_base: 0,
        user_elf_size: 0,
        font_base: 0,
        font_size: 0,
        tsc_bootloader_start: 0,
    };
    BOOT_INFO_DATA = core::ptr::read_unaligned(boot_info_phys);

    // Check if the original pointers are valid
    BOOT_INFO_DATA.fb_base = arch::amd64::paging::p2v(BOOT_INFO_DATA.fb_base as usize) as u64;
    if BOOT_INFO_DATA.font_base != 0 {
        BOOT_INFO_DATA.font_base =
            arch::amd64::paging::p2v(BOOT_INFO_DATA.font_base as usize) as u64;
    }
    if BOOT_INFO_DATA.user_elf_base != 0 {
        BOOT_INFO_DATA.user_elf_base =
            arch::amd64::paging::p2v(BOOT_INFO_DATA.user_elf_base as usize) as u64;
    }
    let boot_info = core::ptr::addr_of!(BOOT_INFO_DATA) as *const boot::CoreOS_BootInfo;

    bench::stamp(bench::Phase::KernelEntry);
    let bl_tsc = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).tsc_bootloader_start));
    bench::set_bootloader_tsc(bl_tsc);

    // -----------------------------------------------------------------------
    // 1. Switch to our own stack
    // -----------------------------------------------------------------------
    extern "C" {
        static __stack_top: u8;
    }
    core::arch::asm!(
        "lea rsp, [{stack}]",
        stack = sym __stack_top,
        options(nostack, nomem)
    );
    drivers::serial::write_str("stack ok\n");

    // -----------------------------------------------------------------------
    // 2. Physical memory manager
    // -----------------------------------------------------------------------    // 2. Physical memory manager
    let kernel_end = &raw const __stack_top as usize - 0xFFFFFFFF80000000 + 0x200000;
    drivers::serial::write_str("calling pmm::init\n");
    mem::pmm::init(boot_info, kernel_end);
    drivers::serial::write_str("pmm ok\n");
    bench::stamp(bench::Phase::PmmDone);

    // -----------------------------------------------------------------------
    // 2b. Save user ELF bytes to a static buffer
    // -----------------------------------------------------------------------
    static mut ELF_BUF: [u8; 64 * 1024] = [0u8; 64 * 1024];
    static mut ELF_LEN: usize = 0;
    static mut FONT_BUF: [u8; 16 * 1024] = [0u8; 16 * 1024];
    static mut FONT_LEN: usize = 0;

    let font_base = (*boot_info).font_base;
    let font_size = (*boot_info).font_size as usize;
    if font_base != 0 && font_size > 0 && font_size <= FONT_BUF.len() {
        let src = core::slice::from_raw_parts(font_base as *const u8, font_size);
        FONT_BUF[..font_size].copy_from_slice(src);
        FONT_LEN = font_size;
        drivers::serial::write_str("font loaded into static buffer\n");
    }

    let elf_base = (*boot_info).user_elf_base;
    let elf_size = (*boot_info).user_elf_size as usize;
    if elf_base != 0 && elf_size > 0 && elf_size <= ELF_BUF.len() {
        let src = core::slice::from_raw_parts(elf_base as *const u8, elf_size);
        ELF_BUF[..elf_size].copy_from_slice(src);
        ELF_LEN = elf_size;
        drivers::serial::write_str("elf bytes saved to static buffer\n");
    }

    // -----------------------------------------------------------------------
    // 3. Paging (identity map first 4 GB + framebuffer)
    // -----------------------------------------------------------------------
    arch::paging::init(boot_info);
    drivers::serial::write_str("paging ok\n");
    bench::stamp(bench::Phase::PagingDone);

    // Point BootInfo font fields at our static buffer now that paging is up
    if FONT_LEN > 0 {
        (*boot_info.cast_mut()).font_base = FONT_BUF.as_ptr() as u64;
        (*boot_info.cast_mut()).font_size = FONT_LEN as u64;
    }

    // -----------------------------------------------------------------------
    // 4. Kernel subsystems
    // -----------------------------------------------------------------------
    *fs::FILESYSTEM.lock() = Some(fs::RamFS::new());

    // Load saved ELF bytes into RamFS now that heap is available
    if ELF_LEN > 0 {
        serial_fmt!(
            "ELF_BUF addr={:#x} magic at copy time: {:x} {:x} {:x} {:x}\n",
            ELF_BUF.as_ptr() as usize,
            ELF_BUF[0],
            ELF_BUF[1],
            ELF_BUF[2],
            ELF_BUF[3]
        );
        let name: &[char] = &['t', 'e', 's', 't'];
        let mut fs_guard = fs::FILESYSTEM.lock();
        if let Some(ref mut fs) = *fs_guard {
            if fs.create(name) {
                if let Some(f) = fs.find_mut(name) {
                    f.data.extend_from_slice(&ELF_BUF[..ELF_LEN]);
                    serial_fmt!(
                        "user ELF loaded: {} bytes, magic: {:x} {:x} {:x} {:x}\n",
                        f.data.len(),
                        f.data[0],
                        f.data[1],
                        f.data[2],
                        f.data[3]
                    );
                }
            }
        }
    }

    drivers::vga::init_global(boot_info);

    arch::gdt::init();
    drivers::serial::write_str("gdt ok\n");
    bench::stamp(bench::Phase::GdtDone);

    extern "C" {
        static __stack_bottom: u8;
    }
    arch::gdt::TSS.rsp0 = &raw const __stack_top as u64;
    arch::gdt::TSS_RSP0 = &raw const __stack_top as u64;

    arch::idt::init();
    drivers::serial::write_str("idt ok\n");
    bench::stamp(bench::Phase::IdtDone);

    syscall::init();
    drivers::serial::write_str("syscall gate ok\n");
    bench::stamp(bench::Phase::SyscallDone);

    hw::pit::init();
    drivers::serial::write_str("pit ok\n");
    bench::stamp(bench::Phase::PitDone);

    proc::task::init_main_task(&raw const __stack_bottom as usize);

    core::arch::asm!("sti");
    drivers::serial::write_str("sti ok\n");

    // -----------------------------------------------------------------------
    // 5. Heap smoke-test
    // -----------------------------------------------------------------------
    {
        use alloc::vec::Vec;
        let mut v: Vec<u64> = Vec::new();
        v.push(42);
        v.push(1337);
        serial_fmt!("heap smoke-test: {:?} {:?}\n", v[0], v[1]);
    }

    bench::stamp(bench::Phase::HeapDone);

    // -----------------------------------------------------------------------
    // 6. Register demo background task
    // -----------------------------------------------------------------------
    proc::task::add_task(demo_task);
    drivers::serial::write_str("task added\n");
    bench::stamp(bench::Phase::RamfsDone);

    // -----------------------------------------------------------------------
    // 7. Shell UI
    // -----------------------------------------------------------------------
    shell::ui::run_shell(boot_info);
}
