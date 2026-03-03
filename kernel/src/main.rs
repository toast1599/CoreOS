#![feature(alloc_error_handler)]

#![no_std]
#![no_main]

extern crate alloc;

mod boot;
mod vga;
mod hw;
mod fs;
mod heap;

use core::fmt::Write;
use core::panic::PanicInfo;
use vga::Console;
use crate::heap::BumpAllocator;

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

use core::alloc::Layout;

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    loop {}
}

#[no_mangle]
static mut STACK: [u8; 1024 * 1024] = [0; 1024 * 1024];

static mut FILESYSTEM: Option<fs::RamFS> = None;

#[no_mangle]
pub extern "C" fn keyboard_handler() {
    unsafe {
        let scancode = hw::ps2::read_data();
        // For now, just ignore it or store it later
        let _ = scancode;
    }
}

fn get_arg_chars(buffer: &[char; 64], cmd_len: usize) -> &[char] {
    let start = cmd_len + 1;
    if start >= 64 || buffer[start] == '\0' { return &[]; }

    let mut end = start;
    while end < 64 && buffer[end] != '\0' && buffer[end] != ' ' {
        end += 1;
    }
    &buffer[start..end]
}

fn command_is(buffer: &[char; 64], cmd: &str) -> bool {
    let mut i = 0;
    for c in cmd.chars() {
        if buffer[i] != c { return false; }
        i += 1;
    }
    buffer[i] == '\0' || buffer[i] == ' '
}

#[export_name = "_start"]
#[link_section = ".text._start"]
pub unsafe extern "win64" fn _start(boot_info: *const boot::CoreOS_BootInfo) -> ! {

    unsafe {
        FILESYSTEM = Some(fs::RamFS::new());
    }

    core::arch::asm!(
        "lea rsp, [{stack} + 16384]",
        stack = sym STACK,
        options(nostack, nomem)
    );

    use alloc::vec::Vec;
    let mut v = Vec::new();
    
    v.push(42);
    v.push(1337);
    
    let width = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).width)) as usize;
    let height = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).height)) as usize;
    vga::draw_rect(0, 0, width, height, vga::BG_COLOR, boot_info);
    let mut global_scale: usize = 1;
    let mut current_y = 120;

    let mut header = Console { x: 20, y: 20, color: vga::CLOCK_COLOR, scale: 2, boot_info };
    let _ = write!(header, "CoreOS Shell v0.1.0");
    vga::draw_rect(20, 60, 550, 4, vga::CLOCK_COLOR, boot_info);

    let mut shell = Shell::new();

    let mut line = Console { x: 20, y: current_y, color: vga::TEXT_COLOR, scale: global_scale, boot_info };
    let _ = write!(line, "> ");

    loop {
        let (h, m, s) = hw::rtc::get_time();

        let clock_x = width - 150;
        vga::draw_rect(clock_x, 20, 130, 32, vga::BG_COLOR, boot_info);

        let mut clock = Console { x: clock_x, y: 20, color: vga::CLOCK_COLOR, scale: 2, boot_info };
        let _ = write!(clock, "{:02}:{:02}:{:02}", h, m, s);

        if (hw::ps2::read_status() & 0x01) != 0 {
            let scancode = hw::ps2::read_data();
            let c = hw::ps2::scancode_to_char(scancode);

            if c != '\0' {
                match c {
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
                        else if command_is(&shell.buffer, "cat") || command_is(&shell.buffer, "print") {
                            let cmd_len = if command_is(&shell.buffer, "cat") { 3 } else { 5 };
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
                        // FONT
                        // =====================
                        else if command_is(&shell.buffer, "font") {
                            if shell.buffer[5] == '+' {
                                if global_scale < 4 { global_scale += 1; }
                            } else if shell.buffer[5] == '-' {
                                if global_scale > 1 { global_scale -= 1; }
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
                        shell.push(c);
                        vga::clear_line(current_y, global_scale, boot_info);
                        let mut redraw = Console { x: 20, y: current_y, color: vga::TEXT_COLOR, scale: global_scale, boot_info };
                        let _ = write!(redraw, "> ");
                        for i in 0..shell.cursor { let _ = write!(redraw, "{}", shell.buffer[i]); }
                    }
                }
            }
        }
        for _ in 0..50_000 { core::arch::asm!("nop"); }
    }
}

struct Shell {
    buffer: [char; 64],
    cursor: usize,
}

impl Shell {
    fn new() -> Self {
        Self { buffer: ['\0'; 64], cursor: 0 }
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
        for i in 0..64 { self.buffer[i] = '\0'; }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop {} }
