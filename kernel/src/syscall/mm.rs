use crate::arch::paging;
use crate::mem::pmm;
use crate::proc;
use crate::proc::task;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};

const PROT_EXEC: u32 = 0x1;
const PROT_WRITE: u32 = 0x2;
const MAP_SHARED: u32 = 0x01;
const MAP_PRIVATE: u32 = 0x02;
const MAP_FIXED: u32 = 0x10;
const MAP_ANONYMOUS: u32 = 0x20;

#[repr(C)]
#[derive(Clone, Copy)]
struct MmapArgs {
    addr: u64,
    len: u64,
    prot: u32,
    flags: u32,
    fd: i32,
    off: i64,
}

fn map_flags_from_prot(prot: u32) -> paging::MapFlags {
    paging::MapFlags {
        writable: (prot & PROT_WRITE) != 0,
        user: true,
        executable: (prot & PROT_EXEC) != 0,
    }
}

pub unsafe fn brk(addr: u64) -> u64 {
    result::ret(brk_impl(addr))
}

unsafe fn brk_impl(addr: u64) -> SysResult {
    let current_brk = proc::current_brk();
    if addr == 0 {
        return result::ok(current_brk as u64);
    }
    let new_brk = addr as usize;
    if new_brk <= current_brk {
        proc::set_brk(new_brk);
        return result::ok(new_brk as u64);
    }

    let mut page = (current_brk + 0xFFF) & !0xFFF;
    let end = (new_brk + 0xFFF) & !0xFFF;

    while page < end {
        let frame = pmm::alloc_frame();
        if frame == 0 {
            crate::dbg_log!("BRK", "OOM");
            return result::ok(current_brk as u64);
        }
        paging::map_page_in(
            task::current_pml4(),
            page,
            frame,
            paging::MapFlags {
                writable: true,
                user: true,
                executable: false,
            },
        );
        core::ptr::write_bytes(paging::p2v(frame) as *mut u8, 0, 0x1000);
        page += 0x1000;
    }

    proc::set_brk(new_brk);
    result::ok(new_brk as u64)
}

pub unsafe fn mmap(args_ptr: u64) -> u64 {
    result::ret(mmap_impl(args_ptr))
}

unsafe fn mmap_impl(args_ptr: u64) -> SysResult {
    let args: MmapArgs = result::option(helpers::copy_struct_from_user(args_ptr), SysError::Fault)?;
    result::ensure(args.len != 0, SysError::Invalid)?;
    result::ensure((args.flags & (MAP_PRIVATE | MAP_SHARED)) != 0, SysError::Invalid)?;
    result::ensure((args.flags & MAP_SHARED) == 0, SysError::Unsupported)?;
    let anonymous = (args.flags & MAP_ANONYMOUS) != 0;
    if anonymous {
        result::ensure(args.fd == -1 && args.off == 0, SysError::Invalid)?;
    } else {
        result::ensure(args.fd >= 0, SysError::BadFd)?;
        result::ensure(args.off >= 0 && (args.off as usize & 0xFFF) == 0, SysError::Invalid)?;
    }

    let len = ((args.len as usize) + 0xFFF) & !0xFFF;
    let start = if args.addr == 0 {
        result::option(proc::reserve_mmap_base(len), SysError::Invalid)?
    } else {
        let addr = (args.addr as usize) & !0xFFF;
        result::ensure(
            (args.flags & MAP_FIXED) != 0 && !proc::region_conflicts(addr, len),
            SysError::Invalid,
        )?;
        addr
    };

    result::ensure(!proc::region_conflicts(start, len), SysError::Invalid)?;
    result::ensure(proc::alloc_vma(start, len, args.prot, args.flags), SysError::Invalid)?;

    let pml4 = task::current_pml4();
    let flags = map_flags_from_prot(args.prot);
    let file_seed = if anonymous {
        None
    } else {
        match proc::descriptor_info(args.fd as usize) {
            Some(proc::DescriptorInfo::File { file_idx, .. }) => {
                Some(crate::vfs::read_range(file_idx, args.off as usize, len).unwrap_or_default())
            }
            _ => return result::err(SysError::BadFd),
        }
    };

    let mut mapped = 0usize;
    for off in (0..len).step_by(0x1000) {
        let frame = pmm::alloc_frame();
        if frame == 0 {
            for undo in (0..mapped).step_by(0x1000) {
                if let Some(old_frame) = paging::unmap_page_in(pml4, start + undo) {
                    pmm::free_frame(old_frame);
                }
            }
            if let Some(vma) = proc::find_vma_exact_mut(start, len) {
                *vma = proc::VmRegion::empty();
            }
            return result::err(SysError::NoMem);
        }
        paging::map_page_in(pml4, start + off, frame, flags);
        core::ptr::write_bytes(paging::p2v(frame) as *mut u8, 0, 0x1000);
        if let Some(bytes) = file_seed.as_ref() {
            let src = off.min(bytes.len());
            let end = (off + 0x1000).min(bytes.len());
            if src < end {
                core::ptr::copy_nonoverlapping(
                    bytes[src..end].as_ptr(),
                    (start + off) as *mut u8,
                    end - src,
                );
            }
        }
        mapped += 0x1000;
    }
    result::ok(start as u64)
}

pub unsafe fn munmap(addr: u64, len: u64) -> u64 {
    result::ret(munmap_impl(addr, len))
}

unsafe fn munmap_impl(addr: u64, len: u64) -> SysResult {
    result::ensure(addr != 0 && len != 0, SysError::Invalid)?;
    let start = (addr as usize) & !0xFFF;
    let len = ((len as usize) + 0xFFF) & !0xFFF;

    let pml4 = task::current_pml4();
    let vma = result::option(proc::find_vma_exact_mut(start, len), SysError::Invalid)?;

    for off in (0..len).step_by(0x1000) {
        if let Some(frame) = paging::unmap_page_in(pml4, start + off) {
            pmm::free_frame(frame);
        }
    }
    *vma = proc::VmRegion::empty();
    result::ok(0u64)
}

pub unsafe fn mprotect(addr: u64, len: u64, prot: u64) -> u64 {
    result::ret(mprotect_impl(addr, len, prot))
}

unsafe fn mprotect_impl(addr: u64, len: u64, prot: u64) -> SysResult {
    result::ensure(addr != 0 && len != 0, SysError::Invalid)?;
    let start = (addr as usize) & !0xFFF;
    let len = ((len as usize) + 0xFFF) & !0xFFF;

    let pml4 = task::current_pml4();
    let vma = result::option(proc::find_vma_exact_mut(start, len), SysError::Invalid)?;
    let flags = map_flags_from_prot(prot as u32);
    for off in (0..len).step_by(0x1000) {
        if !paging::protect_page_in(pml4, start + off, flags) {
            return result::err(SysError::Invalid);
        }
    }
    vma.prot = prot as u32;
    result::ok(0u64)
}
