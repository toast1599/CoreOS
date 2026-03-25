// Filesystem helpers used by the VFS-facing syscall layer.

pub unsafe fn fs_find_idx(name: &[char]) -> Option<usize> {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    fs_guard
        .as_ref()?
        .files
        .iter()
        .position(|f| f.name.as_slice() == name)
}

pub unsafe fn fs_file_size(file_idx: usize) -> usize {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    match fs_guard.as_ref() {
        Some(fs) if file_idx < fs.files.len() => fs.files[file_idx].data.len(),
        _ => 0,
    }
}

pub unsafe fn fs_read(file_idx: usize, offset: usize, buf: *mut u8, count: usize) -> usize {
    let fs_guard = crate::fs::FILESYSTEM.lock();
    let fs = match fs_guard.as_ref() {
        Some(f) => f,
        None => return 0,
    };
    if file_idx >= fs.files.len() {
        return 0;
    }
    let data = &fs.files[file_idx].data;
    let available = data.len().saturating_sub(offset);
    let to_read = count.min(available);
    if to_read == 0 {
        return 0;
    }
    core::ptr::copy_nonoverlapping(data[offset..].as_ptr(), buf, to_read);
    to_read
}
