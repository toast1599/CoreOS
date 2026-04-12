use core::arch::asm;

use uefi::boot::{self, MemoryType};
use uefi::mem::memory_map::MemoryMap;

use crate::bootinfo::{CoreOsBootInfo, CoreOsMemMapEntry, MAX_MMAP_ENTRIES};

pub fn capture_memory_map(boot_info: &mut CoreOsBootInfo) -> uefi::Result<()> {
    let map = boot::memory_map(MemoryType::LOADER_DATA)?;

    let mut stored = 0usize;
    for desc in map.entries() {
        if stored >= MAX_MMAP_ENTRIES {
            break;
        }
        boot_info.mmap[stored] = CoreOsMemMapEntry {
            physical_start: desc.phys_start,
            num_pages: desc.page_count,
            mem_type: desc.ty.0,
            _pad: 0,
        };
        stored += 1;
    }
    boot_info.mmap_count = stored as u32;
    Ok(())
}

pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nostack, preserves_flags));
    }
    ((hi as u64) << 32) | lo as u64
}
