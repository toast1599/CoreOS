use crate::boot::{CoreOS_BootInfo, FONT};
use core::fmt::Write;
use core::ptr::addr_of;

pub const BG_COLOR: u32 = 0x00282828; // Dark hard-charcoal
pub const TEXT_COLOR: u32 = 0x00EBDBB2; // Retro cream/bone
pub const CLOCK_COLOR: u32 = 0x00FABD2F; // Industrial yellow-gold

pub struct Console {
    pub x: usize,
    pub y: usize,
    pub color: u32,
    pub scale: usize,
    pub boot_info: *const CoreOS_BootInfo,
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            if c == '\n' {
                self.x = 20;
                self.y += 16 * self.scale;
            } else {
                unsafe {
                    putchar(c, self.x, self.y, self.color, self.scale, self.boot_info);
                }
                self.x += 8 * self.scale;
            }
        }
        Ok(())
    }
}

pub unsafe fn draw_rect(
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
    boot_info: *const CoreOS_BootInfo,
) {
    let base = core::ptr::read_unaligned(addr_of!((*boot_info).fb_base)) as *mut u32;
    let pitch = core::ptr::read_unaligned(addr_of!((*boot_info).pitch)) as usize;
    for dy in 0..h {
        for dx in 0..w {
            base.add((y + dy) * pitch + (x + dx)).write_volatile(color); // ← this line was missing
        }
    }
}

pub unsafe fn putchar(
    c: char,
    x: usize,
    y: usize,
    color: u32,
    scale: usize,
    boot_info: *const CoreOS_BootInfo,
) {
    let font_ptr = FONT.as_ptr();

    let header_size = core::ptr::read_unaligned(font_ptr.add(8) as *const u32) as usize;
    let bytes_per_glyph = core::ptr::read_unaligned(font_ptr.add(20) as *const u32) as usize;
    let font_height = core::ptr::read_unaligned(font_ptr.add(24) as *const u32) as usize;

    let glyph = font_ptr.add(header_size + (c as usize) * bytes_per_glyph);

    let base = core::ptr::read_unaligned(addr_of!((*boot_info).fb_base)) as *mut u32;
    let pitch = core::ptr::read_unaligned(addr_of!((*boot_info).pitch)) as usize;

    for row in 0..font_height {
        let bitmask = *glyph.add(row);
        for bit in 0..8 {
            if (bitmask & (0x80 >> bit)) != 0 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + (bit * scale) + sx;
                        let py = y + (row * scale) + sy;
                        base.add(py * pitch + px).write_volatile(color);
                    }
                }
            }
        }
    }
}

pub unsafe fn clear_from(start_y: usize, boot_info: *const CoreOS_BootInfo) {
    let width = core::ptr::read_unaligned(addr_of!((*boot_info).width)) as usize;
    let height = core::ptr::read_unaligned(addr_of!((*boot_info).height)) as usize;

    draw_rect(0, start_y, width, height - start_y, BG_COLOR, boot_info);
}

pub unsafe fn clear_line(y: usize, scale: usize, boot_info: *const CoreOS_BootInfo) {
    let width = core::ptr::read_unaligned(addr_of!((*boot_info).width)) as usize;

    draw_rect(0, y, width, 16 * scale, BG_COLOR, boot_info);
}
