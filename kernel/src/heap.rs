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
            crate::dbg_log!(
                "HEAP",
                "OOM! requested={} align={} next={}",
                size,
                layout.align(),
                NEXT
            );
            return null_mut();
        }

        NEXT = start + size;
        let heap_ptr = core::ptr::addr_of_mut!(HEAP.0) as *mut u8;
        let ptr = heap_ptr.add(start);
        crate::dbg_log!(
            "HEAP",
            "alloc size={} align={} -> ptr={:#x} (used={}/{})",
            size,
            layout.align(),
            ptr as usize,
            NEXT,
            HEAP_SIZE
        );
        ptr
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // no-op for bump allocator
    }
}
