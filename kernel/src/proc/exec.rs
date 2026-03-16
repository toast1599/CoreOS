use crate::arch::paging;
use crate::mem::pmm::{alloc_frames, PAGE_SIZE};

const USER_STACK_PAGES: usize = 16;
const USER_STACK_TOP: usize = 0x0000_0000_8000_0000;

/// Load `elf_data`, spawn a user task, register a process entry.
/// Returns the PID on success, or 0 on failure.
pub unsafe fn exec_as_task(elf_data: &[u8]) -> (usize, usize) {
    let new_pml4 = paging::clone_kernel_address_space();
    let entry = match super::elf::load_into(new_pml4, elf_data) {
        Ok(e) => e,
        Err(err) => {
            crate::dbg_log!("EXEC", "ELF load failed: {:?}", err);
            return (0, 0);
        }
    };
    crate::dbg_log!("EXEC", "ELF entry={:#x}", entry);

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
    let user_stack_top = (USER_STACK_TOP & !0xF) - 8;
    crate::dbg_log!(
        "EXEC",
        "user stack base={:#x} top={:#x}",
        user_stack_base,
        user_stack_top
    );

    let slot = match super::task::spawn_user_task(entry, user_stack_top as u64, new_pml4) {
        Some(s) => s,
        None => {
            crate::dbg_log!("EXEC", "failed to spawn user task");
            return (0, 0);
        }
    };

    // -----------------------------------------------------------------------
    // 5. Register process entry
    // -----------------------------------------------------------------------
    let pid = super::spawn(slot, new_pml4);
    crate::dbg_log!("EXEC", "process pid={} running in task slot={}", pid, slot);
    (pid, slot)
}
