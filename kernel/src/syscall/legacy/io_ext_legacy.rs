// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

use crate::proc::process;
use crate::usercopy::{copy_from_user, copy_to_user};
use crate::vfs::NodeType;

const EINVAL: i64 = -22;
const EBADF: i64 = -9;
const ESPIPE: i64 = -29;

/// pread64(fd, buf, count, offset) - Read from file at offset without changing file offset
pub fn syscall_pread64(fd: i32, buf: u64, count: u64, offset: i64) -> i64 {
    if buf == 0 || count == 0 || offset < 0 {
        return EINVAL;
    }

    let current_task = process::current_task();
    let task = current_task.lock();

    let fd_entry = match task.fd_table.get(fd as usize) {
        Some(Some(entry)) => entry.clone(),
        _ => return EBADF,
    };
    drop(task);

    let fd_locked = fd_entry.lock();
    let node = fd_locked.node.lock();

    if node.node_type == NodeType::Pipe {
        return ESPIPE; // Pipes don't support seeking
    }

    let offset = offset as usize;
    let available = node.data.len().saturating_sub(offset);
    let to_read = core::cmp::min(count as usize, available);

    if to_read == 0 {
        return 0;
    }

    let data = &node.data[offset..offset + to_read];
    if copy_to_user(buf, data).is_err() {
        return EINVAL;
    }

    to_read as i64
}

/// pwrite64(fd, buf, count, offset) - Write to file at offset without changing file offset
pub fn syscall_pwrite64(fd: i32, buf: u64, count: u64, offset: i64) -> i64 {
    if buf == 0 || count == 0 || offset < 0 {
        return EINVAL;
    }

    let current_task = process::current_task();
    let task = current_task.lock();

    let fd_entry = match task.fd_table.get(fd as usize) {
        Some(Some(entry)) => entry.clone(),
        _ => return EBADF,
    };
    drop(task);

    let fd_locked = fd_entry.lock();
    let mut node = fd_locked.node.lock();

    if node.node_type == NodeType::Pipe {
        return ESPIPE;
    }

    let offset = offset as usize;
    let mut write_buf = alloc::vec![0u8; count as usize];
    if copy_from_user(buf, &mut write_buf).is_err() {
        return EINVAL;
    }

    // Extend file if necessary
    if offset + write_buf.len() > node.data.len() {
        node.data.resize(offset + write_buf.len(), 0);
    }

    // Write data at offset
    node.data[offset..offset + write_buf.len()].copy_from_slice(&write_buf);

    count as i64
}

/// sendfile(out_fd, in_fd, offset, count) - Copy data between file descriptors
pub fn syscall_sendfile(out_fd: i32, in_fd: i32, offset: u64, count: u64) -> i64 {
    let current_task = process::current_task();
    let task = current_task.lock();

    let in_entry = match task.fd_table.get(in_fd as usize) {
        Some(Some(entry)) => entry.clone(),
        _ => return EBADF,
    };

    let out_entry = match task.fd_table.get(out_fd as usize) {
        Some(Some(entry)) => entry.clone(),
        _ => return EBADF,
    };
    drop(task);

    // Read from input fd
    let in_fd_locked = in_entry.lock();
    let in_node = in_fd_locked.node.lock();

    let read_offset = if offset != 0 {
        // offset is a pointer to offset value
        // For simplicity, we'll treat offset==0 as NULL
        0
    } else {
        in_fd_locked.offset
    };

    let available = in_node.data.len().saturating_sub(read_offset);
    let to_copy = core::cmp::min(count as usize, available);

    if to_copy == 0 {
        return 0;
    }

    let data = in_node.data[read_offset..read_offset + to_copy].to_vec();
    drop(in_node);
    drop(in_fd_locked);

    // Write to output fd
    let out_fd_locked = out_entry.lock();
    let mut out_node = out_fd_locked.node.lock();

    let write_offset = out_fd_locked.offset;
    if write_offset + data.len() > out_node.data.len() {
        out_node.data.resize(write_offset + data.len(), 0);
    }

    out_node.data[write_offset..write_offset + data.len()].copy_from_slice(&data);
    drop(out_node);
    drop(out_fd_locked);

    // Update output fd offset
    let mut out_fd_mut = out_entry.lock();
    out_fd_mut.offset += to_copy;

    to_copy as i64
}

use alloc;
