use crate::boot::CoreOS_BootInfo;
/// Paging — 4-level PML4 identity map for first 4 GB + framebuffer.
use crate::pmm::alloc_frame;
use core::ptr::addr_of;

const PTE_PRESENT: u64 = 1 << 0;
const PTE_WRITABLE: u64 = 1 << 1;
const PTE_HUGE: u64 = 1 << 7;

unsafe fn alloc_table() -> usize {
    let addr = alloc_frame();
    if addr == 0 {
        panic!("paging: OOM allocating page table");
    }
    addr
}

#[inline]
unsafe fn write_entry(table_phys: usize, index: usize, value: u64) {
    let ptr = (table_phys + index * 8) as *mut u64;
    ptr.write_volatile(value);
}

pub unsafe fn init(boot_info: *const CoreOS_BootInfo) {
    let pml4 = alloc_table();
    let pdpt = alloc_table();
    write_entry(pml4, 0, pdpt as u64 | PTE_PRESENT | PTE_WRITABLE);

    for pdpt_i in 0..4usize {
        let pd = alloc_table();
        write_entry(pdpt, pdpt_i, pd as u64 | PTE_PRESENT | PTE_WRITABLE);
        for pd_i in 0..512usize {
            let phys = ((pdpt_i * 512 + pd_i) as u64) * (2 * 1024 * 1024);
            write_entry(pd, pd_i, phys | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE);
        }
    }

    let fb_base = core::ptr::read_unaligned(addr_of!((*boot_info).fb_base)) as usize;
    let fb_size = core::ptr::read_unaligned(addr_of!((*boot_info).fb_size)) as usize;
    if fb_base >= 4 * 1024 * 1024 * 1024 {
        map_range_2mb(pml4, fb_base, fb_base + fb_size);
    }

    core::arch::asm!("mov cr3, {}", in(reg) pml4 as u64, options(nostack, nomem));
    crate::dbg_log!("PAGING", "identity map active (PML4 @ {:#x})", pml4);
}

unsafe fn map_range_2mb(pml4: usize, start: usize, end: usize) {
    let two_mb: usize = 2 * 1024 * 1024;
    let mut addr = start & !(two_mb - 1);

    while addr < end {
        let pml4_i = (addr >> 39) & 0x1FF;
        let pdpt_i = (addr >> 30) & 0x1FF;
        let pd_i = (addr >> 21) & 0x1FF;

        let pml4_ptr = (pml4 + pml4_i * 8) as *mut u64;
        if *pml4_ptr == 0 {
            let t = alloc_table();
            pml4_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE);
        }
        let pdpt = (*pml4_ptr & !0xFFF) as usize;

        let pdpt_ptr = (pdpt + pdpt_i * 8) as *mut u64;
        if *pdpt_ptr == 0 {
            let t = alloc_table();
            pdpt_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE);
        }
        let pd = (*pdpt_ptr & !0xFFF) as usize;

        let pd_ptr = (pd + pd_i * 8) as *mut u64;
        pd_ptr.write_volatile(addr as u64 | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE);

        addr += two_mb;
    }
}

