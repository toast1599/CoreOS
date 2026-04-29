use super::super::{DescriptorInfo, OpenFile, MAX_OPEN_FILES};

pub(super) fn alloc(file_idx: usize, status_flags: u32) -> Option<usize> {
    let mut files = super::OPEN_FILES.lock();
    for (i, of) in files.iter_mut().enumerate() {
        if !of.in_use {
            *of = OpenFile {
                file_idx,
                offset: 0,
                status_flags,
                refs: 1,
                in_use: true,
            };
            return Some(i);
        }
    }
    None
}

pub(super) fn retain(open_idx: usize) -> bool {
    let mut files = super::OPEN_FILES.lock();
    if open_idx >= MAX_OPEN_FILES || !files[open_idx].in_use {
        return false;
    }
    files[open_idx].refs += 1;
    true
}

pub(super) fn release(open_idx: usize) {
    let mut files = super::OPEN_FILES.lock();
    if open_idx < MAX_OPEN_FILES && files[open_idx].in_use {
        let of = &mut files[open_idx];
        if of.refs > 1 {
            of.refs -= 1;
        } else {
            *of = OpenFile::empty();
        }
    }
}

pub(super) unsafe fn descriptor_info(open_idx: usize) -> Option<DescriptorInfo> {
    let files = super::OPEN_FILES.lock();
    if open_idx >= MAX_OPEN_FILES || !files[open_idx].in_use {
        return None;
    }
    let of = &files[open_idx];
    Some(DescriptorInfo::File {
        file_idx: of.file_idx,
        size: crate::syscall::fs::fs_file_size(of.file_idx),
    })
}

pub fn with_mut<R>(open_idx: usize, f: impl FnOnce(&mut OpenFile) -> R) -> Option<R> {
    let mut files = super::OPEN_FILES.lock();

    if open_idx >= MAX_OPEN_FILES || !files[open_idx].in_use {
        return None;
    }

    Some(f(&mut files[open_idx]))
}

pub(super) fn status_flags(open_idx: usize) -> Option<u32> {
    let files = super::OPEN_FILES.lock();
    if open_idx >= MAX_OPEN_FILES || !files[open_idx].in_use {
        return None;
    }
    Some(files[open_idx].status_flags)
}

pub(super) fn set_status_flags(open_idx: usize, flags: u32) -> bool {
    let mut files = super::OPEN_FILES.lock();

    if open_idx >= MAX_OPEN_FILES || !files[open_idx].in_use {
        return false;
    }

    files[open_idx].status_flags = flags;
    true
}
