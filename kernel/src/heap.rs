use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

pub struct BumpAllocator;

const HEAP_SIZE: usize = 1024 * 1024; // 1MB

#[repr(align(16))]
struct AlignedHeap([u8; HEAP_SIZE]);

static mut HEAP: AlignedHeap = AlignedHeap([0; HEAP_SIZE]);
static mut NEXT: usize = 0;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align_mask = layout.align() - 1;
        let size = layout.size();

        let mut start = NEXT;

        // Align
        if start & align_mask != 0 {
            start = (start + align_mask) & !align_mask;
        }

        if start + size > HEAP_SIZE {
            return null_mut();
        }

        NEXT = start + size;
        HEAP.0.as_mut_ptr().add(start)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // no-op for bump allocator
    }
}
