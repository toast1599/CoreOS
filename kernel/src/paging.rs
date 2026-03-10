use crate::boot::CoreOS_BootInfo;
/// Paging
///
/// Sets up 4-level (PML4) identity mapping for:
///   - The first 4 GB of physical memory (covers kernel + heap + devices)
///   - The framebuffer region (may be above 4 GB on some systems)
///
/// We use 2 MB hugepages for the identity map to keep the page-table
/// footprint tiny (only 3 extra pages needed for the first 4 GB).
///
/// All page tables are allocated from the PMM one frame at a time.
use crate::pmm::alloc_frame;
use core::ptr::addr_of;

// Page-table entry flags
const PTE_PRESENT: u64 = 1 << 0;
const PTE_WRITABLE: u64 = 1 << 1;
const PTE_HUGE: u64 = 1 << 7; // 2 MB pages at PD level

/// Allocate a zeroed page for a page table and return its physical address.
unsafe fn alloc_table() -> usize {
    let addr = alloc_frame();
    if addr == 0 {
        panic!("paging: OOM allocating page table");
    }
    addr
}

/// Write a u64 to a physical address (identity-mapped, so phys == virt here).
#[inline]
unsafe fn write_entry(table_phys: usize, index: usize, value: u64) {
    let ptr = (table_phys + index * 8) as *mut u64;
    ptr.write_volatile(value);
}

/// Set up paging and load the new CR3.
///
/// Call this AFTER pmm::init() so we can alloc page-table frames.
pub unsafe fn init(boot_info: *const CoreOS_BootInfo) {
    // -----------------------------------------------------------------------
    // Allocate the top-level PML4 table.
    // -----------------------------------------------------------------------
    let pml4 = alloc_table();

    // -----------------------------------------------------------------------
    // Identity-map the first 4 GB using 2 MB pages.
    //
    // PML4[0] -> PDPT
    // PDPT[0..3] -> PD[0..3]
    // Each PD has 512 entries × 2 MB = 1 GB per PDPT entry → 4 × 1 GB = 4 GB
    // -----------------------------------------------------------------------
    let pdpt = alloc_table();
    write_entry(pml4, 0, pdpt as u64 | PTE_PRESENT | PTE_WRITABLE);

    for pdpt_i in 0..4usize {
        let pd = alloc_table();
        write_entry(pdpt, pdpt_i, pd as u64 | PTE_PRESENT | PTE_WRITABLE);

        for pd_i in 0..512usize {
            let phys = ((pdpt_i * 512 + pd_i) as u64) * (2 * 1024 * 1024); // 2 MB steps
            write_entry(pd, pd_i, phys | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE);
        }
    }

    // -----------------------------------------------------------------------
    // If the framebuffer is outside the first 4 GB, map it too.
    // -----------------------------------------------------------------------
    let fb_base = core::ptr::read_unaligned(addr_of!((*boot_info).fb_base)) as usize;
    let fb_size = core::ptr::read_unaligned(addr_of!((*boot_info).fb_size)) as usize;

    if fb_base >= 4 * 1024 * 1024 * 1024 {
        map_range_2mb(pml4, fb_base, fb_base + fb_size);
    }

    // -----------------------------------------------------------------------
    // Load the new PML4 into CR3.
    // -----------------------------------------------------------------------
    core::arch::asm!(
        "mov cr3, {}",
        in(reg) pml4 as u64,
        options(nostack, nomem)
    );

    crate::dbg_log!("PAGING", "identity map active (PML4 @ {:#x})", pml4);
}

/// Map a physical address range using 2 MB hugepages.
/// `virt_start` == `phys_start` (identity map).
unsafe fn map_range_2mb(pml4: usize, start: usize, end: usize) {
    let two_mb: usize = 2 * 1024 * 1024;
    let mut addr = start & !(two_mb - 1); // align down

    while addr < end {
        let pml4_i = (addr >> 39) & 0x1FF;
        let pdpt_i = (addr >> 30) & 0x1FF;
        let pd_i = (addr >> 21) & 0x1FF;

        // Walk / allocate PML4 entry
        let pml4_ptr = (pml4 + pml4_i * 8) as *mut u64;
        if *pml4_ptr == 0 {
            let t = alloc_table();
            pml4_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE);
        }
        let pdpt = (*pml4_ptr & !0xFFF) as usize;

        // Walk / allocate PDPT entry
        let pdpt_ptr = (pdpt + pdpt_i * 8) as *mut u64;
        if *pdpt_ptr == 0 {
            let t = alloc_table();
            pdpt_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE);
        }
        let pd = (*pdpt_ptr & !0xFFF) as usize;

        // Set the 2 MB page entry
        let pd_ptr = (pd + pd_i * 8) as *mut u64;
        pd_ptr.write_volatile(addr as u64 | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE);

        addr += two_mb;
    }
}

