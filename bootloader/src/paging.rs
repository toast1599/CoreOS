use core::arch::asm;
use core::ptr::write_bytes;

use uefi::boot::{self, AllocateType, MemoryType};

pub const KERNEL_VIRT_BASE: u64 = 0xFFFF_FFFF_8000_0000;
pub const KERNEL_ENTRY_VIRT_ADDR: u64 = KERNEL_VIRT_BASE + 0x0010_0000;

const PAGE_SIZE: usize = 0x1000;
const HUGE_PAGE_SIZE: u64 = 0x20_0000;
const PTE_PRESENT: u64 = 1 << 0;
const PTE_WRITABLE: u64 = 1 << 1;
const PTE_HUGE: u64 = 1 << 7;
const PTE_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

pub fn setup_page_tables(kernel_phys: u64, kernel_span: usize) -> uefi::Result<u64> {
    let pml4 = alloc_zeroed_table()?;
    let dm_pdpt = alloc_zeroed_table()?;
    let k_pdpt = alloc_zeroed_table()?;

    write_entry(pml4, 256, dm_pdpt | PTE_PRESENT | PTE_WRITABLE);
    write_entry(pml4, 511, k_pdpt | PTE_PRESENT | PTE_WRITABLE);

    for i in 0..4usize {
        let pd = alloc_zeroed_table()?;
        write_entry(dm_pdpt, i, pd | PTE_PRESENT | PTE_WRITABLE);

        for j in 0..512usize {
            let phys = ((i * 512 + j) as u64) * HUGE_PAGE_SIZE;
            write_entry(pd, j, phys | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE);
        }
    }

    map_4k_range(pml4, KERNEL_ENTRY_VIRT_ADDR, kernel_phys, kernel_span)?;

    Ok(pml4)
}

pub fn activate_page_tables(new_pml4: u64) {
    let old_pml4 = current_cr3() & !0xFFF;
    let old_entries = old_pml4 as *const u64;
    let new_entries = new_pml4 as *mut u64;

    for i in 0..512 {
        unsafe {
            if new_entries.add(i).read() == 0 {
                new_entries.add(i).write(old_entries.add(i).read());
            }
        }
    }

    unsafe {
        asm!("mov cr3, {}", in(reg) new_pml4, options(nostack, preserves_flags));
    }
}

fn current_cr3() -> u64 {
    let cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
    }
    cr3
}

fn map_4k_range(pml4: u64, virt_start: u64, phys_start: u64, size: usize) -> uefi::Result<()> {
    let mut offset = 0usize;
    while offset < size {
        let virt = virt_start + offset as u64;
        let phys = phys_start + offset as u64;

        let pml4_i = ((virt >> 39) & 0x1FF) as usize;
        let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
        let pd_i = ((virt >> 21) & 0x1FF) as usize;
        let pt_i = ((virt >> 12) & 0x1FF) as usize;

        let pdpt = ensure_child_table(pml4, pml4_i)?;
        let pd = ensure_child_table(pdpt, pdpt_i)?;
        let pt = ensure_child_table(pd, pd_i)?;
        write_entry(pt, pt_i, phys | PTE_PRESENT | PTE_WRITABLE);

        offset += PAGE_SIZE;
    }
    Ok(())
}

fn ensure_child_table(parent: u64, index: usize) -> uefi::Result<u64> {
    let entry = read_entry(parent, index);
    if (entry & PTE_PRESENT) != 0 {
        return Ok(entry & PTE_ADDR_MASK);
    }

    let child = alloc_zeroed_table()?;
    write_entry(parent, index, child | PTE_PRESENT | PTE_WRITABLE);
    Ok(child)
}

fn alloc_zeroed_table() -> uefi::Result<u64> {
    let ptr = boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)?;
    unsafe { write_bytes(ptr.as_ptr().cast::<u8>(), 0, PAGE_SIZE) };
    Ok(ptr.as_ptr() as u64)
}

fn read_entry(table: u64, index: usize) -> u64 {
    unsafe { (table as *const u64).add(index).read() }
}

fn write_entry(table: u64, index: usize, value: u64) {
    unsafe {
        (table as *mut u64).add(index).write(value);
    }
}
