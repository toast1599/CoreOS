#![allow(dead_code)]

use crate::boot::CoreOS_BootInfo;
use core::fmt::Write;
use core::ptr::addr_of;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const BG_COLOR: u32 = 0x00191724; // Base Midnight
pub const TEXT_COLOR: u32 = 0x00E0DEF4; // Text Lavender
pub const CLOCK_COLOR: u32 = 0x00EBBCBA; // Rose Pink

// ---------------------------------------------------------------------------
// Global framebuffer state — set once at boot, used by syscall write()
// ---------------------------------------------------------------------------

static FB_BASE: AtomicUsize = AtomicUsize::new(0);
static FB_PITCH: AtomicUsize = AtomicUsize::new(0);
static FB_WIDTH: AtomicUsize = AtomicUsize::new(0);
static FB_HEIGHT: AtomicUsize = AtomicUsize::new(0);
static FONT_BASE: AtomicUsize = AtomicUsize::new(0);

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
// Global output cursor — shared between kernel and userspace output path.
// Protected by cli/sti at call sites (single-core).
// ---------------------------------------------------------------------------

/// Current Y position of the userspace output cursor (pixels).
static USERSPACE_CURSOR_Y: AtomicUsize = AtomicUsize::new(0);
/// Current X position of the userspace output cursor (pixels).
static USERSPACE_CURSOR_X: AtomicUsize = AtomicUsize::new(20);
/// Current font scale for userspace output.
static GLOBAL_SCALE: AtomicUsize = AtomicUsize::new(1);

/// Set the starting Y for userspace output (called by shell after drawing header).
pub fn set_userspace_cursor(x: usize, y: usize) {
    USERSPACE_CURSOR_X.store(x, Ordering::Relaxed);
    USERSPACE_CURSOR_Y.store(y, Ordering::Relaxed);
}

pub fn get_userspace_cursor_y() -> usize {
    USERSPACE_CURSOR_Y.load(Ordering::Relaxed)
}

pub fn set_font_scale(scale: usize) {
    GLOBAL_SCALE.store(scale.clamp(1, 4), Ordering::Relaxed);
}

pub fn get_font_scale() -> usize {
    GLOBAL_SCALE.load(Ordering::Relaxed)
}

/// Write a byte to the global framebuffer at the userspace cursor position.
/// Handles newline and basic scrolling (clears back to y=120 when near bottom).
pub fn write_byte_to_fb(b: u8) {
    let fb_base = FB_BASE.load(Ordering::Relaxed);
    let fb_pitch = FB_PITCH.load(Ordering::Relaxed);
    let fb_w = FB_WIDTH.load(Ordering::Relaxed);
    let fb_h = FB_HEIGHT.load(Ordering::Relaxed);
    let font_b = FONT_BASE.load(Ordering::Relaxed);

    if fb_base == 0 || font_b == 0 || fb_w == 0 || fb_h == 0 {
        return;
    }

    let mut x = USERSPACE_CURSOR_X.load(Ordering::Relaxed);
    let mut y = USERSPACE_CURSOR_Y.load(Ordering::Relaxed);

    // Initialize cursor if never set
    if y == 0 {
        y = 120;
    }

    let scale = GLOBAL_SCALE.load(Ordering::Relaxed);
    let char_w = 8 * scale;
    let char_h = 16 * scale;
    const MARGIN_LEFT: usize = 20;
    const START_Y: usize = 120;

    match b {
        b'\n' => {
            x = MARGIN_LEFT;
            y += char_h;
            if y + char_h >= fb_h {
                unsafe {
                    clear_terminal_area();
                }
                y = START_Y;
            }
        }
        b'\x08' => {
            if x >= MARGIN_LEFT + char_w {
                x -= char_w;
                unsafe {
                    let base = fb_base as *mut u32;
                    for py in y..y + char_h {
                        if py >= fb_h {
                            break;
                        }
                        let row_ptr = base.add(py * fb_pitch + x);
                        for px in 0..char_w {
                            if x + px >= fb_w {
                                break;
                            }
                            row_ptr.add(px).write_volatile(BG_COLOR);
                        }
                    }
                }
            }
        }
        _ => {
            // Skip non-ASCII silently
            if b > 127 {
                USERSPACE_CURSOR_X.store(x, Ordering::Relaxed);
                USERSPACE_CURSOR_Y.store(y, Ordering::Relaxed);
                return;
            }
            if y + char_h <= fb_h && x + char_w <= fb_w {
                unsafe {
                    putchar_raw(
                        b as char, x, y, TEXT_COLOR, scale, fb_base, fb_pitch, font_b,
                    );
                }
            }
            x += char_w;
            if x + char_w >= fb_w {
                x = MARGIN_LEFT;
                y += char_h;
                if y + char_h >= fb_h {
                    unsafe {
                        clear_terminal_area();
                    }
                    y = START_Y;
                }
            }
        }
    }

    USERSPACE_CURSOR_X.store(x, Ordering::Relaxed);
    USERSPACE_CURSOR_Y.store(y, Ordering::Relaxed);
}

pub unsafe fn clear_terminal_area() {
    let fb_base = FB_BASE.load(Ordering::Relaxed);
    let fb_pitch = FB_PITCH.load(Ordering::Relaxed);
    let fb_w = FB_WIDTH.load(Ordering::Relaxed);
    let fb_h = FB_HEIGHT.load(Ordering::Relaxed);

    if fb_base == 0 || fb_w == 0 || fb_h <= 120 {
        return;
    }

    draw_rect_raw(0, 120, fb_w, fb_h - 120, BG_COLOR, fb_base, fb_pitch);
}
// ---------------------------------------------------------------------------
// Internal raw glyph renderer (no boot_info pointer needed)
// ---------------------------------------------------------------------------

unsafe fn putchar_raw(
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
// Existing Console / draw helpers (unchanged)
// ---------------------------------------------------------------------------

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
