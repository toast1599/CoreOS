/// Physical Memory Manager (PMM)
///
/// Manages physical 4KB page frames using a bitmap.
/// One bit per frame: 0 = free, 1 = used.
///
/// The bitmap itself lives in the first usable conventional-memory
/// region that is large enough to hold it.
use crate::boot::{CoreOS_BootInfo, EFI_CONVENTIONAL_MEMORY};
use core::ptr::addr_of;

pub const PAGE_SIZE: usize = 4096;

// Maximum physical memory we will track: 4 GB.
// That requires 4GB / 4KB = 1M frames = 128KB of bitmap.
const MAX_FRAMES: usize = 1024 * 1024; // 1M frames = 4 GB
const BITMAP_BYTES: usize = MAX_FRAMES / 8; // 128 KB

/// The bitmap is a static array so it lives in BSS.
/// Frame N is represented by bit (N % 8) of byte (N / 8).
/// 0 = free, 1 = used.
static mut BITMAP: [u8; BITMAP_BYTES] = [0xFF; BITMAP_BYTES]; // start all used
static mut TOTAL_FRAMES: usize = 0;
static mut FREE_FRAMES: usize = 0;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[inline]
unsafe fn set_used(frame: usize) {
    BITMAP[frame / 8] |= 1 << (frame % 8);
}

#[inline]
unsafe fn set_free(frame: usize) {
    BITMAP[frame / 8] &= !(1 << (frame % 8));
}

#[inline]
unsafe fn is_free(frame: usize) -> bool {
    (BITMAP[frame / 8] & (1 << (frame % 8))) == 0
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the PMM from the UEFI memory map inside `boot_info`.
///
/// After this call:
///  - All frames are marked used by default.
///  - Frames that fall inside EfiConventionalMemory regions are marked free.
///  - Frames used by the kernel image (0x100000 … kernel_end) are re-marked used.
pub unsafe fn init(boot_info: *const CoreOS_BootInfo, kernel_end_phys: usize) {
    let mmap_count = core::ptr::read_unaligned(addr_of!((*boot_info).mmap_count)) as usize;

    // 1. Mark all conventional (free) memory regions as free.
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

    // 2. Re-mark frames used by the kernel (0x100000 .. kernel_end).
    let kernel_start_frame = 0x100000 / PAGE_SIZE;
    let kernel_end_frame = (kernel_end_phys + PAGE_SIZE - 1) / PAGE_SIZE;

    for f in kernel_start_frame..kernel_end_frame {
        if f < MAX_FRAMES && is_free(f) {
            set_used(f);
            FREE_FRAMES -= 1;
        }
    }

    // 3. Always mark the null page (frame 0) as used.
    set_used(0);

    crate::dbg_log!(
        "PMM",
        "init done: {} free frames ({} MB free)",
        FREE_FRAMES,
        (FREE_FRAMES * PAGE_SIZE) / (1024 * 1024)
    );
}

/// Allocate one physical page frame. Returns the physical address or 0 on OOM.
pub unsafe fn alloc_frame() -> usize {
    for f in 1..MAX_FRAMES {
        if is_free(f) {
            set_used(f);
            if FREE_FRAMES > 0 {
                FREE_FRAMES -= 1;
            }
            let addr = f * PAGE_SIZE;
            // Zero the frame before handing it out.
            core::ptr::write_bytes(addr as *mut u8, 0, PAGE_SIZE);
            return addr;
        }
    }
    crate::dbg_log!("PMM", "OOM: no free frames!");
    0
}

/// Free a physical page frame previously returned by `alloc_frame`.
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

/// How many bytes of physical RAM are currently free.
pub fn free_bytes() -> usize {
    unsafe { FREE_FRAMES * PAGE_SIZE }
}

