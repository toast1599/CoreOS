// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

#![feature(alloc_error_handler)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod bench;
mod boot;
mod debug;
mod elf;
mod exec;
mod fs;
mod gdt;
mod heap;
mod hw;
mod idt;
mod paging;
mod pmm;
mod process;
mod scheduler;
mod serial;
mod shell;
mod syscall;
mod task;
mod vga;

use crate::heap::SlabAllocator;
use core::fmt::Write;
use core::panic::PanicInfo;
use vga::Console;

pub mod main_fs {
    pub static mut FILESYSTEM: Option<crate::fs::RamFS> = None;
}

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
// Interrupt handlers (called from IDT stubs in idt.rs)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn keyboard_handler() {
    unsafe {
        hw::pic::eoi(1);
        let scancode = hw::ps2::read_data();
        crate::serial_fmt!("[KBD] scancode={:#x}\n", scancode);
        let c = hw::ps2::scancode_to_char(scancode);
        if c != '\0' {
            hw::kbd_buffer::KEYBUF.push(c);
        }
    }
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
pub unsafe extern "win64" fn _start(boot_info: *const boot::CoreOS_BootInfo) -> ! {
    // -----------------------------------------------------------------------
    // 0. Zero BSS
    // -----------------------------------------------------------------------
    extern "C" {
        static mut __bss_start: u8;
        static mut __bss_end: u8;
    }
    let bss_start = &raw mut __bss_start as *mut u8;
    let bss_end = &raw mut __bss_end as *mut u8;
    core::ptr::write_bytes(bss_start, 0, bss_end as usize - bss_start as usize);

    serial::write_str("kernel start\n");
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
    serial::write_str("stack ok\n");

    // -----------------------------------------------------------------------
    // 2. Physical memory manager
    // -----------------------------------------------------------------------
    let kernel_end = &raw const __stack_top as usize + 0x200000; // stack top + 2 MB margin
    pmm::init(boot_info, kernel_end);
    serial::write_str("pmm ok\n");
    bench::stamp(bench::Phase::PmmDone);

    // -----------------------------------------------------------------------
    // 2b. Save user ELF bytes to a static buffer before anything can
    //     overwrite 0x200000 (paging, heap, etc.)
    // -----------------------------------------------------------------------
    static mut ELF_BUF: [u8; 64 * 1024] = [0u8; 64 * 1024]; // 64 KB max
    static mut ELF_LEN: usize = 0;
    static mut FONT_BUF: [u8; 16 * 1024] = [0u8; 16 * 1024]; // 16 KB max
    static mut FONT_LEN: usize = 0;
    {
        let font_base = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).font_base));
        let font_size =
            core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).font_size)) as usize;

        if font_base != 0 && font_size > 0 && font_size <= FONT_BUF.len() {
            let src = core::slice::from_raw_parts(font_base as *const u8, font_size);
            FONT_BUF[..font_size].copy_from_slice(src);
            FONT_LEN = font_size;
            serial::write_str("font loaded into static buffer\n");
        }
    }
    {
        let elf_base = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).user_elf_base));
        let elf_size =
            core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).user_elf_size)) as usize;

        serial_fmt!("elf_base={:#x} elf_size={}\n", elf_base, elf_size);

        if elf_base != 0 && elf_size > 0 && elf_size <= ELF_BUF.len() {
            let src = core::slice::from_raw_parts(elf_base as *const u8, elf_size);
            serial_fmt!(
                "ELF magic: {:x} {:x} {:x} {:x}\n",
                src[0],
                src[1],
                src[2],
                src[3]
            );
            ELF_BUF[..elf_size].copy_from_slice(src);
            ELF_LEN = elf_size;
            serial::write_str("elf bytes saved to static buffer\n");
        }
    }

    // -----------------------------------------------------------------------
    // 3. Paging (identity map first 4 GB + framebuffer)
    // -----------------------------------------------------------------------
    paging::init(boot_info);
    serial::write_str("paging ok\n");
    bench::stamp(bench::Phase::PagingDone);

    // Point BootInfo font fields at our static buffer now that paging is up
    if FONT_LEN > 0 {
        (*boot_info.cast_mut()).font_base = FONT_BUF.as_ptr() as u64;
        (*boot_info.cast_mut()).font_size = FONT_LEN as u64;
    }

    // -----------------------------------------------------------------------
    // 4. Kernel subsystems
    // -----------------------------------------------------------------------
    main_fs::FILESYSTEM = Some(fs::RamFS::new());

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
        if let Some(ref mut fs) = main_fs::FILESYSTEM {
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

    vga::init_global(boot_info);

    gdt::init();
    serial::write_str("gdt ok\n");
    bench::stamp(bench::Phase::GdtDone);

    extern "C" {
        static __stack_bottom: u8;
    }
    gdt::TSS.rsp0 = &raw const __stack_top as u64;
    gdt::TSS_RSP0 = &raw const __stack_top as u64;

    idt::init();
    serial::write_str("idt ok\n");
    bench::stamp(bench::Phase::IdtDone);

    syscall::init();
    serial::write_str("syscall gate ok\n");
    bench::stamp(bench::Phase::SyscallDone);

    hw::pit::init();
    serial::write_str("pit ok\n");
    bench::stamp(bench::Phase::PitDone);

    task::init_main_task(&raw const __stack_bottom as usize);

    core::arch::asm!("sti");
    serial::write_str("sti ok\n");

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
    task::add_task(demo_task);
    serial::write_str("task added\n");
    bench::stamp(bench::Phase::RamfsDone);
    // -----------------------------------------------------------------------
    // 7. Shell UI
    // -----------------------------------------------------------------------
    run_shell(boot_info);
}

// ---------------------------------------------------------------------------
// Shell UI loop (extracted from _start for readability)
// ---------------------------------------------------------------------------

unsafe fn run_shell(boot_info: *const boot::CoreOS_BootInfo) -> ! {
    let width = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).width)) as usize;
    let height = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).height)) as usize;

    // Clear screen
    vga::draw_rect(0, 0, width, height, vga::BG_COLOR, boot_info);

    // Header
    let mut header = Console {
        x: 20,
        y: 20,
        color: vga::CLOCK_COLOR,
        scale: 2,
        boot_info,
    };
    let _ = write!(header, "CoreOS Shell v0.01");
    vga::draw_rect(20, 60, 550, 4, vga::CLOCK_COLOR, boot_info);

    serial::write_str("Starting default userspace shell...\n");

    loop {
        let name: &[char] = &['t', 'e', 's', 't'];
        let elf_data = {
            if let Some(ref fs) = main_fs::FILESYSTEM {
                if let Some(f) = fs.find(name) {
                    Some(f.data.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(data) = elf_data {
            let (pid, slot) = crate::exec::exec_as_task(&data);
            if pid > 0 {
                let mut last_s = 255u8;
                // Wait for userspace shell to exit
                while process::is_running_in_slot(slot) {
                    // Update clock
                    let (h, m, s) = hw::rtc::get_time();
                    if s != last_s {
                        let clock_x = width - 150;
                        vga::draw_rect(clock_x, 20, 130, 32, vga::BG_COLOR, boot_info);
                        draw_clock(boot_info, clock_x, 20, h, m, s);
                        last_s = s;
                    }

                    core::arch::asm!("hlt");
                }
                if let Some(code) = process::reap_slot(slot) {
                    serial_fmt!("Userspace shell (pid {}) exited with code {}. Restarting...\n", pid, code);
                }
            } else {
                serial::write_str("Failed to spawn userspace shell. Hitting hlt loop.\n");
                loop { core::arch::asm!("hlt"); }
            }
        } else {
            serial::write_str("test.elf not found in RamFS. Hitting hlt loop.\n");
            loop { core::arch::asm!("hlt"); }
        }
    }
}

/// HH:MM:SS draw helper for the kernel's top bar.
unsafe fn draw_clock(
    boot_info: *const boot::CoreOS_BootInfo,
    x: usize,
    y: usize,
    h: u8,
    m: u8,
    s: u8,
) {
    let mut con = Console {
        x,
        y,
        color: vga::CLOCK_COLOR,
        scale: 2,
        boot_info,
    };

    let write_two = |con: &mut Console, n: u8| {
        let _ = write!(con, "{:02}", n);
    };

    write_two(&mut con, h);
    let _ = write!(con, ":");
    write_two(&mut con, m);
    let _ = write!(con, ":");
    write_two(&mut con, s);
}

// ---------------------------------------------------------------------------
// Default exception handler
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn default_exception(vector: u64) {
    unsafe {
        core::arch::asm!("sti");
        crate::serial_fmt!("[EXCEPTION] fault #{} in task — marking dead\n", vector);
        // Mark process as exited with error code so kernel shell can reap it
        crate::process::exit(vector as i64);
        if let Some(slot) = crate::task::current_task_slot() {
            crate::task::kill_task(slot);
        }
    }
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

// ---------------------------------------------------------------------------
// Panic handler
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        serial::write_str("[PERNEL KANIC] ");
    }
    if let Some(msg) = info.message().as_str() {
        unsafe {
            serial::write_str(msg);
        }
    }
    if let Some(loc) = info.location() {
        crate::serial_fmt!(" @ {}:{}\n", loc.file(), loc.line());
    } else {
        unsafe {
            serial::write_str("\n");
        }
    }
    unsafe {
        // Disable interrupts and shut down via QEMU magic port.
        // On real hardware this falls through to the hlt loop safely.
        core::arch::asm!("cli");
        core::arch::asm!(
            "out dx, ax",
            in("dx") 0x604u16,
            in("ax") 0x2000u16,
            options(nostack, nomem)
        );
        loop {
            core::arch::asm!("hlt");
        }
    }
}
