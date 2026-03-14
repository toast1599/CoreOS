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

    #[allow(dead_code)]
    pub fn push(&self, c: char) {
        unsafe {
            let head = *self.head.get();
            let next = (head + 1) % BUF_SIZE;

            if next != *self.tail.get() {
                (*self.buf.get())[head] = c;
                *self.head.get() = next;
            }
        }
    }

    pub fn pop(&self) -> Option<char> {
        unsafe {
            let tail = *self.tail.get();
            if tail == *self.head.get() {
                return None;
            }

            let c = (*self.buf.get())[tail];
            *self.tail.get() = (tail + 1) % BUF_SIZE;
            Some(c)
        }
    }
    #[allow(dead_code)]
    pub fn flush(&self) {
        unsafe {
            *self.head.get() = 0;
            *self.tail.get() = 0;
        }
    }
}

pub static KEYBUF: KeyBuffer = KeyBuffer::new();
