pub const MAX_MMAP_ENTRIES: usize = 256;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct CoreOsMemMapEntry {
    pub physical_start: u64,
    pub num_pages: u64,
    pub mem_type: u32,
    pub _pad: u32,
}

#[repr(C, packed)]
pub struct CoreOsBootInfo {
    pub fb_base: u64,
    pub fb_size: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub mmap: [CoreOsMemMapEntry; MAX_MMAP_ENTRIES],
    pub mmap_count: u32,
    pub _pad: u32,
    pub kernel_phys_base: u64,
    pub kernel_alloc_size: u64,
    pub user_elf_base: u64,
    pub user_elf_size: u64,
    pub font_base: u64,
    pub font_size: u64,
    pub tsc_bootloader_start: u64,
}

impl CoreOsBootInfo {
    pub const fn zeroed() -> Self {
        Self {
            fb_base: 0,
            fb_size: 0,
            width: 0,
            height: 0,
            pitch: 0,
            mmap: [CoreOsMemMapEntry {
                physical_start: 0,
                num_pages: 0,
                mem_type: 0,
                _pad: 0,
            }; MAX_MMAP_ENTRIES],
            mmap_count: 0,
            _pad: 0,
            kernel_phys_base: 0,
            kernel_alloc_size: 0,
            user_elf_base: 0,
            user_elf_size: 0,
            font_base: 0,
            font_size: 0,
            tsc_bootloader_start: 0,
        }
    }
}

pub type KernelEntry = unsafe extern "win64" fn(*const CoreOsBootInfo) -> !;
