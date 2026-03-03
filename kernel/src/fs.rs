use alloc::vec::Vec;

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
        Self {
            files: Vec::new(),
        }
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
}
