use crate::boot::CoreOS_BootInfo;

pub fn validate_mmap(boot_info: &CoreOS_BootInfo) {
    let count = boot_info.mmap_count as usize;

    // Basic sanity
    if count == 0 || count > 256 {
        panic!("Invalid mmap count: {}", count);
    }

    for i in 0..count {
        let entry = &boot_info.mmap[i];

        let start = entry.physical_start;
        let size = entry.num_pages * 4096;

        // 1. Alignment check
        if start % 4096 != 0 {
            panic!("Unaligned region at {:#x}", start);
        }

        // 2. Zero-size check
        if entry.num_pages == 0 {
            panic!("Zero-sized region at {:#x}", start);
        }

        let end = start + size;

        // 3. Overlap check
        for j in (i + 1)..count {
            let other = &boot_info.mmap[j];
            let o_start = other.physical_start;
            let o_end = o_start + other.num_pages * 4096;

            if start < o_end && o_start < end {
                panic!(
                    "Memory overlap: [{:#x}, {:#x}) with [{:#x}, {:#x})",
                    start, end, o_start, o_end
                );
            }
        }
    }
}
