#![feature(alloc_error_handler)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod boot;
mod debug;
mod fs;
mod heap;
mod hw;
mod idt;
mod scheduler;
mod serial;
mod task;
mod vga;

use crate::heap::BumpAllocator;
use core::fmt::Write;
use core::panic::PanicInfo;
use vga::Console;

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

use core::alloc::Layout;

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
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
    // Zero BSS first, before anything else
    extern "C" {
        static mut __bss_start: u8;
        static mut __bss_end: u8;
    }
    let bss_start = &raw mut __bss_start as *mut u8;
    let bss_end = &raw mut __bss_end as *mut u8;
    let bss_size = bss_end as usize - bss_start as usize;
    core::ptr::write_bytes(bss_start, 0, bss_size);

    serial::write_str("kernel start\n");

    extern "C" {
        static __stack_top: u8;
    }
    core::arch::asm!(
        "lea rsp, [{stack}]",
        stack = sym __stack_top,
        options(nostack, nomem)
    );
    serial::write_str("stage1\n");

    FILESYSTEM = Some(fs::RamFS::new());

    serial::write_str("stage2\n");

    idt::init_idt();

    serial::write_str("stage3\n");

    hw::pit::init_pit();

    serial::write_str("stage4\n");

    core::arch::asm!("sti");
    serial::write_str("sti done\n");

    use alloc::vec::Vec;
    let mut v = Vec::new();
    serial::write_str("vec created\n");

    v.push(42);
    serial::write_str("push 42\n");

    v.push(1337);
    serial::write_str("push 1337\n");

    unsafe {
        task::add_task(demo_task);
    }
    serial::write_str("task added\n");

    let width = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).width)) as usize;
    let height = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).height)) as usize;
    serial::write_str("boot info read\n");

    vga::draw_rect(0, 0, width, height, vga::BG_COLOR, boot_info);
    serial::write_str("screen cleared\n");

    let mut global_scale: usize = 1;
    let mut current_y = 120;

    let mut header = Console {
        x: 20,
        y: 20,
        color: vga::CLOCK_COLOR,
        scale: 2,
        boot_info,
    };
    let _ = write!(header, "CoreOS Shell v0.01");
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

    loop {
        let (h, m, s) = hw::rtc::get_time();

        let clock_x = width - 150;
        let pitch =
            unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).pitch)) as usize };

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

        let key = unsafe {
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

                    unsafe {
                        core::arch::asm!("cli", options(nostack, nomem));
                    }
                    crate::serial::write_str("[SHELL] cmd: [");
                    for i in 0..shell.cursor {
                        let c = shell.buffer[i];
                        if c != '\0' {
                            unsafe {
                                crate::serial::write_byte(c as u8);
                            }
                        }
                    }
                    crate::serial::write_str("]\n");
                    unsafe {
                        core::arch::asm!("sti", options(nostack, nomem));
                    }

                    // =====================
                    // CLEAR
                    // =====================
                    if command_is(&shell.buffer, "clear") {
                        vga::clear_from(120, boot_info);
                        current_y = 120;
                    }
                    // =====================
                    // LS
                    // =====================
                    else if command_is(&shell.buffer, "ls") {
                        if let Some(fs) = unsafe { FILESYSTEM.as_ref() } {
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
                    }
                    // =====================
                    // TOUCH
                    // =====================
                    else if command_is(&shell.buffer, "touch") {
                        let filename = get_arg_chars(&shell.buffer, 5);

                        if let Some(fs) = unsafe { FILESYSTEM.as_mut() } {
                            if fs.create(filename) {
                                let _ = write!(resp, "File created.");
                            } else {
                                let _ = write!(resp, "Error: file exists or invalid.");
                            }
                        }

                        current_y += 16 * global_scale;
                    }
                    // =====================
                    // UPTIME
                    // =====================
                    else if command_is(&shell.buffer, "uptime") {
                        let seconds = hw::pit::uptime_seconds();
                        let _ = write!(resp, "Uptime: {} seconds", seconds);
                        current_y += 16 * global_scale;
                    }
                    // =====================
                    // TICKS
                    // =====================
                    else if command_is(&shell.buffer, "ticks") {
                        let t = hw::pit::ticks();
                        let _ = write!(resp, "Kernel ticks: {}", t);
                        current_y += 16 * global_scale;
                    }
                    // =====================
                    // SLEEP
                    // =====================
                    else if command_is(&shell.buffer, "sleep") {
                        let arg = get_arg_chars(&shell.buffer, 5);

                        let mut n: u64 = 0;
                        for c in arg {
                            if *c >= '0' && *c <= '9' {
                                n = n * 10 + (*c as u64 - '0' as u64);
                            }
                        }

                        hw::pit::sleep(n * 100); // 100 ticks = 1 second
                    }
                    // =====================
                    // WRITE (overwrite)
                    // =====================
                    else if command_is(&shell.buffer, "write") {
                        let filename = get_arg_chars(&shell.buffer, 5);

                        if let Some(fs) = unsafe { FILESYSTEM.as_mut() } {
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
                    }
                    // =====================
                    // PUSH (append)
                    // =====================
                    else if command_is(&shell.buffer, "push") {
                        let filename = get_arg_chars(&shell.buffer, 4);

                        if let Some(fs) = unsafe { FILESYSTEM.as_mut() } {
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
                    }
                    // =====================
                    // CAT / PRINT
                    // =====================
                    else if command_is(&shell.buffer, "cat") || command_is(&shell.buffer, "print")
                    {
                        let cmd_len = if command_is(&shell.buffer, "cat") {
                            3
                        } else {
                            5
                        };
                        let filename = get_arg_chars(&shell.buffer, cmd_len);

                        if let Some(fs) = unsafe { FILESYSTEM.as_ref() } {
                            if let Some(file) = fs.find(filename) {
                                for byte in file.data.iter() {
                                    let _ = write!(resp, "{}", *byte as char);
                                }
                            } else {
                                let _ = write!(resp, "Error: file not found.");
                            }
                        }

                        current_y += 16 * global_scale;
                    }
                    // =====================
                    // RM
                    // =====================
                    else if command_is(&shell.buffer, "rm") {
                        let filename = get_arg_chars(&shell.buffer, 2);

                        if let Some(fs) = unsafe { FILESYSTEM.as_mut() } {
                            if fs.remove(filename) {
                                let _ = write!(resp, "File removed.");
                            } else {
                                let _ = write!(resp, "Error: file not found.");
                            }
                        }

                        current_y += 16 * global_scale;
                    }
                    // =====================
                    // FONT
                    // =====================
                    else if command_is(&shell.buffer, "font") {
                        if shell.buffer[5] == '+' {
                            if global_scale < 4 {
                                global_scale += 1;
                            }
                        } else if shell.buffer[5] == '-' {
                            if global_scale > 1 {
                                global_scale -= 1;
                            }
                        }
                    }
                    // =====================
                    // REBOOT
                    // =====================
                    else if command_is(&shell.buffer, "reboot") {
                        hw::reboot();
                    }
                    // =====================
                    // ECHO
                    // =====================
                    else if command_is(&shell.buffer, "echo") {
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

                        unsafe {
                            crate::serial_fmt!("tick={}\n", hw::pit::ticks());
                        }
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
        serial::write_str("[PERNEL KANIC - !] ");
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
