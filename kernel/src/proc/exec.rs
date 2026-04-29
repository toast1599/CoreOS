use crate::arch::paging;
use crate::mem::pmm::{alloc_frames, PAGE_SIZE};

const USER_STACK_PAGES: usize = 16;
const USER_STACK_TOP: usize = 0x0000_0000_8000_0000;
const AT_NULL: u64 = 0;
const AT_PHDR: u64 = 3;
const AT_PHENT: u64 = 4;
const AT_PHNUM: u64 = 5;
const AT_PAGESZ: u64 = 6;
const AT_ENTRY: u64 = 9;
const AT_UID: u64 = 11;
const AT_EUID: u64 = 12;
const AT_GID: u64 = 13;
const AT_EGID: u64 = 14;
const AT_SECURE: u64 = 23;
const AT_RANDOM: u64 = 25;
const AT_EXECFN: u64 = 31;

#[derive(Clone, Copy)]
struct AuxEntry {
    key: u64,
    value: u64,
}

unsafe fn write_user_bytes(pml4: usize, mut user_addr: usize, bytes: &[u8]) -> bool {
    let mut written = 0usize;
    while written < bytes.len() {
        let Some(frame) = paging::translate_page(pml4, user_addr) else {
            return false;
        };
        let page_off = user_addr & (PAGE_SIZE - 1);
        let chunk = (PAGE_SIZE - page_off).min(bytes.len() - written);
        let dst = (paging::p2v(frame) + page_off) as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr().add(written), dst, chunk);
        user_addr += chunk;
        written += chunk;
    }
    true
}

unsafe fn push_user_bytes(pml4: usize, sp: &mut usize, bytes: &[u8], align: usize) -> Option<u64> {
    let mask = !(align.saturating_sub(1));
    *sp = sp.checked_sub(bytes.len())? & mask;
    write_user_bytes(pml4, *sp, bytes).then_some(*sp as u64)
}

unsafe fn push_user_u64(pml4: usize, sp: &mut usize, value: u64) -> Option<u64> {
    let bytes = value.to_ne_bytes();
    push_user_bytes(pml4, sp, &bytes, core::mem::size_of::<u64>())
}

unsafe fn build_initial_stack(
    pml4: usize,
    mut stack_top: usize,
    name: &[char],
    image: crate::proc::elf::LoadedImage,
) -> Option<u64> {
    let mut execfn = [0u8; super::EXE_PATH_MAX + 2];
    execfn[0] = b'/';
    let mut exec_len = 1usize;
    for &ch in name.iter().take(super::EXE_PATH_MAX) {
        execfn[exec_len] = ch as u8;
        exec_len += 1;
    }
    execfn[exec_len] = 0;
    let execfn_ptr = push_user_bytes(pml4, &mut stack_top, &execfn[..=exec_len], 1)?;

    let random_bytes = [
        0x13, 0x37, 0xca, 0xfe, 0xba, 0xbe, 0xde, 0xad,
        0xfa, 0xce, 0x12, 0x34, 0x56, 0x78, 0xab, 0xcd,
    ];
    let random_ptr = push_user_bytes(pml4, &mut stack_top, &random_bytes, 16)?;

    let process = super::current_process();
    let uid = process.map(|p| p.uid as u64).unwrap_or(0);
    let euid = process.map(|p| p.euid as u64).unwrap_or(0);
    let gid = process.map(|p| p.gid as u64).unwrap_or(0);
    let egid = process.map(|p| p.egid as u64).unwrap_or(0);

    let auxv = [
        AuxEntry { key: AT_PHDR, value: image.phdr },
        AuxEntry { key: AT_PHENT, value: image.phent },
        AuxEntry { key: AT_PHNUM, value: image.phnum },
        AuxEntry { key: AT_PAGESZ, value: PAGE_SIZE as u64 },
        AuxEntry { key: AT_ENTRY, value: image.entry },
        AuxEntry { key: AT_UID, value: uid },
        AuxEntry { key: AT_EUID, value: euid },
        AuxEntry { key: AT_GID, value: gid },
        AuxEntry { key: AT_EGID, value: egid },
        AuxEntry { key: AT_SECURE, value: 0 },
        AuxEntry { key: AT_RANDOM, value: random_ptr },
        AuxEntry { key: AT_EXECFN, value: execfn_ptr },
        AuxEntry { key: AT_NULL, value: 0 },
    ];

    for entry in auxv.iter().rev() {
        push_user_u64(pml4, &mut stack_top, entry.value)?;
        push_user_u64(pml4, &mut stack_top, entry.key)?;
    }
    push_user_u64(pml4, &mut stack_top, 0)?; // envp terminator
    push_user_u64(pml4, &mut stack_top, 0)?; // argv terminator
    push_user_u64(pml4, &mut stack_top, execfn_ptr)?;
    push_user_u64(pml4, &mut stack_top, 1)?;

    Some((stack_top & !0xFu64 as usize) as u64)
}

/// Load `elf_data`, spawn a user task, register a process entry.
/// Returns the PID on success, or 0 on failure.
pub unsafe fn exec_as_task(elf_data: &[u8], name: &[char]) -> (usize, usize) {
    let new_pml4 = paging::clone_kernel_address_space();
    let image = match super::elf::load_into(new_pml4, elf_data) {
        Ok(image) => image,
        Err(err) => {
            crate::dbg_log!("EXEC", "ELF load failed: {:?}", err);
            return (0, 0);
        }
    };

    // -----------------------------------------------------------------------
    // 2. Allocate user stack — must be contiguous so the full 64 KB is usable.
    //    alloc_frames() returns the base address of a contiguous block.
    // -----------------------------------------------------------------------
    let user_stack_base = USER_STACK_TOP - USER_STACK_PAGES * PAGE_SIZE;
    for off in (0..USER_STACK_PAGES * PAGE_SIZE).step_by(PAGE_SIZE) {
        let frame = alloc_frames(1);
        if frame == 0 {
            crate::dbg_log!("EXEC", "OOM allocating user stack");
            return (0, 0);
        }
        paging::map_page_in(
            new_pml4,
            user_stack_base + off,
            frame,
            paging::MapFlags {
                writable: true,
                user: true,
                executable: false,
            },
        );
        core::ptr::write_bytes(paging::p2v(frame) as *mut u8, 0, PAGE_SIZE);
    }
    let user_stack_top = match build_initial_stack(new_pml4, USER_STACK_TOP & !0xF, name, image) {
        Some(sp) => sp,
        None => {
            crate::dbg_log!("EXEC", "failed to build initial user stack");
            return (0, 0);
        }
    };
    let slot = match super::task::spawn_user_task(image.entry, user_stack_top, new_pml4) {
        Some(s) => s,
        None => {
            crate::dbg_log!("EXEC", "failed to spawn user task");
            return (0, 0);
        }
    };

    // -----------------------------------------------------------------------
    // 5. Register process entry
    // -----------------------------------------------------------------------
    let pid = super::spawn_named(slot, new_pml4, name);
    crate::dbg_log!("EXEC", "process pid={} running in task slot={}", pid, slot);
    (pid, slot)
}
