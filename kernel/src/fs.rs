use alloc::vec::Vec;

extern crate alloc;

#[derive(Clone)]
pub struct File {
    pub name: Vec<char>,
    pub data: Vec<u8>,
}

pub struct RamFS {
    pub files: Vec<File>,
}

impl RamFS {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn create(&mut self, name: &[char]) -> bool {
        if name.is_empty() {
            return false;
        }

        if self.files.iter().any(|f| f.name == name) {
            return false;
        }

        self.files.push(File {
            name: name.to_vec(),
            data: Vec::new(),
        });

        true
    }

    pub fn find_mut(&mut self, name: &[char]) -> Option<&mut File> {
        self.files.iter_mut().find(|f| f.name == name)
    }

    pub fn find(&self, name: &[char]) -> Option<&File> {
        self.files.iter().find(|f| f.name == name)
    }

    pub fn remove(&mut self, name: &[char]) -> bool {
        if let Some(idx) = self.files.iter().position(|f| f.name.as_slice() == name) {
            self.files.remove(idx);
            true
        } else {
            false
        }
    }
}

pub static FILESYSTEM: crate::sync::SpinLock<Option<RamFS>> = crate::sync::SpinLock::new(None);

pub fn selftest_ramfs_basic() {
    crate::serial_fmt!("[SELFTEST] RamFS basic\n");
    let mut fs = RamFS::new();

    let alpha: [char; 5] = ['a', 'l', 'p', 'h', 'a'];
    let beta: [char; 4] = ['b', 'e', 't', 'a'];

    assert!(fs.create(&alpha));
    assert!(!fs.create(&alpha));
    assert!(fs.find(&alpha).is_some());
    assert!(fs.find(&beta).is_none());
    assert!(fs.remove(&alpha));
    assert!(!fs.remove(&alpha));
}
