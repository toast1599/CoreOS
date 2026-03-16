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
    crate::serial_fmt!("pmm::init begin\n");
    let mmap_count = core::ptr::read_unaligned(addr_of!((*boot_info).mmap_count)) as usize;
    crate::serial_fmt!("pmm::init mmap_count={}\n", mmap_count);

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
        let mtype = entry.mem_type;
        let pstart = entry.physical_start;
        let npages = entry.num_pages;
        crate::dbg_log!(
            "PMM",
            "map entry {}: type={} start={:#x} pages={}",
            i,
            mtype,
            pstart,
            npages
        );
        for f in start_frame..start_frame + frame_count {
            if f < MAX_FRAMES && !is_free(f) {
                set_free(f);
                FREE_FRAMES += 1;
                TOTAL_FRAMES += 1;
            }
        }
    }
    crate::serial_fmt!("pmm::init mmap parsed\n");

    let kernel_start_frame = 0x100000 / PAGE_SIZE;
    let kernel_end_frame = (kernel_end_phys + PAGE_SIZE - 1) / PAGE_SIZE;
    for f in kernel_start_frame..kernel_end_frame {
        if f < MAX_FRAMES && is_free(f) {
            set_used(f);
            FREE_FRAMES -= 1;
        }
    }
    if is_free(0) {
        set_used(0); // null page always used
        FREE_FRAMES = FREE_FRAMES.saturating_sub(1);
    }
    crate::serial_fmt!("pmm::init kernel frames reserved\n");

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
            core::ptr::write_bytes(crate::arch::amd64::paging::p2v(addr) as *mut u8, 0, PAGE_SIZE);
            return addr;
        }
    }
    crate::dbg_log!("PMM", "OOM: no free frames!");
    0
}

pub unsafe fn alloc_frames(count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if count == 1 {
        return alloc_frame();
    }

    let mut consecutive = 0;
    let mut start_frame = 0;

    for f in 1..MAX_FRAMES {
        if is_free(f) {
            if consecutive == 0 {
                start_frame = f;
            }
            consecutive += 1;
            if consecutive == count {
                for i in 0..count {
                    set_used(start_frame + i);
                }
                if FREE_FRAMES >= count {
                    FREE_FRAMES -= count;
                }
                let addr = start_frame * PAGE_SIZE;
                core::ptr::write_bytes(crate::arch::amd64::paging::p2v(addr) as *mut u8, 0, count * PAGE_SIZE);
                return addr;
            }
        } else {
            consecutive = 0;
        }
    }
    crate::dbg_log!("PMM", "OOM: no {} contiguous free frames!", count);
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

/// Mark a specific physical frame (by address) as used in the bitmap.
/// Used by the ELF loader to reserve target virtual addresses.
/// Has no effect if the frame is already marked used.
#[allow(dead_code)]
pub unsafe fn mark_frame_used(phys_addr: usize) {
    let f = phys_addr / PAGE_SIZE;
    if f == 0 || f >= MAX_FRAMES {
        return;
    }
    if is_free(f) {
        set_used(f);
        if FREE_FRAMES > 0 {
            FREE_FRAMES -= 1;
        }
    }
}

/// Returns true if the given physical frame is free in the bitmap.
#[allow(dead_code)]
pub unsafe fn is_frame_free_at(phys_addr: usize) -> bool {
    let f = phys_addr / PAGE_SIZE;
    if f >= MAX_FRAMES {
        return false;
    }
    is_free(f)
}

pub fn free_bytes() -> usize {
    unsafe { FREE_FRAMES * PAGE_SIZE }
}
