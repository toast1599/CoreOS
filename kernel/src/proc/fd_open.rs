use super::super::{DescriptorInfo, OpenFile, MAX_OPEN_FILES};

pub(super) unsafe fn alloc(file_idx: usize) -> Option<usize> {
    for (i, of) in super::OPEN_FILES.iter_mut().enumerate() {
        if !of.in_use {
            *of = OpenFile {
                file_idx,
                offset: 0,
                status_flags: 0,
                refs: 1,
                in_use: true,
            };
            return Some(i);
        }
    }
    None
}

pub(super) unsafe fn retain(open_idx: usize) -> bool {
    if open_idx >= MAX_OPEN_FILES || !super::OPEN_FILES[open_idx].in_use {
        return false;
    }
    super::OPEN_FILES[open_idx].refs += 1;
    true
}

pub(super) unsafe fn release(open_idx: usize) {
    if open_idx < MAX_OPEN_FILES && super::OPEN_FILES[open_idx].in_use {
        let of = &mut super::OPEN_FILES[open_idx];
        if of.refs > 1 {
            of.refs -= 1;
        } else {
            *of = OpenFile::empty();
        }
    }
}

pub(super) unsafe fn descriptor_info(open_idx: usize) -> Option<DescriptorInfo> {
    if open_idx >= MAX_OPEN_FILES || !super::OPEN_FILES[open_idx].in_use {
        return None;
    }
    let of = &super::OPEN_FILES[open_idx];
    Some(DescriptorInfo::File {
        file_idx: of.file_idx,
        size: crate::syscall::fs::fs_file_size(of.file_idx),
    })
}

pub(super) unsafe fn get_mut(open_idx: usize) -> Option<&'static mut OpenFile> {
    if open_idx >= MAX_OPEN_FILES || !super::OPEN_FILES[open_idx].in_use {
        return None;
    }
    Some(&mut super::OPEN_FILES[open_idx])
}

pub(super) unsafe fn status_flags(open_idx: usize) -> Option<u32> {
    if open_idx >= MAX_OPEN_FILES || !super::OPEN_FILES[open_idx].in_use {
        return None;
    }
    Some(super::OPEN_FILES[open_idx].status_flags)
}

pub(super) unsafe fn set_status_flags(open_idx: usize, flags: u32) -> bool {
    if open_idx >= MAX_OPEN_FILES || !super::OPEN_FILES[open_idx].in_use {
        return false;
    }
    super::OPEN_FILES[open_idx].status_flags = flags;
    true
}
