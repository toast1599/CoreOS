use core::cell::UnsafeCell;

use crate::drivers::{serial, vga};

const COOKED_CAPACITY: usize = 1024;
const LINE_CAPACITY: usize = 256;

struct Inner {
    cooked: [u8; COOKED_CAPACITY],
    cooked_head: usize,
    cooked_tail: usize,
    line: [u8; LINE_CAPACITY],
    line_len: usize,
}

impl Inner {
    const fn new() -> Self {
        Self {
            cooked: [0; COOKED_CAPACITY],
            cooked_head: 0,
            cooked_tail: 0,
            line: [0; LINE_CAPACITY],
            line_len: 0,
        }
    }

    #[inline]
    fn cooked_is_empty(&self) -> bool {
        self.cooked_head == self.cooked_tail
    }

    #[inline]
    fn cooked_is_full(&self) -> bool {
        (self.cooked_head + 1) % COOKED_CAPACITY == self.cooked_tail
    }

    #[inline]
    fn cooked_push(&mut self, byte: u8) {
        if self.cooked_is_full() {
            return;
        }
        self.cooked[self.cooked_head] = byte;
        self.cooked_head = (self.cooked_head + 1) % COOKED_CAPACITY;
    }

    #[inline]
    fn cooked_pop(&mut self) -> Option<u8> {
        if self.cooked_is_empty() {
            return None;
        }
        let byte = self.cooked[self.cooked_tail];
        self.cooked_tail = (self.cooked_tail + 1) % COOKED_CAPACITY;
        Some(byte)
    }

    fn commit_line(&mut self) {
        for i in 0..self.line_len {
            self.cooked_push(self.line[i]);
        }
        self.cooked_push(b'\n');
        self.line_len = 0;
    }
}

enum Echo {
    None,
    Byte(u8),
    Backspace,
    Newline,
}

pub struct Tty {
    inner: UnsafeCell<Inner>,
}

unsafe impl Sync for Tty {}

impl Tty {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(Inner::new()),
        }
    }

    #[inline]
    fn with_lock<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Inner) -> R,
    {
        unsafe {
            let flags = crate::arch::amd64::cpu::push_cli();
            let inner = &mut *self.inner.get();
            let result = f(inner);
            crate::arch::amd64::cpu::pop_cli(flags);
            result
        }
    }

    pub fn input_char(&self, byte: u8) {
        let echo = self.with_lock(|inner| match byte {
            b'\r' | b'\n' => {
                inner.commit_line();
                Echo::Newline
            }
            0x08 | 0x7f => {
                if inner.line_len == 0 {
                    Echo::None
                } else {
                    inner.line_len -= 1;
                    Echo::Backspace
                }
            }
            0x20..=0x7e => {
                if inner.line_len >= LINE_CAPACITY {
                    Echo::None
                } else {
                    inner.line[inner.line_len] = byte;
                    inner.line_len += 1;
                    Echo::Byte(byte)
                }
            }
            _ => Echo::None,
        });

        match echo {
            Echo::None => {}
            Echo::Byte(byte) => write_bytes(core::slice::from_ref(&byte)),
            Echo::Backspace => write_bytes(b"\x08 \x08"),
            Echo::Newline => write_bytes(b"\n"),
        }
    }

    pub fn read(&self, buf: *mut u8, count: usize) -> usize {
        if count == 0 {
            return 0;
        }

        loop {
            let copied = self.with_lock(|inner| {
                let mut copied = 0usize;
                while copied < count {
                    let Some(byte) = inner.cooked_pop() else {
                        break;
                    };
                    unsafe {
                        buf.add(copied).write(byte);
                    }
                    copied += 1;
                    if byte == b'\n' {
                        break;
                    }
                }
                copied
            });

            if copied != 0 {
                return copied;
            }

            crate::proc::scheduler::wait_for_event(1);
        }
    }

    pub fn write(&self, buf: *const u8, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        let bytes = unsafe { core::slice::from_raw_parts(buf, len) };
        write_bytes(bytes);
        len
    }
}

fn write_bytes(bytes: &[u8]) {
    for &byte in bytes {
        unsafe {
            serial::write_byte(byte);
        }
        vga::console::write_byte_to_fb(byte);
    }
}

pub static TTY0: Tty = Tty::new();
