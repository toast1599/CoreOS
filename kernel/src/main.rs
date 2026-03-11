// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

#![feature(alloc_error_handler)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod boot;
mod debug;
mod elf;
mod fs;
mod gdt;
mod heap;
mod hw;
mod idt;
mod paging;
mod pmm;
mod scheduler;
mod serial;
mod shell;
mod syscall;
mod task;
mod vga;

use crate::heap::SlabAllocator;
use crate::shell::{
    commands::{ShellContext, ShellOutput},
    Shell,
};
use core::fmt::Write;
use core::panic::PanicInfo;
use vga::Console;

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
// Kernel-global state
// ---------------------------------------------------------------------------

static mut FILESYSTEM: Option<fs::RamFS> = None;

// ---------------------------------------------------------------------------
// Interrupt handlers (called from IDT stubs in idt.rs)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn keyboard_handler() {
    unsafe {
        // EOI before processing so we don't block subsequent interrupts
        hw::pic::eoi(1);
        let scancode = hw::ps2::read_data();
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

    // -----------------------------------------------------------------------
    // 2b. Save user ELF bytes to a static buffer before anything can
    //     overwrite 0x200000 (paging, heap, etc.)
    // -----------------------------------------------------------------------
    static mut ELF_BUF: [u8; 64 * 1024] = [0u8; 64 * 1024]; // 64 KB max
    static mut ELF_LEN: usize = 0;

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

    // -----------------------------------------------------------------------
    // 4. Kernel subsystems
    // -----------------------------------------------------------------------
    FILESYSTEM = Some(fs::RamFS::new());

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
        if let Some(ref mut fs) = FILESYSTEM {
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

    gdt::init();
    serial::write_str("gdt ok\n");

    extern "C" {
        static __stack_bottom: u8;
    }
    gdt::TSS.rsp0 = &raw const __stack_top as u64;
    gdt::TSS_RSP0 = &raw const __stack_top as u64;

    idt::init();
    serial::write_str("idt ok\n");

    syscall::init();
    serial::write_str("syscall gate ok\n");

    hw::pit::init();
    serial::write_str("pit ok\n");

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

    // -----------------------------------------------------------------------
    // 6. Register demo background task
    // -----------------------------------------------------------------------
    task::add_task(demo_task);
    serial::write_str("task added\n");

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

    let mut global_scale: usize = 1;
    let mut current_y: usize = 120;

    let mut shell = Shell::new();

    // Initial prompt
    let mut line = Console {
        x: 20,
        y: current_y,
        color: vga::TEXT_COLOR,
        scale: global_scale,
        boot_info,
    };
    let _ = write!(line, "> ");

    loop {
        // ── Clock ──────────────────────────────────────────────────────────
        let (h, m, s) = hw::rtc::get_time();
        let clock_x = width - 150;
        vga::draw_rect(clock_x, 20, 130, 32, vga::BG_COLOR, boot_info);
        draw_clock(boot_info, clock_x, 20, h, m, s);

        // ── Key input ──────────────────────────────────────────────────────
        core::arch::asm!("cli", options(nostack, nomem));
        let key = hw::kbd_buffer::KEYBUF.pop();
        core::arch::asm!("sti", options(nostack, nomem));

        if let Some(c) = key {
            match c {
                // Backspace
                '\x08' => {
                    shell.pop();
                    redraw_prompt(boot_info, current_y, global_scale, &shell);
                }

                // Enter — execute command
                '\n' => {
                    current_y += 16 * global_scale;
                    if current_y + 16 * global_scale >= height {
                        vga::clear_from(120, boot_info);
                        current_y = 120;
                    }

                    let mut ctx = ShellContext {
                        boot_info,
                        filesystem: &mut FILESYSTEM,
                        global_scale: &mut global_scale,
                        current_y: &mut current_y,
                        screen_h: height,
                    };

                    let output = shell.execute(&mut ctx);
                    current_y = *ctx.current_y; // commands may update current_y

                    match output {
                        ShellOutput::Clear => {
                            vga::clear_from(120, boot_info);
                            current_y = 120;
                        }
                        ShellOutput::Print(s) => {
                            let mut resp = Console {
                                x: 20,
                                y: current_y,
                                color: vga::TEXT_COLOR,
                                scale: global_scale,
                                boot_info,
                            };
                            let _ = write!(resp, "{}", s);
                            current_y += 16 * global_scale;
                        }
                        ShellOutput::PrintLines(lines) => {
                            for line_str in lines {
                                let mut resp = Console {
                                    x: 20,
                                    y: current_y,
                                    color: vga::TEXT_COLOR,
                                    scale: global_scale,
                                    boot_info,
                                };
                                let _ = write!(resp, "{}", line_str);
                                current_y += 16 * global_scale;
                            }
                        }
                        ShellOutput::None => {}
                    }

                    // Next prompt
                    let mut next = Console {
                        x: 20,
                        y: current_y,
                        color: vga::TEXT_COLOR,
                        scale: global_scale,
                        boot_info,
                    };
                    let _ = write!(next, "> ");
                }

                // Printable character
                _ => {
                    let max_chars = (width - 40) / (8 * global_scale) - 2;
                    if shell.cursor < max_chars && shell.cursor < 63 {
                        shell.push(c);
                        redraw_prompt(boot_info, current_y, global_scale, &shell);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

/// Redraw the current input line: "> <buffer contents>".
unsafe fn redraw_prompt(
    boot_info: *const boot::CoreOS_BootInfo,
    y: usize,
    scale: usize,
    shell: &Shell,
) {
    vga::clear_line(y, scale, boot_info);
    let mut con = Console {
        x: 20,
        y,
        color: vga::TEXT_COLOR,
        scale,
        boot_info,
    };
    let _ = write!(con, "> ");
    for i in 0..shell.cursor {
        let _ = write!(con, "{}", shell.buffer[i]);
    }
}

/// Draw HH:MM:SS at (x, y) using the clock colour.
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

pub extern "C" fn default_exception() {
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
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
