use crate::boot;
use crate::fs;
use crate::drivers::vga;
use crate::hw;
use crate::proc;
use core::fmt::Write;

pub unsafe fn run_shell(boot_info: *const boot::CoreOS_BootInfo) -> ! {
    let width = (*boot_info).width as usize;
    let height = (*boot_info).height as usize;

    // Clear screen
    vga::draw_rect(0, 0, width, height, vga::BG_COLOR, boot_info);

    // Header
    let mut header = vga::console::Console {
        x: 20,
        y: 20,
        color: vga::CLOCK_COLOR,
        scale: 2,
        boot_info,
    };
    let _ = write!(header, "CoreOS Shell v0.01");
    vga::draw_rect(20, 60, 550, 4, vga::CLOCK_COLOR, boot_info);

    crate::drivers::serial::write_str("Starting default userspace shell...\n");

    loop {
        let name: &[char] = &['t', 'e', 's', 't'];
        let mut elf_data = None;

        if let Some(ref fs) = fs::FILESYSTEM {
            if let Some(f) = fs.find(name) {
                elf_data = Some(f.data.clone());
            }
        }

        if let Some(data) = elf_data {
            let (pid, slot) = crate::proc::exec::exec_as_task(&data);
            if pid > 0 {
                let mut last_s = 255u8;
                while proc::is_running_in_slot(slot) {
                    let (h, m, s) = hw::rtc::get_time();
                    if s != last_s {
                        let clock_x = width - 150;
                        vga::draw_rect(clock_x, 20, 130, 32, vga::BG_COLOR, boot_info);
                        draw_clock(boot_info, clock_x, 20, h, m, s);
                        last_s = s;
                    }
                    core::arch::asm!("hlt");
                }
                if let Some(code) = proc::reap_slot(slot) {
                    crate::serial_fmt!(
                        "Userspace shell (pid {}) exited with code {}. Restarting...\n",
                        pid,
                        code
                    );
                }
            } else {
                crate::drivers::serial::write_str("Failed to spawn userspace shell. Hitting hlt loop.\n");
                loop {
                    core::arch::asm!("hlt");
                }
            }
        } else {
            crate::drivers::serial::write_str("test.elf not found in RamFS. Hitting hlt loop.\n");
            loop {
                core::arch::asm!("hlt");
            }
        }
    }
}

/// HH:MM:SS draw helper for the kernel's top bar.
pub unsafe fn draw_clock(
    boot_info: *const boot::CoreOS_BootInfo,
    x: usize,
    y: usize,
    h: u8,
    m: u8,
    s: u8,
) {
    let mut con = vga::console::Console {
        x,
        y,
        color: vga::CLOCK_COLOR,
        scale: 2,
        boot_info,
    };

    let write_two = |con: &mut vga::console::Console, n: u8| {
        let _ = write!(con, "{:02}", n);
    };

    write_two(&mut con, h);
    let _ = write!(con, ":");
    write_two(&mut con, m);
    let _ = write!(con, ":");
    write_two(&mut con, s);
}
