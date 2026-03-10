/// Physical Memory Manager (PMM) — bitmap allocator.
use crate::boot::{CoreOS_BootInfo, EFI_CONVENTIONAL_MEMORY};
use core::ptr::addr_of;

pub const PAGE_SIZE: usize = 4096;

const MAX_FRAMES: usize = 1024 * 1024; // 4 GB
const BITMAP_BYTES: usize = MAX_FRAMES / 8;

static mut BITMAP: [u8; BITMAP_BYTES] = [0xFF; BITMAP_BYTES];
static mut TOTAL_FRAMES: usize = 0;
static mut FREE_FRAMES: usize = 0;

#[inline]
unsafe fn set_used(f: usize) {
    BITMAP[f / 8] |= 1 << (f % 8);
}
#[inline]
unsafe fn set_free(f: usize) {
    BITMAP[f / 8] &= !(1 << (f % 8));
}
#[inline]
unsafe fn is_free(f: usize) -> bool {
    (BITMAP[f / 8] & (1 << (f % 8))) == 0
}

pub unsafe fn init(boot_info: *const CoreOS_BootInfo, kernel_end_phys: usize) {
    let mmap_count = core::ptr::read_unaligned(addr_of!((*boot_info).mmap_count)) as usize;

    for i in 0..mmap_count {
        let entry = core::ptr::read_unaligned(
            addr_of!((*boot_info).mmap)
                .cast::<u8>()
                .add(i * core::mem::size_of::<crate::boot::MemMapEntry>())
                as *const crate::boot::MemMapEntry,
        );
        if entry.mem_type != EFI_CONVENTIONAL_MEMORY {
            continue;
        }

        let start_frame = (entry.physical_start as usize) / PAGE_SIZE;
        let frame_count = entry.num_pages as usize;
        for f in start_frame..start_frame + frame_count {
            if f < MAX_FRAMES {
                set_free(f);
                FREE_FRAMES += 1;
                TOTAL_FRAMES += 1;
            }
        }
    }

    let kernel_start_frame = 0x100000 / PAGE_SIZE;
    let kernel_end_frame = (kernel_end_phys + PAGE_SIZE - 1) / PAGE_SIZE;
    for f in kernel_start_frame..kernel_end_frame {
        if f < MAX_FRAMES && is_free(f) {
            set_used(f);
            FREE_FRAMES -= 1;
        }
    }

    set_used(0); // null page always used

    crate::dbg_log!(
        "PMM",
        "init done: {} free frames ({} MB free)",
        FREE_FRAMES,
        (FREE_FRAMES * PAGE_SIZE) / (1024 * 1024)
    );
}

pub unsafe fn alloc_frame() -> usize {
    for f in 1..MAX_FRAMES {
        if is_free(f) {
            set_used(f);
            if FREE_FRAMES > 0 {
                FREE_FRAMES -= 1;
            }
            let addr = f * PAGE_SIZE;
            core::ptr::write_bytes(addr as *mut u8, 0, PAGE_SIZE);
            return addr;
        }
    }
    crate::dbg_log!("PMM", "OOM: no free frames!");
    0
}

pub unsafe fn free_frame(phys_addr: usize) {
    let f = phys_addr / PAGE_SIZE;
    if f == 0 || f >= MAX_FRAMES {
        return;
    }
    if !is_free(f) {
        set_free(f);
        FREE_FRAMES += 1;
    }
}

pub fn free_bytes() -> usize {
    unsafe { FREE_FRAMES * PAGE_SIZE }
}

