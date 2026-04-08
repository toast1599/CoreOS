use core::cell::UnsafeCell;

const BUF_SIZE: usize = 128;

pub struct KeyBuffer {
    buf: UnsafeCell<[char; BUF_SIZE]>,
    head: UnsafeCell<usize>,
    tail: UnsafeCell<usize>,
}

unsafe impl Sync for KeyBuffer {}

impl KeyBuffer {
    #[allow(dead_code)]
    pub const fn new() -> Self {
        Self {
            buf: UnsafeCell::new(['\0'; BUF_SIZE]),
            head: UnsafeCell::new(0),
            tail: UnsafeCell::new(0),
        }
    }

    pub fn push(&self, c: char) {
        unsafe {
            let flags = crate::arch::amd64::cpu::push_cli();

            let head = *self.head.get();
            let next = (head + 1) % BUF_SIZE;

            if next != *self.tail.get() {
                (*self.buf.get())[head] = c;
                *self.head.get() = next;
            }

            crate::arch::amd64::cpu::pop_cli(flags);
        }
    }

    pub fn pop(&self) -> Option<char> {
        unsafe {
            let flags = crate::arch::amd64::cpu::push_cli();

            let tail = *self.tail.get();
            if tail == *self.head.get() {
                crate::arch::amd64::cpu::pop_cli(flags);
                return None;
            }

            let c = (*self.buf.get())[tail];
            *self.tail.get() = (tail + 1) % BUF_SIZE;

            crate::arch::amd64::cpu::pop_cli(flags);
            Some(c)
        }
    }

    #[allow(dead_code)]
    pub fn flush(&self) {
        unsafe {
            let flags = crate::arch::amd64::cpu::push_cli();

            *self.head.get() = 0;
            *self.tail.get() = 0;

            crate::arch::amd64::cpu::pop_cli(flags);
        }
    }
}

pub static KEYBUF: KeyBuffer = KeyBuffer::new();
