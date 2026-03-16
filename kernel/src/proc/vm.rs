use super::{current_process, current_process_mut, VmRegion};

fn overlaps(a_start: usize, a_len: usize, b_start: usize, b_len: usize) -> bool {
    let a_end = a_start.saturating_add(a_len);
    let b_end = b_start.saturating_add(b_len);
    a_start < b_end && b_start < a_end
}

pub unsafe fn region_conflicts(start: usize, len: usize) -> bool {
    let Some(p) = current_process() else {
        return true;
    };
    p.vmas
        .iter()
        .any(|v| v.in_use && overlaps(start, len, v.start, v.len))
}

pub unsafe fn alloc_vma(start: usize, len: usize, prot: u32, flags: u32) -> bool {
    let Some(p) = current_process_mut() else {
        return false;
    };
    for vma in &mut p.vmas {
        if !vma.in_use {
            *vma = VmRegion {
                start,
                len,
                prot,
                flags,
                in_use: true,
            };
            return true;
        }
    }
    false
}

pub unsafe fn find_vma_exact_mut(start: usize, len: usize) -> Option<&'static mut VmRegion> {
    let p = current_process_mut()?;
    p.vmas.iter_mut().find(|v| v.in_use && v.start == start && v.len == len)
}

pub unsafe fn reserve_mmap_base(len: usize) -> Option<usize> {
    let p = current_process_mut()?;
    let start = (p.next_mmap_base + 0xFFF) & !0xFFF;
    let end = start.checked_add(len)?;
    p.next_mmap_base = end;
    Some(start)
}
