use crate::drivers::serial;
use crate::drivers::vga;
use crate::hw::kbd_buffer;
use crate::proc;

const IO_WAIT_SPINS: usize = 2000;

unsafe fn write_stdio(buf: *const u8, len: usize) -> usize {
    for i in 0..len {
        let b = *buf.add(i);
        serial::write_byte(b);
        vga::console::write_byte_to_fb(b);
    }
    len
}

unsafe fn read_stdin_byte() -> u8 {
    loop {
        if let Some(c) = kbd_buffer::KEYBUF.pop() {
            return c as u8;
        }
        crate::proc::scheduler::wait_for_event(IO_WAIT_SPINS);
    }
}

unsafe fn read_stdin(buf: *mut u8, count: usize) -> usize {
    for i in 0..count {
        buf.add(i).write(read_stdin_byte());
    }
    count
}

pub unsafe fn write_to_fd(fd: usize, buf: *const u8, len: usize) -> Option<usize> {
    if proc::is_stdout_or_stderr(fd) {
        return Some(write_stdio(buf, len));
    }
    if let Some(written) = proc::write_file(fd, buf, len) {
        return Some(written);
    }
    proc::write_pipe(fd, buf, len)
}

pub unsafe fn read_from_fd(fd: usize, buf: *mut u8, count: usize) -> Option<usize> {
    if count == 0 {
        return Some(0);
    }

    if proc::is_stdin(fd) {
        return Some(read_stdin(buf, count));
    }

    if let Some(read) = proc::read_pipe(fd, buf, count) {
        return Some(read);
    }

    proc::read_file(fd, buf, count)
}
