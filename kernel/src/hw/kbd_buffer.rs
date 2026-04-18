use core::cell::UnsafeCell;

const BUF_SIZE: usize = 128;

/// Internal ring buffer state.
/// All invariants are maintained here.
struct Inner {
    buf: [char; BUF_SIZE],
    head: usize,
    tail: usize,
}

impl Inner {
    const fn new() -> Self {
        Self {
            buf: ['\0'; BUF_SIZE],
            head: 0,
            tail: 0,
        }
    }

    /// Returns true if buffer is empty
    #[inline]
    fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    /// Returns true if buffer is full
    #[inline]
    fn is_full(&self) -> bool {
        (self.head + 1) % BUF_SIZE == self.tail
    }

    /// Push a character into the buffer.
    /// Drops input if buffer is full.
    #[inline]
    fn push(&mut self, c: char) {
        if self.is_full() {
            return;
        }

        self.buf[self.head] = c;
        self.head = (self.head + 1) % BUF_SIZE;
    }

    /// Pop a character from the buffer.
    #[inline]
    fn pop(&mut self) -> Option<char> {
        if self.is_empty() {
            return None;
        }

        let c = self.buf[self.tail];
        self.tail = (self.tail + 1) % BUF_SIZE;
        Some(c)
    }
}

/// Interrupt-safe wrapper around the ring buffer.
///
/// Safety model:
/// - All mutation happens with interrupts disabled
/// - No references escape the critical section
pub struct KeyBuffer {
    inner: UnsafeCell<Inner>,
}

unsafe impl Sync for KeyBuffer {}

impl KeyBuffer {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(Inner::new()),
        }
    }

    /// Execute a closure with interrupts disabled.
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

    /// Push a character into the buffer.
    #[inline]
    pub fn push(&self, c: char) {
        self.with_lock(|inner| inner.push(c));
    }

    /// Pop a character from the buffer.
    #[inline]
    pub fn pop(&self) -> Option<char> {
        self.with_lock(|inner| inner.pop())
    }

    /// Optional helper: check if empty (non-critical, still locked for consistency)
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.with_lock(|inner| inner.is_empty())
    }
}

pub static KEYBUF: KeyBuffer = KeyBuffer::new();
