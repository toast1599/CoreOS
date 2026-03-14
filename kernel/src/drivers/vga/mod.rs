#![allow(dead_code)]

pub mod console;

use crate::boot::CoreOS_BootInfo;
use core::ptr::addr_of;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const BG_COLOR: u32 = 0x00191724; // Base Midnight
pub const TEXT_COLOR: u32 = 0x00E0DEF4; // Text Lavender
pub const CLOCK_COLOR: u32 = 0x00EBBCBA; // Rose Pink

// ---------------------------------------------------------------------------
// Global framebuffer state — set once at boot, used by syscall write()
// ---------------------------------------------------------------------------

pub(super) static FB_BASE: AtomicUsize = AtomicUsize::new(0);
pub(super) static FB_PITCH: AtomicUsize = AtomicUsize::new(0);
pub(super) static FB_WIDTH: AtomicUsize = AtomicUsize::new(0);
pub(super) static FB_HEIGHT: AtomicUsize = AtomicUsize::new(0);
pub(super) static FONT_BASE: AtomicUsize = AtomicUsize::new(0);

/// Call once during kernel init after paging and font are set up.
pub fn init_global(boot_info: *const CoreOS_BootInfo) {
    unsafe {
        let fb_base = core::ptr::read_unaligned(addr_of!((*boot_info).fb_base)) as usize;
        let fb_pitch = core::ptr::read_unaligned(addr_of!((*boot_info).pitch)) as usize;
        let fb_w = core::ptr::read_unaligned(addr_of!((*boot_info).width)) as usize;
        let fb_h = core::ptr::read_unaligned(addr_of!((*boot_info).height)) as usize;
        let font_b = core::ptr::read_unaligned(addr_of!((*boot_info).font_base)) as usize;
        FB_BASE.store(fb_base, Ordering::Relaxed);
        FB_PITCH.store(fb_pitch, Ordering::Relaxed);
        FB_WIDTH.store(fb_w, Ordering::Relaxed);
        FB_HEIGHT.store(fb_h, Ordering::Relaxed);
        FONT_BASE.store(font_b, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Internal raw glyph renderer (no boot_info pointer needed)
// ---------------------------------------------------------------------------

pub(super) unsafe fn putchar_raw(
    c: char,
    x: usize,
    y: usize,
    color: u32,
    scale: usize,
    fb_base: usize,
    fb_pitch: usize,
    font_base: usize,
) {
    if fb_base == 0 || font_base == 0 {
        return;
    }
    let font_ptr = font_base as *const u8;
    let header_size = core::ptr::read_unaligned(font_ptr.add(8) as *const u32) as usize;
    let bytes_per_glyph = core::ptr::read_unaligned(font_ptr.add(20) as *const u32) as usize;
    let font_height = core::ptr::read_unaligned(font_ptr.add(24) as *const u32) as usize;
    let num_glyphs = core::ptr::read_unaligned(font_ptr.add(16) as *const u32) as usize;

    if font_height == 0
        || font_height > 32
        || bytes_per_glyph == 0
        || bytes_per_glyph > 256
        || num_glyphs == 0
    {
        return;
    }

    let fb_w = FB_WIDTH.load(Ordering::Relaxed);
    let fb_h = FB_HEIGHT.load(Ordering::Relaxed);

    // Don't even attempt to draw if the character origin is off-screen
    if x >= fb_w || y >= fb_h {
        return;
    }

    let glyph_idx = (c as usize).min(num_glyphs - 1);
    let glyph = font_ptr.add(header_size + glyph_idx * bytes_per_glyph);
    let base = fb_base as *mut u32;

    for row in 0..font_height {
        let py = y + row * scale;
        if py >= fb_h {
            break;
        } // ← stop if we've gone off the bottom
        let bitmask = *glyph.add(row);
        for bit in 0..8usize {
            if (bitmask & (0x80 >> bit)) != 0 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + bit * scale + sx;
                        let py2 = py + sy;
                        if px >= fb_w || py2 >= fb_h {
                            continue;
                        } // ← clamp
                        base.add(py2 * fb_pitch + px).write_volatile(color);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Existing draw helpers (unchanged)
// ---------------------------------------------------------------------------

pub unsafe fn draw_rect(
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
    _boot_info: *const CoreOS_BootInfo,
) {
    let fb_base = FB_BASE.load(Ordering::Relaxed);
    let fb_pitch = FB_PITCH.load(Ordering::Relaxed);
    if fb_base == 0 {
        return;
    }
    draw_rect_raw(x, y, w, h, color, fb_base, fb_pitch);
}

pub unsafe fn draw_rect_raw(
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
    fb_base: usize,
    fb_pitch: usize,
) {
    let fb_w = FB_WIDTH.load(Ordering::Relaxed);
    let fb_h = FB_HEIGHT.load(Ordering::Relaxed);
    if fb_base == 0 || fb_w == 0 || fb_h == 0 {
        return;
    }

    let base = fb_base as *mut u32;
    for dy in 0..h {
        let py = y + dy;
        if py >= fb_h {
            break;
        }
        let row_ptr = base.add(py * fb_pitch);
        for dx in 0..w {
            let px = x + dx;
            if px >= fb_w {
                break;
            }
            row_ptr.add(px).write_volatile(color);
        }
    }
}

pub unsafe fn putchar(
    c: char,
    x: usize,
    y: usize,
    color: u32,
    scale: usize,
    _boot_info: *const CoreOS_BootInfo,
) {
    let font_base = FONT_BASE.load(Ordering::Relaxed);
    let fb_base = FB_BASE.load(Ordering::Relaxed);
    let fb_pitch = FB_PITCH.load(Ordering::Relaxed);
    putchar_raw(c, x, y, color, scale, fb_base, fb_pitch, font_base);
}

pub unsafe fn clear_from(start_y: usize, _boot_info: *const CoreOS_BootInfo) {
    let fb_base = FB_BASE.load(Ordering::Relaxed);
    let fb_pitch = FB_PITCH.load(Ordering::Relaxed);
    let fb_w = FB_WIDTH.load(Ordering::Relaxed);
    let fb_h = FB_HEIGHT.load(Ordering::Relaxed);
    if fb_base == 0 || fb_w == 0 || fb_h <= start_y {
        return;
    }
    draw_rect_raw(
        0,
        start_y,
        fb_w,
        fb_h - start_y,
        BG_COLOR,
        fb_base,
        fb_pitch,
    );
}

pub unsafe fn clear_line(y: usize, scale: usize, _boot_info: *const CoreOS_BootInfo) {
    let fb_base = FB_BASE.load(Ordering::Relaxed);
    let fb_pitch = FB_PITCH.load(Ordering::Relaxed);
    let fb_w = FB_WIDTH.load(Ordering::Relaxed);
    if fb_base == 0 || fb_w == 0 {
        return;
    }
    draw_rect_raw(0, y, fb_w, 16 * scale, BG_COLOR, fb_base, fb_pitch);
}
