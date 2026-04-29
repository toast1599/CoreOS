use crate::drivers::serial;
use crate::drivers::vga;
use crate::hw::kbd_buffer;
use crate::proc;
use crate::proc::FdTarget;

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

unsafe fn write_char_device(target: FdTarget, buf: *const u8, len: usize) -> usize {
    match target {
        FdTarget::Tty => write_stdio(buf, len),
        FdTarget::Null | FdTarget::Zero => len,
        _ => 0,
    }
}

unsafe fn read_char_device(target: FdTarget, buf: *mut u8, count: usize) -> usize {
    match target {
        FdTarget::Tty => read_stdin(buf, count),
        FdTarget::Null => 0,
        FdTarget::Zero => {
            core::ptr::write_bytes(buf, 0, count);
            count
        }
        _ => 0,
    }
}

pub unsafe fn write_to_fd(fd: usize, buf: *const u8, len: usize) -> Option<usize> {
    if proc::is_stdout_or_stderr(fd) {
        return Some(write_stdio(buf, len));
    }
    if let Some(target @ (FdTarget::Tty | FdTarget::Null | FdTarget::Zero)) =
        proc::get_fd_target(fd)
    {
        return Some(write_char_device(target, buf, len));
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
    if let Some(target @ (FdTarget::Tty | FdTarget::Null | FdTarget::Zero)) =
        proc::get_fd_target(fd)
    {
        return Some(read_char_device(target, buf, count));
    }

    if let Some(read) = proc::read_pipe(fd, buf, count) {
        return Some(read);
    }

    proc::read_file(fd, buf, count)
}
