/// Must match CoreOS_MemMapEntry in bootloader/main.c exactly.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MemMapEntry {
    pub physical_start: u64,
    pub num_pages: u64,
    pub mem_type: u32,
    pub _pad: u32,
}

/// EFI memory types we care about.
/// Type 7 = EfiConventionalMemory (free RAM).
pub const EFI_CONVENTIONAL_MEMORY: u32 = 7;

/// Must match CoreOS_BootInfo in bootloader/main.c exactly.
#[repr(C, packed)]
pub struct CoreOS_BootInfo {
    pub fb_base: u64,
    pub fb_size: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,

    pub mmap: [MemMapEntry; 256],
    pub mmap_count: u32,
    pub _pad: u32,

    // Initial userspace ELF loaded by the bootloader.
    // user_elf_base == 0 means no binary was found.
    pub user_elf_base: u64,
    pub user_elf_size: u64,
}
// Keep the font here as it is a boot-time resource.
pub const FONT: &[u8] = include_bytes!("font.psfu");
