use super::super::{Pipe, MAX_PIPES, PIPE_CAPACITY};

const PIPE_WAIT_SPINS: usize = 2000;

pub(super) unsafe fn retain_read(pipe_idx: usize) -> bool {
    if pipe_idx >= MAX_PIPES || !super::PIPES[pipe_idx].in_use {
        return false;
    }
    super::PIPES[pipe_idx].read_refs += 1;
    true
}

pub(super) unsafe fn retain_write(pipe_idx: usize) -> bool {
    if pipe_idx >= MAX_PIPES || !super::PIPES[pipe_idx].in_use {
        return false;
    }
    super::PIPES[pipe_idx].write_refs += 1;
    true
}

pub(super) unsafe fn release_read(pipe_idx: usize) {
    if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use {
        let pipe = &mut super::PIPES[pipe_idx];
        if pipe.read_refs > 0 {
            pipe.read_refs -= 1;
        }
        if pipe.read_refs == 0 && pipe.write_refs == 0 {
            *pipe = Pipe::empty();
        }
    }
}

pub(super) unsafe fn release_write(pipe_idx: usize) {
    if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use {
        let pipe = &mut super::PIPES[pipe_idx];
        if pipe.write_refs > 0 {
            pipe.write_refs -= 1;
        }
        if pipe.read_refs == 0 && pipe.write_refs == 0 {
            *pipe = Pipe::empty();
        }
    }
}

pub(super) unsafe fn get_mut(fd: usize) -> Option<(&'static mut Pipe, bool)> {
    match super::get_fd_target(fd)? {
        super::super::FdTarget::PipeRead(pipe_idx)
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            Some((&mut super::PIPES[pipe_idx], true))
        }
        super::super::FdTarget::PipeWrite(pipe_idx)
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            Some((&mut super::PIPES[pipe_idx], false))
        }
        _ => None,
    }
}

pub(super) unsafe fn peer_closed(fd: usize) -> Option<bool> {
    match super::get_fd_target(fd)? {
        super::super::FdTarget::PipeRead(pipe_idx)
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            Some(super::PIPES[pipe_idx].write_refs == 0)
        }
        super::super::FdTarget::PipeWrite(pipe_idx)
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            Some(super::PIPES[pipe_idx].read_refs == 0)
        }
        _ => None,
    }
}

pub(super) unsafe fn status_flags(fd: usize) -> Option<u32> {
    match super::get_fd_target(fd)? {
        super::super::FdTarget::PipeRead(pipe_idx)
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            Some(super::PIPES[pipe_idx].read_flags)
        }
        super::super::FdTarget::PipeWrite(pipe_idx)
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            Some(super::PIPES[pipe_idx].write_flags)
        }
        _ => None,
    }
}

pub(super) unsafe fn set_status_flags(fd: usize, flags: u32) -> bool {
    match super::get_fd_target(fd) {
        Some(super::super::FdTarget::PipeRead(pipe_idx))
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            super::PIPES[pipe_idx].read_flags = flags;
            true
        }
        Some(super::super::FdTarget::PipeWrite(pipe_idx))
            if pipe_idx < MAX_PIPES && super::PIPES[pipe_idx].in_use =>
        {
            super::PIPES[pipe_idx].write_flags = flags;
            true
        }
        _ => false,
    }
}

pub(super) unsafe fn write(fd: usize, buf: *const u8, len: usize) -> Option<usize> {
    let (pipe, is_read_end) = get_mut(fd)?;
    if is_read_end {
        return None;
    }
    if pipe.read_refs == 0 {
        return Some(0);
    }

    let mut written = 0usize;
    while written < len && pipe.len < PIPE_CAPACITY {
        pipe.buf[pipe.write_pos] = *buf.add(written);
        pipe.write_pos = (pipe.write_pos + 1) % PIPE_CAPACITY;
        pipe.len += 1;
        written += 1;
    }
    Some(written)
}

pub(super) unsafe fn read(fd: usize, buf: *mut u8, count: usize) -> Option<usize> {
    loop {
        let peer_closed = peer_closed(fd)?;
        let (pipe, is_read_end) = get_mut(fd)?;
        if !is_read_end {
            return None;
        }
        if pipe.len > 0 {
            let mut read = 0usize;
            while read < count && pipe.len > 0 {
                buf.add(read).write(pipe.buf[pipe.read_pos]);
                pipe.read_pos = (pipe.read_pos + 1) % PIPE_CAPACITY;
                pipe.len -= 1;
                read += 1;
            }
            return Some(read);
        }
        if peer_closed {
            return Some(0);
        }

        crate::proc::scheduler::wait_for_event(PIPE_WAIT_SPINS);
    }
}

pub(super) unsafe fn alloc() -> Option<usize> {
    let pipe_idx = super::PIPES.iter().position(|p| !p.in_use)?;
    super::PIPES[pipe_idx] = Pipe {
        buf: [0; PIPE_CAPACITY],
        read_pos: 0,
        write_pos: 0,
        len: 0,
        read_refs: 1,
        write_refs: 1,
        read_flags: 0,
        write_flags: 0,
        in_use: true,
    };
    Some(pipe_idx)
}

pub(super) unsafe fn clear(pipe_idx: usize) {
    if pipe_idx < MAX_PIPES {
        super::PIPES[pipe_idx] = Pipe::empty();
    }
}
