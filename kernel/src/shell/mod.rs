#![allow(dead_code)]
#![allow(unused_variables)]

/// Shell — input buffer management and command dispatch.
///
/// The shell owns a fixed-size character buffer and a cursor position.
/// `execute()` parses the current buffer and dispatches to the command
/// handlers in `shell::commands`.
pub mod commands;
pub mod ui;

// ---------------------------------------------------------------------------
// Input buffer
// ---------------------------------------------------------------------------

pub const BUF_LEN: usize = 64;

pub struct Shell {
    pub buffer: [char; BUF_LEN],
    pub cursor: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            buffer: ['\0'; BUF_LEN],
            cursor: 0,
        }
    }

    pub fn push(&mut self, c: char) {
        if self.cursor < BUF_LEN - 1 {
            self.buffer[self.cursor] = c;
            self.cursor += 1;
        }
    }

    pub fn pop(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer[self.cursor] = '\0';
        }
    }

    pub fn clear(&mut self) {
        self.buffer = ['\0'; BUF_LEN];
        self.cursor = 0;
    }

    /// Execute the current buffer contents and clear it afterwards.
    /// Returns `ShellOutput` describing what the UI should do next.
    pub fn execute(&mut self, ctx: &mut commands::ShellContext) -> commands::ShellOutput {
        // Log the command to serial
        unsafe {
            core::arch::asm!("cli", options(nostack, nomem));
            crate::drivers::serial::write_str("[SHELL] cmd: [");
            for i in 0..self.cursor {
                let c = self.buffer[i];
                if c != '\0' {
                    crate::drivers::serial::write_byte(c as u8);
                }
            }
            crate::drivers::serial::write_str("]\n");
            core::arch::asm!("sti", options(nostack, nomem));
        }

        let output = commands::dispatch(self, ctx);
        self.clear();
        output
    }
}

// ---------------------------------------------------------------------------
// Buffer helpers (used by commands module)
// ---------------------------------------------------------------------------

/// Check whether the buffer starts with `cmd` followed by end-of-input or a space.
pub fn cmd_is(buffer: &[char; BUF_LEN], cmd: &str) -> bool {
    let mut i = 0;
    for c in cmd.chars() {
        if i >= BUF_LEN || buffer[i] != c {
            return false;
        }
        i += 1;
    }
    i >= BUF_LEN || buffer[i] == '\0' || buffer[i] == ' '
}

/// Return the first word after the command (stops at space or null).
/// Use this for filenames: `touch area`, `cat area`, `rm area`.
pub fn get_arg(buffer: &[char; BUF_LEN], cmd_len: usize) -> &[char] {
    let start = cmd_len + 1;
    if start >= BUF_LEN || buffer[start] == '\0' {
        return &[];
    }
    let mut end = start;
    while end < BUF_LEN && buffer[end] != '\0' && buffer[end] != ' ' {
        end += 1;
    }
    &buffer[start..end]
}

/// Return everything after `skip` characters (i.e. after "cmd filename ").
/// Use this for content args: `write area <content>`, `push area <content>`.
pub fn get_rest(buffer: &[char; BUF_LEN], skip: usize) -> &[char] {
    if skip >= BUF_LEN || buffer[skip] == '\0' {
        return &[];
    }
    let mut end = skip;
    while end < BUF_LEN && buffer[end] != '\0' {
        end += 1;
    }
    &buffer[skip..end]
}
