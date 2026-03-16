use core::fmt::Write;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::boot::CoreOS_BootInfo;
use super::{FB_BASE, FB_PITCH, FB_WIDTH, FB_HEIGHT, FONT_BASE, BG_COLOR, TEXT_COLOR, putchar_raw, draw_rect_raw};

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

pub fn tty_winsize() -> (u16, u16, u16, u16) {
    let scale = get_font_scale().max(1);
    let fb_w = super::FB_WIDTH.load(Ordering::Relaxed);
    let fb_h = super::FB_HEIGHT.load(Ordering::Relaxed);
    let cols = fb_w.saturating_sub(40) / (8 * scale);
    let rows = fb_h.saturating_sub(120) / (16 * scale);
    (
        rows.min(u16::MAX as usize) as u16,
        cols.min(u16::MAX as usize) as u16,
        fb_w.min(u16::MAX as usize) as u16,
        fb_h.min(u16::MAX as usize) as u16,
    )
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
                    super::putchar(c, self.x, self.y, self.color, self.scale, self.boot_info);
                }
                self.x += 8 * self.scale;
            }
        }
        Ok(())
    }
}
