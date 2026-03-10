#![feature(alloc_error_handler)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod boot;
mod debug;
mod fs;
mod gdt;
mod heap;
mod hw;
mod idt;
mod paging;
mod pmm;
mod scheduler;
mod serial;
mod task;
mod vga;

use crate::heap::SlabAllocator;
use core::fmt::Write;
use core::panic::PanicInfo;
use vga::Console;

#[global_allocator]
static ALLOCATOR: SlabAllocator = SlabAllocator;

use core::alloc::Layout;

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    crate::serial_fmt!(
        "[ALLOC ERROR] size={} align={}\n",
        layout.size(),
        layout.align()
    );
    loop {}
}

static mut FILESYSTEM: Option<fs::RamFS> = None;

#[no_mangle]
pub extern "C" fn keyboard_handler() {
    unsafe {
        let scancode = hw::ps2::read_data();
        let c = hw::ps2::scancode_to_char(scancode);
        if c != '\0' {
            hw::kbd_buffer::KEYBUF.push(c);
        }
    }
}

fn get_arg_chars(buffer: &[char; 64], cmd_len: usize) -> &[char] {
    let start = cmd_len + 1;
    if start >= 64 || buffer[start] == '\0' {
        return &[];
    }
    let mut end = start;
    while end < 64 && buffer[end] != '\0' && buffer[end] != ' ' {
        end += 1;
    }
    &buffer[start..end]
}

fn command_is(buffer: &[char; 64], cmd: &str) -> bool {
    let mut i = 0;
    for c in cmd.chars() {
        if buffer[i] != c {
            return false;
        }
        i += 1;
    }
    buffer[i] == '\0' || buffer[i] == ' '
}

fn demo_task() {
    loop {
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

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
    // 2. Physical Memory Manager
    //    kernel_end = rough upper bound: stack top symbol
    // -----------------------------------------------------------------------
    extern "C" {
        static __stack_bottom: u8;
    } // bottom of our static stack
    let kernel_end = &raw const __stack_top as usize + 0x200000; // stack top + 2 MB margin

    pmm::init(boot_info, kernel_end);
    serial::write_str("pmm ok\n");

    // -----------------------------------------------------------------------
    // 3. Paging (identity map)
    // -----------------------------------------------------------------------
    paging::init(boot_info);
    serial::write_str("paging ok\n");

    // -----------------------------------------------------------------------
    // 4. Kernel subsystems
    // -----------------------------------------------------------------------
    FILESYSTEM = Some(fs::RamFS::new());
    serial::write_str("fs ok\n");

    gdt::init();
    serial::write_str("gdt ok\n");

    // Point TSS.rsp0 at our kernel stack top so interrupts from ring 3
    // land on the right stack (needed once we add user mode)
    gdt::TSS.rsp0 = &raw const __stack_top as u64;

    idt::init_idt();
    serial::write_str("idt ok\n");

    hw::pit::init_pit();
    serial::write_str("pit ok\n");

    // Register the main loop as task 0 before enabling interrupts
    task::init_main_task(&raw const __stack_bottom as usize);

    core::arch::asm!("sti");
    serial::write_str("sti ok\n");

    // -----------------------------------------------------------------------
    // 5. Quick heap smoke-test
    // -----------------------------------------------------------------------
    {
        use alloc::vec::Vec;
        let mut v: Vec<u64> = Vec::new();
        v.push(42);
        v.push(1337);
        serial_fmt!("heap smoke-test: {:?} {:?}\n", v[0], v[1]);
    }

    // -----------------------------------------------------------------------
    // 6. Add demo task
    // -----------------------------------------------------------------------
    task::add_task(demo_task);
    serial::write_str("task added\n");

    // -----------------------------------------------------------------------
    // 7. Draw shell UI
    // -----------------------------------------------------------------------
    let width = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).width)) as usize;
    let height = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).height)) as usize;

    vga::draw_rect(0, 0, width, height, vga::BG_COLOR, boot_info);

    let mut global_scale: usize = 1;
    let mut current_y = 120;

    let mut header = Console {
        x: 20,
        y: 20,
        color: vga::CLOCK_COLOR,
        scale: 2,
        boot_info,
    };
    let _ = write!(header, "CoreOS Shell v0.02");
    vga::draw_rect(20, 60, 550, 4, vga::CLOCK_COLOR, boot_info);

    let mut shell = Shell::new();

    let mut line = Console {
        x: 20,
        y: current_y,
        color: vga::TEXT_COLOR,
        scale: global_scale,
        boot_info,
    };
    let _ = write!(line, "> ");

    // -----------------------------------------------------------------------
    // 8. Main loop
    // -----------------------------------------------------------------------
    loop {
        let (h, m, s) = hw::rtc::get_time();
        let clock_x = width - 150;

        vga::draw_rect(clock_x, 20, 130, 32, vga::BG_COLOR, boot_info);

        let mut clock = Console {
            x: clock_x,
            y: 20,
            color: vga::CLOCK_COLOR,
            scale: 2,
            boot_info,
        };
        unsafe {
            unsafe fn d(con: &mut Console, n: u8) {
                vga::putchar(
                    ('0' as u8 + n / 10) as char,
                    con.x,
                    con.y,
                    con.color,
                    con.scale,
                    con.boot_info,
                );
                con.x += 8 * con.scale;
                vga::putchar(
                    ('0' as u8 + n % 10) as char,
                    con.x,
                    con.y,
                    con.color,
                    con.scale,
                    con.boot_info,
                );
                con.x += 8 * con.scale;
            }
            unsafe fn colon(con: &mut Console) {
                vga::putchar(':', con.x, con.y, con.color, con.scale, con.boot_info);
                con.x += 8 * con.scale;
            }
            d(&mut clock, h);
            colon(&mut clock);
            d(&mut clock, m);
            colon(&mut clock);
            d(&mut clock, s);
        }

        let key = {
            core::arch::asm!("cli", options(nostack, nomem));
            let k = hw::kbd_buffer::KEYBUF.pop();
            core::arch::asm!("sti", options(nostack, nomem));
            k
        };

        if let Some(c) = key {
            match c {
                '\x08' => {
                    shell.pop();
                    vga::clear_line(current_y, global_scale, boot_info);
                    let mut redraw = Console {
                        x: 20,
                        y: current_y,
                        color: vga::TEXT_COLOR,
                        scale: global_scale,
                        boot_info,
                    };
                    let _ = write!(redraw, "> ");
                    for i in 0..shell.cursor {
                        let _ = write!(redraw, "{}", shell.buffer[i]);
                    }
                }

                '\n' => {
                    current_y += 16 * global_scale;
                    if current_y + (16 * global_scale) >= height {
                        vga::clear_from(120, boot_info);
                        current_y = 120;
                    }

                    let mut resp = Console {
                        x: 20,
                        y: current_y,
                        color: vga::TEXT_COLOR,
                        scale: global_scale,
                        boot_info,
                    };

                    core::arch::asm!("cli", options(nostack, nomem));
                    crate::serial::write_str("[SHELL] cmd: [");
                    for i in 0..shell.cursor {
                        let c = shell.buffer[i];
                        if c != '\0' {
                            crate::serial::write_byte(c as u8);
                        }
                    }
                    crate::serial::write_str("]\n");
                    core::arch::asm!("sti", options(nostack, nomem));

                    // ---- commands ----
                    if command_is(&shell.buffer, "clear") {
                        vga::clear_from(120, boot_info);
                        current_y = 120;
                    } else if command_is(&shell.buffer, "meminfo") {
                        let free_mb = pmm::free_bytes() / (1024 * 1024);
                        let _ = write!(resp, "Free physical RAM: {} MB", free_mb);
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "ls") {
                        if let Some(fs) = FILESYSTEM.as_ref() {
                            let _ = write!(resp, "Files in RAM:");
                            resp.y += 16 * global_scale;
                            for f in fs.files.iter() {
                                let _ = write!(resp, " - ");
                                for c in f.name.iter() {
                                    let _ = write!(resp, "{}", c);
                                }
                                let _ = write!(resp, " ({} bytes)", f.data.len());
                                resp.y += 16 * global_scale;
                            }
                            current_y = resp.y;
                        }
                    } else if command_is(&shell.buffer, "touch") {
                        let filename = get_arg_chars(&shell.buffer, 5);
                        if let Some(fs) = FILESYSTEM.as_mut() {
                            if fs.create(filename) {
                                let _ = write!(resp, "File created.");
                            } else {
                                let _ = write!(resp, "Error: file exists or invalid.");
                            }
                        }
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "uptime") {
                        let _ = write!(resp, "Uptime: {} seconds", hw::pit::uptime_seconds());
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "ticks") {
                        let _ = write!(resp, "Kernel ticks: {}", hw::pit::ticks());
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "sleep") {
                        let arg = get_arg_chars(&shell.buffer, 5);
                        let mut n: u64 = 0;
                        for c in arg {
                            if *c >= '0' && *c <= '9' {
                                n = n * 10 + (*c as u64 - '0' as u64);
                            }
                        }
                        hw::pit::sleep(n * 100);
                    } else if command_is(&shell.buffer, "write") {
                        let filename = get_arg_chars(&shell.buffer, 5);
                        if let Some(fs) = FILESYSTEM.as_mut() {
                            if let Some(file) = fs.find_mut(filename) {
                                file.data.clear();
                                let mut i = 6 + filename.len();
                                while i < 64 && shell.buffer[i] != '\0' {
                                    file.data.push(shell.buffer[i] as u8);
                                    i += 1;
                                }
                                let _ = write!(resp, "Overwritten.");
                            } else {
                                let _ = write!(resp, "Error: file not found.");
                            }
                        }
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "push") {
                        let filename = get_arg_chars(&shell.buffer, 4);
                        if let Some(fs) = FILESYSTEM.as_mut() {
                            if let Some(file) = fs.find_mut(filename) {
                                let mut i = 5 + filename.len();
                                while i < 64 && shell.buffer[i] != '\0' {
                                    file.data.push(shell.buffer[i] as u8);
                                    i += 1;
                                }
                                let _ = write!(resp, "Appended.");
                            } else {
                                let _ = write!(resp, "Error: file not found.");
                            }
                        }
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "cat") || command_is(&shell.buffer, "print")
                    {
                        let cmd_len = if command_is(&shell.buffer, "cat") {
                            3
                        } else {
                            5
                        };
                        let filename = get_arg_chars(&shell.buffer, cmd_len);
                        if let Some(fs) = FILESYSTEM.as_ref() {
                            if let Some(file) = fs.find(filename) {
                                for byte in file.data.iter() {
                                    let _ = write!(resp, "{}", *byte as char);
                                }
                            } else {
                                let _ = write!(resp, "Error: file not found.");
                            }
                        }
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "rm") {
                        let filename = get_arg_chars(&shell.buffer, 2);
                        if let Some(fs) = FILESYSTEM.as_mut() {
                            if fs.remove(filename) {
                                let _ = write!(resp, "File removed.");
                            } else {
                                let _ = write!(resp, "Error: file not found.");
                            }
                        }
                        current_y += 16 * global_scale;
                    } else if command_is(&shell.buffer, "font") {
                        if shell.buffer[5] == '+' {
                            if global_scale < 4 {
                                global_scale += 1;
                            }
                        } else if shell.buffer[5] == '-' {
                            if global_scale > 1 {
                                global_scale -= 1;
                            }
                        }
                    } else if command_is(&shell.buffer, "reboot") {
                        hw::reboot();
                    } else if command_is(&shell.buffer, "echo") {
                        let mut i = 5;
                        while i < 64 && shell.buffer[i] != '\0' {
                            let _ = write!(resp, "{}", shell.buffer[i]);
                            i += 1;
                        }
                        current_y += 16 * global_scale;
                    }

                    shell.clear();
                    let mut next_line = Console {
                        x: 20,
                        y: current_y,
                        color: vga::TEXT_COLOR,
                        scale: global_scale,
                        boot_info,
                    };
                    let _ = write!(next_line, "> ");
                }

                _ => {
                    let max_chars = (width - 40) / (8 * global_scale) - 2;
                    if shell.cursor < max_chars && shell.cursor < 63 {
                        shell.push(c);
                        vga::clear_line(current_y, global_scale, boot_info);
                        let mut redraw = Console {
                            x: 20,
                            y: current_y,
                            color: vga::TEXT_COLOR,
                            scale: global_scale,
                            boot_info,
                        };
                        let _ = write!(redraw, "> ");
                        for i in 0..shell.cursor {
                            let _ = write!(redraw, "{}", shell.buffer[i]);
                        }
                        crate::serial_fmt!("tick={}\n", hw::pit::ticks());
                    }
                }
            }
        }
    }
}

struct Shell {
    buffer: [char; 64],
    cursor: usize,
}
impl Shell {
    fn new() -> Self {
        Self {
            buffer: ['\0'; 64],
            cursor: 0,
        }
    }
    fn push(&mut self, c: char) {
        if self.cursor < 63 {
            self.buffer[self.cursor] = c;
            self.cursor += 1;
        }
    }
    fn pop(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer[self.cursor] = '\0';
        }
    }
    fn clear(&mut self) {
        self.cursor = 0;
        for i in 0..64 {
            self.buffer[i] = '\0';
        }
    }
}

extern "C" fn default_exception() {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

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

