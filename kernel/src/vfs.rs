use alloc::vec::Vec;

use crate::fs::{File, RamFS, FILESYSTEM};

extern crate alloc;

pub fn init() {
    *FILESYSTEM.lock() = Some(RamFS::new());
}

pub fn with_fs<R>(f: impl FnOnce(&RamFS) -> R) -> Option<R> {
    let guard = FILESYSTEM.lock();
    guard.as_ref().map(f)
}

pub fn with_fs_mut<R>(f: impl FnOnce(&mut RamFS) -> R) -> Option<R> {
    let mut guard = FILESYSTEM.lock();
    guard.as_mut().map(f)
}

pub fn create(name: &[char]) -> bool {
    with_fs_mut(|fs| fs.create(name)).unwrap_or(false)
}

pub fn find(name: &[char]) -> Option<File> {
    with_fs(|fs| fs.find(name).cloned()).flatten()
}

pub fn clone_bytes(name: &[char]) -> Option<Vec<u8>> {
    with_fs(|fs| fs.find(name).map(|f| f.data.clone())).flatten()
}

pub fn append_all(name: &[char], bytes: &[u8]) -> bool {
    with_fs_mut(|fs| {
        let Some(file) = fs.find_mut(name) else {
            return false;
        };
        file.data.extend_from_slice(bytes);
        true
    })
    .unwrap_or(false)
}
