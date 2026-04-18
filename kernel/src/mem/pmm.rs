/// Physical Memory Manager (PMM) — bitmap allocator.
use crate::boot::{CoreOS_BootInfo, EFI_CONVENTIONAL_MEMORY};
use core::ptr::addr_of;

pub const PAGE_SIZE: usize = 4096;

const MAX_FRAMES: usize = 1024 * 1024; // 4 GB
const BITMAP_BYTES: usize = MAX_FRAMES / 8;

// ---------------------------------------------------------------------------
// Simple spinlock (no_std)
// ---------------------------------------------------------------------------

use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    data: core::cell::UnsafeCell<T>,
}

unsafe impl<T> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: core::cell::UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }
}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> core::ops::Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> core::ops::DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// PMM State
// ---------------------------------------------------------------------------

struct PMMState {
    bitmap: [u8; BITMAP_BYTES],
    total_frames: usize,
    free_frames: usize,
}

impl PMMState {
    const fn new() -> Self {
        Self {
            bitmap: [0xFF; BITMAP_BYTES],
            total_frames: 0,
            free_frames: 0,
        }
    }

    #[inline]
    fn set_used(&mut self, f: usize) {
        self.bitmap[f / 8] |= 1 << (f % 8);
    }

    #[inline]
    fn set_free(&mut self, f: usize) {
        self.bitmap[f / 8] &= !(1 << (f % 8));
    }

    #[inline]
    fn is_free(&self, f: usize) -> bool {
        (self.bitmap[f / 8] & (1 << (f % 8))) == 0
    }
}

// Global PMM instance
static PMM: SpinLock<PMMState> = SpinLock::new(PMMState::new());

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub unsafe fn init(boot_info: *const CoreOS_BootInfo) {
    let mut pmm = PMM.lock();

    crate::serial_fmt!("pmm::init begin\n");

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
            if f < MAX_FRAMES && !pmm.is_free(f) {
                pmm.set_free(f);
                pmm.free_frames += 1;
                pmm.total_frames += 1;
            }
        }
    }

    let kernel_phys_base =
        core::ptr::read_unaligned(addr_of!((*boot_info).kernel_phys_base)) as usize;

    let kernel_alloc_size =
        core::ptr::read_unaligned(addr_of!((*boot_info).kernel_alloc_size)) as usize;

    let start = kernel_phys_base / PAGE_SIZE;
    let end = (kernel_phys_base + kernel_alloc_size + PAGE_SIZE - 1) / PAGE_SIZE;

    for f in start..end {
        if f < MAX_FRAMES && pmm.is_free(f) {
            pmm.set_used(f);
            pmm.free_frames -= 1;
        }
    }

    if pmm.is_free(0) {
        pmm.set_used(0);
        pmm.free_frames = pmm.free_frames.saturating_sub(1);
    }

    crate::dbg_log!(
        "PMM",
        "init done: {} free frames ({} MB free)",
        pmm.free_frames,
        (pmm.free_frames * PAGE_SIZE) / (1024 * 1024)
    );
}

// ---------------------------------------------------------------------------
// Allocation
// ---------------------------------------------------------------------------

pub fn alloc_frame() -> usize {
    let mut pmm = PMM.lock();

    for f in 1..MAX_FRAMES {
        if pmm.is_free(f) {
            pmm.set_used(f);
            pmm.free_frames = pmm.free_frames.saturating_sub(1);

            let addr = f * PAGE_SIZE;

            unsafe {
                core::ptr::write_bytes(
                    crate::arch::amd64::paging::p2v(addr) as *mut u8,
                    0,
                    PAGE_SIZE,
                );
            }

            return addr;
        }
    }

    crate::dbg_log!("PMM", "OOM: no free frames!");
    0
}

pub fn alloc_frames(count: usize) -> usize {
    if count == 0 {
        return 0;
    }

    if count == 1 {
        return alloc_frame();
    }

    let mut pmm = PMM.lock();

    let mut consecutive = 0;
    let mut start_frame = 0;

    for f in 1..MAX_FRAMES {
        if pmm.is_free(f) {
            if consecutive == 0 {
                start_frame = f;
            }
            consecutive += 1;

            if consecutive == count {
                for i in 0..count {
                    pmm.set_used(start_frame + i);
                }

                pmm.free_frames = pmm.free_frames.saturating_sub(count);

                let addr = start_frame * PAGE_SIZE;

                unsafe {
                    core::ptr::write_bytes(
                        crate::arch::amd64::paging::p2v(addr) as *mut u8,
                        0,
                        count * PAGE_SIZE,
                    );
                }

                return addr;
            }
        } else {
            consecutive = 0;
        }
    }

    crate::dbg_log!("PMM", "OOM: no {} contiguous frames!", count);
    0
}

// ---------------------------------------------------------------------------
// Free
// ---------------------------------------------------------------------------

pub fn free_frame(phys_addr: usize) {
    let mut pmm = PMM.lock();

    let f = phys_addr / PAGE_SIZE;

    if f == 0 || f >= MAX_FRAMES {
        return;
    }

    if !pmm.is_free(f) {
        pmm.set_free(f);
        pmm.free_frames += 1;
    }
}

// ---------------------------------------------------------------------------
// Info
// ---------------------------------------------------------------------------

pub fn free_bytes() -> usize {
    PMM.lock().free_frames * PAGE_SIZE
}

pub fn total_bytes() -> usize {
    PMM.lock().total_frames * PAGE_SIZE
}

