/// Task management — kernel stacks + round-robin scheduler support.
use crate::mem::pmm::{alloc_frames, PAGE_SIZE};
use crate::syscall::types::SyscallFrame;

const SYSCALL_FRAME_SIZE: usize = 16 * 8;

#[repr(C)]
pub struct Context {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TaskState {
    Ready = 0,
    Running = 1,
    Dead = 2,
}

#[allow(dead_code)]
pub struct Task {
    pub rsp: usize,
    pub stack_base: usize,
    pub state: TaskState,
    pub id: usize,
    pub rsp_valid: bool,
    pub pml4: usize,
}

pub const MAX_TASKS: usize = 8;

static mut TASKS: [Option<Task>; MAX_TASKS] = [None, None, None, None, None, None, None, None];
static mut CURRENT: usize = 0;
static mut NEXT_ID: usize = 1;

pub unsafe fn add_task(entry: fn()) -> bool {
    let slot = match TASKS.iter_mut().position(|t| t.is_none()) {
        Some(s) => s,
        None => {
            crate::dbg_log!("TASK", "no free task slots");
            return false;
        }
    };

    let stack_pages = 4;
    let stack_phys = alloc_frames(stack_pages);
    if stack_phys == 0 {
        crate::dbg_log!("TASK", "OOM allocating stack");
        return false;
    }
    let stack_base = crate::arch::paging::p2v(stack_phys);

    let stack_top = stack_base + stack_pages * PAGE_SIZE;
    let mut sp = stack_top;

    sp -= 8;
    (sp as *mut u64).write(entry as u64);

    sp -= core::mem::size_of::<Context>();
    (sp as *mut Context).write(Context {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        rbx: 0,
        rbp: 0,
    });

    let id = NEXT_ID;
    NEXT_ID += 1;

    TASKS[slot] = Some(Task {
        rsp: sp,
        stack_base,
        state: TaskState::Ready,
        id,
        pml4: crate::arch::paging::KERNEL_PML4,
        rsp_valid: true,
    });
    crate::dbg_log!(
        "TASK",
        "created task {} in slot {} (stack={:#x})",
        id,
        slot,
        stack_phys
    );
    true
}

#[allow(dead_code)]
pub unsafe fn current_idx() -> usize {
    CURRENT
}

pub unsafe fn current_task_slot() -> Option<usize> {
    Some(CURRENT)
}

pub unsafe fn current_pml4() -> usize {
    TASKS[CURRENT]
        .as_ref()
        .map(|t| t.pml4)
        .unwrap_or(crate::arch::paging::KERNEL_PML4)
}

pub unsafe fn next_task_switch() -> Option<(*mut usize, usize, usize, u64)> {
    static mut DEAD_RSP: usize = 0;

    crate::serial_fmt!(
        "[SCHED] cur={} t0={} t1={} t2={}\n",
        CURRENT,
        TASKS[0].as_ref().map_or(99, |t| t.state as u8),
        TASKS[1].as_ref().map_or(99, |t| t.state as u8),
        TASKS[2].as_ref().map_or(99, |t| t.state as u8),
    );
    // Find next ready task (excluding current)
    let next_idx = {
        let mut found = None;
        for i in 1..=MAX_TASKS {
            let idx = (CURRENT + i) % MAX_TASKS;
            if let Some(ref t) = TASKS[idx] {
                if t.state == TaskState::Ready && t.rsp_valid {
                    found = Some(idx);
                    break;
                }
            }
        }
        found
    };

    let next_idx = match next_idx {
        Some(i) => i,
        None => return None,
    };

    let old_rsp_ptr: *mut usize = match TASKS[CURRENT] {
        Some(ref mut cur) => {
            if cur.state == TaskState::Running {
                cur.state = TaskState::Ready;
            }
            if cur.state == TaskState::Dead {
                super::task_slot_reaped(CURRENT);
                TASKS[CURRENT] = None;
                &raw mut DEAD_RSP
            } else {
                cur.rsp_valid = true;
                &mut cur.rsp as *mut usize
            }
        }
        None => &raw mut DEAD_RSP,
    };

    TASKS[next_idx].as_mut().unwrap().state = TaskState::Running;
    CURRENT = next_idx;

    // Update TSS.rsp0 so the CPU knows where the kernel stack is for the next
    // time this task enters the kernel from ring 3 (interrupt or syscall).
    let next_stack_top = TASKS[next_idx].as_ref().unwrap().stack_base + 4 * PAGE_SIZE;
    crate::arch::gdt::TSS.rsp0 = next_stack_top as u64;
    crate::arch::gdt::TSS_RSP0 = next_stack_top as u64;

    let next_task = TASKS[next_idx].as_ref().unwrap();
    let fs_base = super::THREADS[next_idx]
        .as_ref()
        .map(|thread| thread.fs_base)
        .unwrap_or(0);

    Some((old_rsp_ptr, next_task.rsp, next_task.pml4, fs_base))
}
pub unsafe fn init_main_task(stack_base: usize) {
    TASKS[0] = Some(Task {
        rsp: 0,
        stack_base,
        state: TaskState::Running,
        id: 0,
        pml4: crate::arch::paging::KERNEL_PML4,
        rsp_valid: true,
    });
    CURRENT = 0;
    crate::dbg_log!("TASK", "main task registered as task 0");
}

/// Spawn a user task that will jump to ring 3 at `entry` with `stack_top`.
/// Returns the task slot index, or None on failure.
pub unsafe fn spawn_user_task(entry: u64, stack_top: u64, pml4: usize) -> Option<usize> {
    let slot = match TASKS.iter_mut().position(|t| t.is_none()) {
        Some(s) => s,
        None => {
            crate::dbg_log!("TASK", "no free task slots for user task");
            return None;
        }
    };

    // Allocate a kernel stack for this task's context switch frame.
    let stack_pages = 4;
    let stack_phys = alloc_frames(stack_pages);
    if stack_phys == 0 {
        crate::dbg_log!("TASK", "OOM allocating user task kernel stack");
        return None;
    }
    let stack_base = crate::arch::paging::p2v(stack_phys);

    // We set rsp to the top of the kernel stack.
    // The task will be switched to via switch_to, which expects
    // callee-saved regs on the stack followed by a return address.
    // We point the return address at a trampoline that does iretq.
    let kstack_top = stack_base + stack_pages * PAGE_SIZE;
    let mut sp = kstack_top;

    // Push values for trampoline: entry, user stack, pml4
    sp -= 8;
    (sp as *mut u64).write(pml4 as u64);
    sp -= 8;
    (sp as *mut u64).write(stack_top);

    sp -= 8;
    (sp as *mut u64).write(entry);

    // Push the trampoline as the return address.
    sp -= 8;
    (sp as *mut u64).write(user_task_trampoline as *const () as u64);

    // Push zeroed callee-saved Context so switch_to can pop it.
    sp -= core::mem::size_of::<Context>();
    (sp as *mut Context).write(Context {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        rbx: 0,
        rbp: 0,
    });

    let id = NEXT_ID;
    NEXT_ID += 1;

    TASKS[slot] = Some(Task {
        rsp: sp,
        stack_base,
        state: TaskState::Ready,
        id,
        pml4,
        rsp_valid: true,
    });

    crate::dbg_log!(
        "TASK",
        "spawned user task {} in slot {} (kstack={:#x} entry={:#x} ustack={:#x})",
        id,
        slot,
        stack_phys,
        entry,
        stack_top
    );

    Some(slot)
}

pub unsafe fn spawn_forked_task(syscall_frame: *const u8, pml4: usize) -> Option<usize> {
    let slot = match TASKS.iter_mut().position(|t| t.is_none()) {
        Some(s) => s,
        None => {
            crate::dbg_log!("TASK", "no free task slots for forked task");
            return None;
        }
    };

    let stack_pages = 4;
    let stack_phys = alloc_frames(stack_pages);
    if stack_phys == 0 {
        crate::dbg_log!("TASK", "OOM allocating forked task kernel stack");
        return None;
    }
    let stack_base = crate::arch::paging::p2v(stack_phys);
    let kstack_top = stack_base + stack_pages * PAGE_SIZE;

    let frame_start = kstack_top - SYSCALL_FRAME_SIZE;
    core::ptr::copy_nonoverlapping(syscall_frame, frame_start as *mut u8, SYSCALL_FRAME_SIZE);
    ((frame_start + 88) as *mut u64).write(0);

    let mut sp = frame_start;
    sp -= 8;
    (sp as *mut u64).write(fork_return_trampoline as *const () as u64);
    sp -= core::mem::size_of::<Context>();
    (sp as *mut Context).write(Context {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        rbx: 0,
        rbp: 0,
    });

    let id = NEXT_ID;
    NEXT_ID += 1;
    TASKS[slot] = Some(Task {
        rsp: sp,
        stack_base,
        state: TaskState::Ready,
        id,
        pml4,
        rsp_valid: true,
    });

    crate::dbg_log!("TASK", "forked user task {} in slot {}", id, slot);
    Some(slot)
}

pub unsafe fn task_frame_mut(slot: usize) -> Option<&'static mut SyscallFrame> {
    let task = TASKS.get(slot)?.as_ref()?;
    let frame_ptr = task.rsp + core::mem::size_of::<Context>() + 8;
    Some(&mut *(frame_ptr as *mut SyscallFrame))
}

/// Trampoline — called via ret from switch_to,
/// Stack at entry:
/// [entry: u64, stack_top: u64, pml4: u64]
/// Drops to ring 3 via iretq.
#[unsafe(naked)]
unsafe extern "C" fn user_task_trampoline() -> ! {
    core::arch::naked_asm!(
        "pop rdi", // entry point
        "pop rsi", // user stack top
        "pop rdx", // pml4
        // switch address space
        "mov cr3, rdx",
        // jump into helper that performs iretq
        "call user_task_iretq",
        "ud2",
    );
}

#[unsafe(naked)]
unsafe extern "C" fn fork_return_trampoline() -> ! {
    core::arch::naked_asm!(
        "pop rbp",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r9",
        "pop r8",
        "pop rsi",
        "pop rdi",
        "pop rdx",
        "pop rbx",
        "pop rax",
        "pop rcx",
        "pop r11",
        "pop r10",
        "add rsp, 8",
        "mov rsp, r10",
        "sysretq",
    );
}

/// Does the actual iretq into ring 3.
/// rdi = entry, rsi = user stack top
#[unsafe(naked)]
#[no_mangle]
unsafe extern "C" fn user_task_iretq(entry: u64, stack_top: u64) -> ! {
    core::arch::naked_asm!(
        "cli",
        "mov rdx, 0x23",  // CS = SEG_UCODE | 3
        "mov rcx, 0x1b",  // SS = SEG_UDATA | 3
        "mov r8,  0x202", // RFLAGS = IF | reserved
        "push rcx",       // SS
        "push rsi",       // RSP
        "push r8",        // RFLAGS
        "push rdx",       // CS
        "push rdi",       // RIP
        "iretq",
    );
}

pub unsafe fn kill_task(slot: usize) {
    if let Some(ref mut t) = TASKS[slot] {
        t.state = TaskState::Dead;
        crate::dbg_log!("TASK", "task in slot {} marked dead", slot);
    }
}
