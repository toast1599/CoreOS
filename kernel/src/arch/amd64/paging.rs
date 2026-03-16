use crate::boot::CoreOS_BootInfo;
use crate::mem::pmm::alloc_frame;
use core::ptr::addr_of;

const PTE_PRESENT: u64 = 1 << 0;
const PTE_WRITABLE: u64 = 1 << 1;
const PTE_USER: u64 = 1 << 2;
const PTE_HUGE: u64 = 1 << 7;
const PTE_NX: u64 = 1 << 63;

pub const PHYSICAL_OFFSET: usize = 0xFFFF800000000000;
pub static mut KERNEL_PML4: usize = 0;

#[derive(Clone, Copy)]
pub struct MapFlags {
    pub writable: bool,
    pub user: bool,
    pub executable: bool,
}

#[inline(always)]
pub fn p2v(phys: usize) -> usize {
    phys + PHYSICAL_OFFSET
}

#[inline(always)]
pub fn v2p(virt: usize) -> usize {
    if virt >= PHYSICAL_OFFSET {
        virt - PHYSICAL_OFFSET
    } else {
        // Fallback for kernel image which was linked at 0xFFFFFFFF80000000
        // with physical load address 0x0
        virt - 0xFFFFFFFF80000000
    }
}

unsafe fn alloc_table() -> usize {
    let addr = alloc_frame();
    if addr == 0 {
        panic!("paging: OOM allocating page table");
    }
    addr
}

#[inline]
unsafe fn write_entry(table_phys: usize, index: usize, value: u64) {
    let ptr = p2v(table_phys + index * 8) as *mut u64;
    ptr.write_volatile(value);
}

pub unsafe fn init(boot_info: *const CoreOS_BootInfo) {
    let pml4 = alloc_table();
    let k_pdpt = alloc_table();
    let dm_pdpt = alloc_table();
    let id_pdpt = alloc_table();

    // 0xFFFFFFFF80000000 -> PML4 511 (Kernel Map)
    write_entry(
        pml4,
        511,
        k_pdpt as u64 | PTE_PRESENT | PTE_WRITABLE,
    );

    // 0xFFFF800000000000 -> PML4 256 (Direct Map)
    write_entry(
        pml4,
        256,
        dm_pdpt as u64 | PTE_PRESENT | PTE_WRITABLE,
    );

    // 0x0000000000000000 -> PML4 0 (Identity map)
    write_entry(
        pml4,
        0,
        id_pdpt as u64 | PTE_PRESENT | PTE_WRITABLE,
    );

    // Map first 4GB into Direct Map AND Identity Map.
    for i in 0..4usize {
        let pd = alloc_table();
        write_entry(
            dm_pdpt,
            i,
            pd as u64 | PTE_PRESENT | PTE_WRITABLE,
        );
        write_entry(
            id_pdpt,
            i,
            pd as u64 | PTE_PRESENT | PTE_WRITABLE,
        );

        // Also map the first 1GB into Kernel Map (k_pdpt[510])
        if i == 0 {
            write_entry(
                k_pdpt,
                510,
                pd as u64 | PTE_PRESENT | PTE_WRITABLE,
            );
        }

        for j in 0..512usize {
            let phys = ((i * 512 + j) as u64) * (2 * 1024 * 1024);
            write_entry(
                pd,
                j,
                phys | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE,
            );
        }
    }

    let fb_base = core::ptr::read_unaligned(addr_of!((*boot_info).fb_base)) as usize;
    let fb_size = core::ptr::read_unaligned(addr_of!((*boot_info).fb_size)) as usize;
    // fb_base inside boot_info was already p2v()'d by main.rs, so it's a VIRTUAL address!
    // But map_range_2mb needs physical start.
    let fb_phys = v2p(fb_base);
    if fb_phys >= 4 * 1024 * 1024 * 1024 {
        map_range_2mb(pml4, fb_base, fb_phys, fb_size);
    }

    KERNEL_PML4 = pml4;

    core::arch::asm!("mov cr3, {}", in(reg) pml4 as u64, options(nostack, nomem));
    crate::dbg_log!(
        "PAGING",
        "direct map & kernel map active (PML4 @ {:#x})",
        pml4
    );
}

unsafe fn map_range_2mb(pml4: usize, virt_start: usize, phys_start: usize, size: usize) {
    let two_mb: usize = 2 * 1024 * 1024;
    let mut virt = virt_start & !(two_mb - 1);
    let mut phys = phys_start & !(two_mb - 1);
    let virt_end = (virt_start + size + two_mb - 1) & !(two_mb - 1);

    while virt < virt_end {
        let pml4_i = (virt >> 39) & 0x1FF;
        let pdpt_i = (virt >> 30) & 0x1FF;
        let pd_i = (virt >> 21) & 0x1FF;

        let pml4_ptr = p2v(pml4 + pml4_i * 8) as *mut u64;
        if *pml4_ptr == 0 {
            let t = alloc_table();
            pml4_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE);
        }
        let pdpt = (*pml4_ptr & !0xFFF) as usize;

        let pdpt_ptr = p2v(pdpt + pdpt_i * 8) as *mut u64;
        if *pdpt_ptr == 0 {
            let t = alloc_table();
            pdpt_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE);
        }
        let pd = (*pdpt_ptr & !0xFFF) as usize;

        let pd_ptr = p2v(pd + pd_i * 8) as *mut u64;
        pd_ptr.write_volatile(phys as u64 | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE);

        virt += two_mb;
        phys += two_mb;
    }
}

pub unsafe fn map_page_in(pml4: usize, virt: usize, phys: usize, flags: MapFlags) {
    let pml4_i = (virt >> 39) & 0x1FF;
    let pdpt_i = (virt >> 30) & 0x1FF;
    let pd_i = (virt >> 21) & 0x1FF;
    let pt_i = (virt >> 12) & 0x1FF;

    let pml4_ptr = p2v(pml4 + pml4_i * 8) as *mut u64;
    if *pml4_ptr == 0 {
        let t = alloc_table();
        let user = if flags.user { PTE_USER } else { 0 };
        pml4_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE | user);
    }
    let pdpt = (*pml4_ptr & !0xFFF) as usize;

    let pdpt_ptr = p2v(pdpt + pdpt_i * 8) as *mut u64;
    if *pdpt_ptr == 0 {
        let t = alloc_table();
        let user = if flags.user { PTE_USER } else { 0 };
        pdpt_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE | user);
    }
    let pd = (*pdpt_ptr & !0xFFF) as usize;

    let pd_ptr = p2v(pd + pd_i * 8) as *mut u64;
    if *pd_ptr == 0 {
        let t = alloc_table();
        let user = if flags.user { PTE_USER } else { 0 };
        pd_ptr.write_volatile(t as u64 | PTE_PRESENT | PTE_WRITABLE | user);
    }
    // Check if it's already a huge page
    if (*pd_ptr & PTE_HUGE) != 0 {
        return;
    }
    let pt = (*pd_ptr & !0xFFF) as usize;

    let pt_ptr = p2v(pt + pt_i * 8) as *mut u64;
    let mut entry = phys as u64 | PTE_PRESENT;
    if flags.writable {
        entry |= PTE_WRITABLE;
    }
    if flags.user {
        entry |= PTE_USER;
    }
    if !flags.executable {
        entry |= PTE_NX;
    }
    pt_ptr.write_volatile(entry);

    let cr3: u64;
    core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
    if (cr3 as usize & !0xFFF) == pml4 {
        core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, nomem));
    }
}

pub unsafe fn translate_page(pml4: usize, virt: usize) -> Option<usize> {
    let pml4_i = (virt >> 39) & 0x1FF;
    let pdpt_i = (virt >> 30) & 0x1FF;
    let pd_i = (virt >> 21) & 0x1FF;
    let pt_i = (virt >> 12) & 0x1FF;

    let pml4e = (p2v(pml4 + pml4_i * 8) as *const u64).read_volatile();
    if (pml4e & PTE_PRESENT) == 0 {
        return None;
    }
    let pdpt = (pml4e & !0xFFF) as usize;

    let pdpte = (p2v(pdpt + pdpt_i * 8) as *const u64).read_volatile();
    if (pdpte & PTE_PRESENT) == 0 {
        return None;
    }
    let pd = (pdpte & !0xFFF) as usize;

    let pde = (p2v(pd + pd_i * 8) as *const u64).read_volatile();
    if (pde & PTE_PRESENT) == 0 {
        return None;
    }
    if (pde & PTE_HUGE) != 0 {
        let base = (pde as usize) & !((2 * 1024 * 1024) - 1);
        return Some(base + (virt & ((2 * 1024 * 1024) - 1)));
    }
    let pt = (pde & !0xFFF) as usize;

    let pte = (p2v(pt + pt_i * 8) as *const u64).read_volatile();
    if (pte & PTE_PRESENT) == 0 {
        return None;
    }
    Some((pte as usize) & !0xFFF)
}

pub unsafe fn clone_kernel_address_space() -> usize {
    let new_pml4 = alloc_frame();
    core::ptr::write_bytes(p2v(new_pml4) as *mut u8, 0, 4096);

    let src = p2v(KERNEL_PML4) as *const u64;
    let dst = p2v(new_pml4) as *mut u64;

    // copy kernel half
    for i in 256..512 {
        dst.add(i).write(src.add(i).read());
    }

    new_pml4
}
