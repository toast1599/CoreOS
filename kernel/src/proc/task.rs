/// Task management — kernel stacks + round-robin scheduler support.
use crate::mem::pmm::{alloc_frames, PAGE_SIZE};
use crate::syscall::types::SyscallFrame;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
const SYSCALL_FRAME_SIZE: usize = 16 * 8;

struct DeadRspCell(UnsafeCell<usize>);
unsafe impl Sync for DeadRspCell {}

static DEAD_RSP: DeadRspCell = DeadRspCell(UnsafeCell::new(0));

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

static TASKS: Mutex<[Option<Task>; MAX_TASKS]> =
    Mutex::new([None, None, None, None, None, None, None, None]);

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
pub unsafe fn add_task(entry: fn()) -> bool {
    let mut tasks = TASKS.lock();
    let slot = match tasks.iter_mut().position(|t| t.is_none()) {
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

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    tasks[slot] = Some(Task {
        rsp: sp,
        stack_base,
        state: TaskState::Ready,
        id,
        pml4: crate::arch::paging::KERNEL_PML4.load(Ordering::SeqCst),
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
pub fn current_idx() -> usize {
    CURRENT.load(Ordering::SeqCst)
}
pub fn current_task_slot() -> Option<usize> {
    Some(CURRENT.load(Ordering::SeqCst))
}
pub fn current_pml4() -> usize {
    let tasks = TASKS.lock();
    tasks[CURRENT.load(Ordering::SeqCst)]
        .as_ref()
        .map(|t| t.pml4)
        .unwrap_or(crate::arch::paging::KERNEL_PML4.load(Ordering::SeqCst))
}

pub unsafe fn next_task_switch() -> Option<(*mut usize, usize, usize, u64)> {
    let mut tasks = TASKS.lock();
    let cur = CURRENT.load(Ordering::SeqCst);

    let mut next_idx = None;
    for i in 1..=MAX_TASKS {
        let idx = (cur + i) % MAX_TASKS;
        if let Some(ref t) = tasks[idx] {
            if t.state == TaskState::Ready && t.rsp_valid {
                next_idx = Some(idx);
                break;
            }
        }
    }

    let next_idx = next_idx?;

    let old_rsp_ptr: *mut usize = match tasks[cur] {
        Some(ref mut task) => {
            if task.state == TaskState::Running {
                task.state = TaskState::Ready;
            }

            if task.state == TaskState::Dead {
                super::task_slot_reaped(cur);
                tasks[cur] = None;
                DEAD_RSP.0.get()
            } else {
                task.rsp_valid = true;
                &mut task.rsp as *mut usize
            }
        }
        None => DEAD_RSP.0.get(),
    };

    tasks[next_idx].as_mut().unwrap().state = TaskState::Running;

    let next_stack_top = tasks[next_idx].as_ref().unwrap().stack_base + 4 * PAGE_SIZE;

    let next_rsp = tasks[next_idx].as_ref().unwrap().rsp;
    let next_pml4 = tasks[next_idx].as_ref().unwrap().pml4;

    drop(tasks);

    CURRENT.store(next_idx, Ordering::SeqCst);

    crate::arch::gdt::TSS.rsp0 = next_stack_top as u64;
    crate::arch::gdt::TSS_RSP0 = next_stack_top as u64;

    let fs_base = super::THREADS[next_idx]
        .as_ref()
        .map(|thread| thread.fs_base)
        .unwrap_or(0);

    Some((old_rsp_ptr, next_rsp, next_pml4, fs_base))
}

pub fn init_main_task(stack_base: usize) {
    let mut tasks = TASKS.lock();
    tasks[0] = Some(Task {
        rsp: 0,
        stack_base,
        state: TaskState::Running,
        id: 0,
        pml4: { crate::arch::paging::KERNEL_PML4.load(Ordering::SeqCst) },
        rsp_valid: true,
    });
    drop(tasks);

    CURRENT.store(0, Ordering::SeqCst);
    crate::dbg_log!("TASK", "main task registered as task 0");
}

/// Spawn a user task that will jump to ring 3 at `entry` with `stack_top`.
/// Returns the task slot index, or None on failure.
pub unsafe fn spawn_user_task(entry: u64, stack_top: u64, pml4: usize) -> Option<usize> {
    let mut tasks = TASKS.lock();
    let slot = match tasks.iter_mut().position(|t| t.is_none()) {
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

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    tasks[slot] = Some(Task {
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
    let mut tasks = TASKS.lock();
    let slot = match tasks.iter_mut().position(|t| t.is_none()) {
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

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    tasks[slot] = Some(Task {
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

pub fn task_frame_mut(slot: usize) -> Option<&'static mut SyscallFrame> {
    let frame_ptr = {
        let tasks = TASKS.lock();
        let task = tasks.get(slot)?.as_ref()?;
        task.rsp + core::mem::size_of::<Context>() + 8
    };

    Some(unsafe { &mut *(frame_ptr as *mut SyscallFrame) })
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
    let mut tasks = TASKS.lock();

    if let Some(ref mut t) = tasks[slot] {
        t.state = TaskState::Dead;
        crate::dbg_log!("TASK", "task in slot {} marked dead", slot);
    }
}
