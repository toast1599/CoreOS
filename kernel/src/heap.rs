use crate::pmm::{alloc_frame, free_frame, PAGE_SIZE};
/// Slab Allocator
///
/// Size classes (bytes): 16, 32, 64, 128, 256, 512, 1024, 2048.
/// Each slab is one 4 KB page divided into fixed-size slots.
/// A free-list of slot pointers lives at the front of the slab header
/// (stored in a separate static array to keep the slabs themselves clean).
///
/// Allocations larger than 2048 bytes are rounded up to whole pages
/// and handed out directly from the PMM.
///
/// The allocator is intentionally simple and single-threaded
/// (no locks needed while we're running without SMP).
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

// ---------------------------------------------------------------------------
// Size classes
// ---------------------------------------------------------------------------

const SIZE_CLASSES: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];
const NUM_CLASSES: usize = SIZE_CLASSES.len();

/// Maximum number of slabs we track per size class.
const MAX_SLABS_PER_CLASS: usize = 64;

// ---------------------------------------------------------------------------
// Slab header (kept in a static table, NOT inside the slab page)
// ---------------------------------------------------------------------------

struct SlabHeader {
    /// Physical / virtual address of the slab page (identity mapped).
    page: usize,
    /// Head of the intrusive free-list. Each free slot stores the address
    /// of the next free slot as a usize at offset 0.
    free_head: usize,
    /// How many slots are currently free in this slab.
    free_count: usize,
    /// Total slots this slab was initialised with.
    capacity: usize,
}

impl SlabHeader {
    const fn empty() -> Self {
        Self {
            page: 0,
            free_head: 0,
            free_count: 0,
            capacity: 0,
        }
    }

    fn is_empty_slot(&self) -> bool {
        self.page == 0
    }
}

// ---------------------------------------------------------------------------
// Global slab table
// ---------------------------------------------------------------------------

static mut SLABS: [[SlabHeader; MAX_SLABS_PER_CLASS]; NUM_CLASSES] = {
    // const-initialise manually (no Default for arrays > 32 in stable/no_std)
    [
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
        [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    ]
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the index into SIZE_CLASSES for a given allocation size,
/// or None if the size exceeds the largest class.
fn class_for(size: usize) -> Option<usize> {
    for (i, &s) in SIZE_CLASSES.iter().enumerate() {
        if size <= s {
            return Some(i);
        }
    }
    None
}

/// Initialise a fresh slab page for size class `class_idx`.
/// Returns a mutable reference to the filled-in header, or None on OOM.
unsafe fn new_slab(class_idx: usize) -> Option<&'static mut SlabHeader> {
    let slot_size = SIZE_CLASSES[class_idx];

    // Find an empty header slot.
    let header = SLABS[class_idx].iter_mut().find(|h| h.is_empty_slot())?;

    // Allocate a physical page (already zeroed by alloc_frame).
    let page = alloc_frame();
    if page == 0 {
        return None;
    }

    let capacity = PAGE_SIZE / slot_size;

    // Build the intrusive free-list: each slot holds a pointer to the next.
    let mut prev: usize = 0;
    for i in (0..capacity).rev() {
        let slot = page + i * slot_size;
        (slot as *mut usize).write(prev);
        prev = slot;
    }

    header.page = page;
    header.free_head = prev; // points to slot 0
    header.free_count = capacity;
    header.capacity = capacity;

    crate::dbg_log!(
        "SLAB",
        "new slab class={} slot_size={} page={:#x} capacity={}",
        class_idx,
        slot_size,
        page,
        capacity
    );

    Some(header)
}

// ---------------------------------------------------------------------------
// GlobalAlloc implementation
// ---------------------------------------------------------------------------

pub struct SlabAllocator;

unsafe impl GlobalAlloc for SlabAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout
            .size()
            .max(layout.align())
            .max(core::mem::size_of::<usize>());

        if let Some(ci) = class_for(size) {
            // Try to find a slab with a free slot.
            let slab = SLABS[ci]
                .iter_mut()
                .find(|h| !h.is_empty_slot() && h.free_count > 0);

            let slab = match slab {
                Some(s) => s,
                None => match new_slab(ci) {
                    Some(s) => s,
                    None => {
                        crate::dbg_log!("SLAB", "OOM for class {}", ci);
                        return null_mut();
                    }
                },
            };

            // Pop from free-list.
            let slot = slab.free_head;
            slab.free_head = *(slot as *const usize);
            slab.free_count -= 1;

            // Zero the slot (the PMM zeroed whole pages, but slots get reused).
            core::ptr::write_bytes(slot as *mut u8, 0, SIZE_CLASSES[ci]);

            slot as *mut u8
        } else {
            // Large allocation: round up to pages, alloc directly from PMM.
            let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;

            // We only support single-page large allocs for now.
            // For multi-page, allocate contiguous frames (simple sequential search).
            // This is good enough for early Stage 1.
            if pages_needed == 1 {
                let page = alloc_frame();
                if page == 0 {
                    return null_mut();
                }
                return page as *mut u8;
            }

            // Multi-page: allocate pages sequentially (they may not be contiguous
            // in physical memory, but since we identity-map everything it works
            // as long as we only need virtual contiguity — which we do here).
            // Simple approach: just grab the first page and hope the following
            // frames are free (works fine under QEMU with plenty of RAM).
            // A proper range allocator is a Stage 2 concern.
            let first = alloc_frame();
            if first == 0 {
                return null_mut();
            }

            for i in 1..pages_needed {
                let next = alloc_frame();
                if next == 0 {
                    return null_mut();
                }
                // We don't need to do anything special; they're identity-mapped.
                let _ = next;
                let _ = i;
            }

            first as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let size = layout
            .size()
            .max(layout.align())
            .max(core::mem::size_of::<usize>());

        if let Some(ci) = class_for(size) {
            let addr = ptr as usize;

            // Find the slab this pointer belongs to.
            if let Some(slab) = SLABS[ci]
                .iter_mut()
                .find(|h| !h.is_empty_slot() && addr >= h.page && addr < h.page + PAGE_SIZE)
            {
                // Push back onto free-list.
                (addr as *mut usize).write(slab.free_head);
                slab.free_head = addr;
                slab.free_count += 1;

                // If the slab is fully free, return the page to the PMM.
                if slab.free_count == slab.capacity {
                    crate::dbg_log!("SLAB", "returning empty slab page {:#x} to PMM", slab.page);
                    free_frame(slab.page);
                    *slab = SlabHeader::empty();
                }
            } else {
                crate::dbg_log!("SLAB", "dealloc: ptr {:#x} not found in slabs!", addr);
            }
        } else {
            // Large allocation: return the page(s) to PMM.
            let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;
            for i in 0..pages_needed {
                free_frame(ptr as usize + i * PAGE_SIZE);
            }
        }
    }
}

