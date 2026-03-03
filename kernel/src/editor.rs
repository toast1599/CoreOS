use crate::vga::{self, Console};
use crate::boot::CoreOS_BootInfo;
use crate::FILESYSTEM;
use core::fmt::Write;

pub struct Editor {
    pub buffer: [u8; 8192],
    pub length: usize,
    pub cursor: usize,
    pub filename: [char; 16],
    pub is_dirty: bool,
}

impl Editor {
    pub const fn new() -> Self {
        Self {
            buffer: [0; 8192],
            length: 0,
            cursor: 0,
            filename: ['\0'; 16],
            is_dirty: false,
        }
    }

    pub unsafe fn render(&self, boot_info: *const CoreOS_BootInfo) {
        // 1. Clear the work area (below header, above status bar)
        // Note: Using a fixed range avoids clearing the status bar area we're about to draw
        vga::clear_from(120, boot_info);
        
        // 2. Draw Text Content
        let mut con = Console { x: 20, y: 120, color: vga::TEXT_COLOR, scale: 1, boot_info };
        
        for i in 0..self.length {
            // Logic: Draw the cursor as an underscore if we are at the cursor position
            if i == self.cursor {
                let _ = write!(con, "_");
                // Reset X because the visual cursor shouldn't push the text
                con.x -= 8 * con.scale; 
            }
            
            let c = self.buffer[i] as char;
            let _ = write!(con, "{}", c);
        }

        // Draw cursor at the very end if cursor is at the length
        if self.cursor == self.length {
            let _ = write!(con, "_");
        }

        // 3. Draw Status Bar (Bottom of screen)
        let height = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).height)) as usize;
        let width = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).width)) as usize;
        let bar_y = height - 30;
        
        vga::draw_rect(0, bar_y, width, 30, vga::CLOCK_COLOR, boot_info);
        let mut bar = Console { x: 10, y: bar_y + 5, color: vga::BG_COLOR, scale: 1, boot_info };
        
        let _ = write!(bar, " FILE: ");
        for &c in self.filename.iter() {
            if c == '\0' { break; }
            let _ = write!(bar, "{}", c);
        }
        let _ = write!(bar, " | POS: {} | {}", self.cursor, if self.is_dirty { "MODIFIED" } else { "SAVED" });
        let _ = write!(bar, " | F1: Save | ESC: Exit");
    }

    pub fn insert_char(&mut self, c: char) {
        if self.length < 8191 {
            // Shift everything after cursor to the right
            for i in (self.cursor..self.length).rev() {
                self.buffer[i + 1] = self.buffer[i];
            }
            self.buffer[self.cursor] = c as u8;
            self.cursor += 1;
            self.length += 1;
            self.is_dirty = true;
        }
    }

    pub fn delete_char(&mut self) {
        if self.cursor > 0 {
            // Shift everything after cursor to the left
            for i in self.cursor..self.length {
                self.buffer[i - 1] = self.buffer[i];
            }
            
            self.cursor -= 1;
            self.length -= 1;
            
            // FIXED: Explicitly clear the byte at the old end of the string
            self.buffer[self.length] = 0; 
            
            self.is_dirty = true;
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor < self.length { self.cursor += 1; }
    }

    pub unsafe fn save_to_fs(&mut self) -> bool {
        // Use the global static FILESYSTEM
        if let Some(file) = FILESYSTEM.find_file_from_chars(&self.filename) {
            // Truncate to MAX_FILE_SIZE (2048) if necessary
            let save_len = if self.length > 2048 { 2048 } else { self.length };
            
            // Clear existing data in the file first
            for i in 0..2048 { file.data[i] = 0; }
            
            for i in 0..save_len {
                file.data[i] = self.buffer[i];
            }
            file.size = save_len;
            self.is_dirty = false;
            true
        } else {
            false
        }
    }
}
