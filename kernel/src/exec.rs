/// exec.rs — Load an ELF and spawn it as a userspace task.
///
/// Unlike the previous version, exec_as_task() RETURNS to the caller.
/// The user process runs as a scheduler task. The shell blocks until
/// process::is_running() returns false.
use crate::pmm::{alloc_frame, PAGE_SIZE};

/// Number of pages to allocate for the user stack.
const USER_STACK_PAGES: usize = 4; // 16 KB

/// Load `elf_data`, spawn a user task, register a process entry.
/// Returns the PID on success, or 0 on failure.
pub unsafe fn exec_as_task(elf_data: &[u8]) -> usize {
    // -----------------------------------------------------------------------
    // 1. Load ELF into memory
    // -----------------------------------------------------------------------
    let entry = match crate::elf::load(elf_data) {
        Ok(e) => e,
        Err(err) => {
            crate::dbg_log!("EXEC", "ELF load failed: {:?}", err);
            return 0;
        }
    };
    crate::dbg_log!("EXEC", "ELF entry={:#x}", entry);

    // -----------------------------------------------------------------------
    // 2. Allocate user stack (4 pages = 16 KB)
    // -----------------------------------------------------------------------
    let mut stack_top: usize = 0;
    for i in 0..USER_STACK_PAGES {
        let frame = alloc_frame();
        if frame == 0 {
            crate::dbg_log!("EXEC", "OOM allocating user stack page {}", i);
            return 0;
        }
        let top = frame + PAGE_SIZE;
        if top > stack_top {
            stack_top = top;
        }
    }
    stack_top = (stack_top & !0xF) - 8;
    crate::dbg_log!("EXEC", "user stack top={:#x}", stack_top);

    // -----------------------------------------------------------------------
    // 3. Allocate a dedicated syscall kernel stack and update TSS
    // -----------------------------------------------------------------------
    let syscall_stack = alloc_frame();
    if syscall_stack == 0 {
        crate::dbg_log!("EXEC", "OOM allocating syscall stack");
        return 0;
    }
    let syscall_stack_top = (syscall_stack + PAGE_SIZE) as u64;
    crate::gdt::TSS.rsp0 = syscall_stack_top;
    crate::gdt::TSS_RSP0 = syscall_stack_top;
    crate::dbg_log!("EXEC", "syscall stack top={:#x}", syscall_stack_top);

    // -----------------------------------------------------------------------
    // 4. Spawn kernel task that will iretq into ring 3
    // -----------------------------------------------------------------------
    let slot = match crate::task::spawn_user_task(entry, stack_top as u64) {
        Some(s) => s,
        None => {
            crate::dbg_log!("EXEC", "failed to spawn user task");
            return 0;
        }
    };

    // -----------------------------------------------------------------------
    // 5. Register process entry
    // -----------------------------------------------------------------------
    let pid = crate::process::spawn(slot);
    crate::dbg_log!("EXEC", "process pid={} running in task slot={}", pid, slot);
    pid
}

