use crate::pmm::{alloc_frame, alloc_frames, PAGE_SIZE};

const USER_STACK_PAGES: usize = 16; // 64 KB — C shell needs headroom
const SYSCALL_STACK_PAGES: usize = 4; // 16 KB — kernel syscall frames

/// Load `elf_data`, spawn a user task, register a process entry.
/// Returns the PID on success, or 0 on failure.
pub unsafe fn exec_as_task(elf_data: &[u8]) -> (usize, usize) {
    // ...
    let entry = match crate::elf::load(elf_data) {
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
    let user_stack_base = alloc_frames(USER_STACK_PAGES);
    if user_stack_base == 0 {
        crate::dbg_log!("EXEC", "OOM allocating user stack");
        return (0, 0);
    }
    // Stack grows downward: top = base + total_size, aligned down to 16 bytes.
    let user_stack_top = ((user_stack_base + USER_STACK_PAGES * PAGE_SIZE) & !0xF) - 8;
    crate::dbg_log!("EXEC", "user stack base={:#x} top={:#x}", user_stack_base, user_stack_top);
    let syscall_stack_base = alloc_frames(SYSCALL_STACK_PAGES);
    if syscall_stack_base == 0 {
        crate::dbg_log!("EXEC", "OOM allocating syscall stack");
        return (0, 0);
    }
    // ...
    let slot = match crate::task::spawn_user_task(entry, user_stack_top as u64) {
        Some(s) => s,
        None => {
            crate::dbg_log!("EXEC", "failed to spawn user task");
            return (0, 0);
        }
    };

    // -----------------------------------------------------------------------
    // 5. Register process entry
    // -----------------------------------------------------------------------
    let pid = crate::process::spawn(slot);
    crate::dbg_log!("EXEC", "process pid={} running in task slot={}", pid, slot);
    (pid, slot)
}

