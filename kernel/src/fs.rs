pub const MAX_FILES: usize = 16;
pub const MAX_FILE_SIZE: usize = 2048;

#[derive(Copy, Clone)]
pub struct File {
    pub name: [char; 16],
    pub data: [u8; MAX_FILE_SIZE],
    pub size: usize,
    pub used: bool,
}

pub struct RamFS {
    pub files: [File; MAX_FILES],
}

impl RamFS {
    pub const fn new() -> Self {
        Self {
            files: [File {
                name: ['\0'; 16],
                data: [0; MAX_FILE_SIZE],
                size: 0,
                used: false,
            }; MAX_FILES],
        }
    }

    pub fn find_file(&mut self, name: &str) -> Option<&mut File> {
        for file in self.files.iter_mut() {
            if !file.used { continue; }
            let mut matches = true;
            let mut i = 0;
            for c in name.chars() {
                if i >= 16 || file.name[i] != c { matches = false; break; }
                i += 1;
            }
            if matches && (i == 16 || file.name[i] == '\0') { return Some(file); }
        }
        None
    }

    pub fn create(&mut self, name: &[char]) -> bool {
        for file in self.files.iter() {
            if file.used {
                let mut matches = true;
                for (i, &c) in name.iter().enumerate() {
                    if i >= 16 || file.name[i] != c { matches = false; break; }
                }
                if matches && (name.len() == 16 || file.name[name.len()] == '\0') {
                    return false;
                }
            }
        }

        for file in self.files.iter_mut() {
            if !file.used {
                file.used = true;
                file.size = 0;
                for i in 0..16 { file.name[i] = '\0'; }
                for (i, &c) in name.iter().enumerate() {
                    if i < 16 { file.name[i] = c; }
                }
                return true;
            }
        }
        false
    }
}
