use super::pmm::{alloc_frame, free_frame, PAGE_SIZE};
/// Slab Allocator
///
/// Size classes (bytes): 16, 32, 64, 128, 256, 512, 1024, 2048.
/// Each slab is one 4 KB page divided into fixed-size slots.
/// Allocations larger than 2048 bytes are rounded up to whole pages
/// and handed out directly from the PMM.
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

const SIZE_CLASSES: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];
const NUM_CLASSES: usize = SIZE_CLASSES.len();
const MAX_SLABS_PER_CLASS: usize = 64;

struct SlabHeader {
    page: usize,
    free_head: usize,
    free_count: usize,
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

static mut SLABS: [[SlabHeader; MAX_SLABS_PER_CLASS]; NUM_CLASSES] = [
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
    [const { SlabHeader::empty() }; MAX_SLABS_PER_CLASS],
];

fn class_for(size: usize) -> Option<usize> {
    SIZE_CLASSES
        .iter()
        .enumerate()
        .find(|(_, &s)| size <= s)
        .map(|(i, _)| i)
}

unsafe fn new_slab(class_idx: usize) -> Option<&'static mut SlabHeader> {
    let slot_size = SIZE_CLASSES[class_idx];
    let header = SLABS[class_idx].iter_mut().find(|h| h.is_empty_slot())?;

    let page = alloc_frame();
    if page == 0 {
        return None;
    }

    let capacity = PAGE_SIZE / slot_size;
    let mut prev: usize = 0;
    for i in (0..capacity).rev() {
        let slot_virt = crate::arch::amd64::paging::p2v(page + i * slot_size);
        (slot_virt as *mut usize).write(prev);
        prev = slot_virt;
    }

    header.page = page;
    header.free_head = prev;
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

pub struct SlabAllocator;

unsafe impl GlobalAlloc for SlabAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout
            .size()
            .max(layout.align())
            .max(core::mem::size_of::<usize>());

        if let Some(ci) = class_for(size) {
            core::arch::asm!("cli", options(nomem, nostack));
            let res = (|| {
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

                let slot = slab.free_head;
                slab.free_head = *(slot as *const usize);
                slab.free_count -= 1;
                core::ptr::write_bytes(slot as *mut u8, 0, SIZE_CLASSES[ci]);
                slot as *mut u8
            })();
            core::arch::asm!("sti", options(nomem, nostack));
            res
        } else {
            let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;
            core::arch::asm!("cli", options(nomem, nostack));
            let first = super::pmm::alloc_frames(pages_needed);
            core::arch::asm!("sti", options(nomem, nostack));
            if first == 0 {
                return null_mut();
            }
            crate::arch::amd64::paging::p2v(first) as *mut u8
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
            let addr_virt = ptr as usize;
            let addr_phys = crate::arch::amd64::paging::v2p(addr_virt);
            core::arch::asm!("cli", options(nomem, nostack));
            if let Some(slab) = SLABS[ci]
                .iter_mut()
                .find(|h| !h.is_empty_slot() && addr_phys >= h.page && addr_phys < h.page + PAGE_SIZE)
            {
                (addr_virt as *mut usize).write(slab.free_head);
                slab.free_head = addr_virt;
                slab.free_count += 1;
                if slab.free_count == slab.capacity {
                    crate::dbg_log!("SLAB", "returning empty slab page {:#x} to PMM", slab.page);
                    free_frame(slab.page);
                    *slab = SlabHeader::empty();
                }
            } else {
                crate::dbg_log!("SLAB", "dealloc: ptr {:#x} not found in slabs!", addr_virt);
            }
            core::arch::asm!("sti", options(nomem, nostack));
        } else {
            let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;
            let addr_phys = crate::arch::amd64::paging::v2p(ptr as usize);
            core::arch::asm!("cli", options(nomem, nostack));
            for i in 0..pages_needed {
                free_frame(addr_phys + i * PAGE_SIZE);
            }
            core::arch::asm!("sti", options(nomem, nostack));
        }
    }
}

