/// Task management
///
/// Each task has:
///   - A kernel stack (one PMM page = 4 KB)
///   - A saved Context pushed on that stack when not running
///   - A State (Ready / Running / Dead)
///
/// Context layout must match the push/pop order in scheduler.rs switch_to().
use crate::pmm::{alloc_frame, PAGE_SIZE};

// ---------------------------------------------------------------------------
// Context — saved register state
// ---------------------------------------------------------------------------

/// Saved general-purpose registers in the order switch_to() pushes them.
/// rip is NOT here — it is the return address naturally on the stack after
/// the `call switch_to` instruction.
#[repr(C)]
pub struct Context {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    // rsp is implicit — it's the stack pointer of this task's kernel stack
}

// ---------------------------------------------------------------------------
// Task state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum TaskState {
    Ready,
    Running,
    Dead,
}

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

pub struct Task {
    /// Saved kernel stack pointer (points to Context on the stack).
    pub rsp: usize,
    /// Bottom of the allocated kernel stack page.
    pub stack_base: usize,
    pub state: TaskState,
    pub id: usize,
    /// False for task 0 until its first switch-out saves a real rsp.
    pub rsp_valid: bool,
}

// ---------------------------------------------------------------------------
// Global task table
// ---------------------------------------------------------------------------

pub const MAX_TASKS: usize = 8;

static mut TASKS: [Option<Task>; MAX_TASKS] = [None, None, None, None, None, None, None, None];
static mut CURRENT: usize = 0;
static mut NEXT_ID: usize = 1;

// ---------------------------------------------------------------------------
// Create a task
// ---------------------------------------------------------------------------

/// Allocate a kernel stack, push a fake Context + entry address so that
/// the first `switch_to` into this task lands at `entry`.
pub unsafe fn add_task(entry: fn()) -> bool {
    // Find a free slot
    let slot = match TASKS.iter_mut().position(|t| t.is_none()) {
        Some(s) => s,
        None => {
            crate::dbg_log!("TASK", "no free task slots");
            return false;
        }
    };

    // Allocate one page for the kernel stack
    let stack_base = alloc_frame();
    if stack_base == 0 {
        crate::dbg_log!("TASK", "OOM allocating stack");
        return false;
    }

    // Stack grows downward; start at the top of the page.
    // We push (in order, high → low):
    //   entry address   ← will be "returned to" by ret in switch_to
    //   Context { rbp=0, rbx=0, r12=0, r13=0, r14=0, r15=0 }
    let stack_top = stack_base + PAGE_SIZE;

    let mut sp = stack_top;

    // Push the entry point as if it were a return address
    sp -= 8;
    (sp as *mut u64).write(entry as u64);

    // Push a zeroed Context
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
        rsp_valid: true,
    });

    crate::dbg_log!(
        "TASK",
        "created task {} in slot {} (stack={:#x})",
        id,
        slot,
        stack_base
    );
    true
}

// ---------------------------------------------------------------------------
// Scheduler helpers
// ---------------------------------------------------------------------------

/// Return the index of the currently running task.
pub unsafe fn current_idx() -> usize {
    CURRENT
}

/// Pick the next Ready task (round-robin), mark it Running,
/// mark the old one Ready (unless Dead). Returns (old_rsp_ptr, new_rsp).
///
/// Returns None if there is only one task or no ready task found.
pub unsafe fn next_task_switch() -> Option<(*mut usize, usize)> {
    let len = MAX_TASKS;

    // If there's no other ready task with a valid rsp, don't switch
    let ready_count = TASKS
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            *i != CURRENT
                && t.as_ref()
                    .map_or(false, |t| t.state == TaskState::Ready && t.rsp_valid)
        })
        .count();

    if ready_count == 0 {
        return None;
    }

    // Mark current as Ready
    if let Some(ref mut cur) = TASKS[CURRENT] {
        if cur.state == TaskState::Running {
            cur.state = TaskState::Ready;
        }
    }

    // Find the next Ready task with a valid rsp
    for i in 1..=len {
        let idx = (CURRENT + i) % len;
        if let Some(ref t) = TASKS[idx] {
            if t.state == TaskState::Ready && t.rsp_valid {
                let new_rsp = t.rsp;
                let new_idx = idx;

                // Get old rsp pointer and mark it valid after this switch
                let old_rsp_ptr = if let Some(ref mut cur) = TASKS[CURRENT] {
                    cur.rsp_valid = true; // switch_to will write the real rsp here
                    &mut cur.rsp as *mut usize
                } else {
                    return None;
                };

                TASKS[new_idx].as_mut().unwrap().state = TaskState::Running;
                CURRENT = new_idx;

                return Some((old_rsp_ptr, new_rsp));
            }
        }
    }

    None
}

/// Register the currently running context (the main kernel loop) as task 0.
/// Must be called before interrupts are enabled.
/// We don't need to set up a fake stack frame — the real rsp will be saved
/// on the first context switch out.
pub unsafe fn init_main_task(stack_base: usize) {
    TASKS[0] = Some(Task {
        rsp: 0,
        stack_base,
        state: TaskState::Running,
        id: 0,
        rsp_valid: false, // rsp is garbage until first switch_to saves it
    });
    CURRENT = 0;
    crate::dbg_log!("TASK", "main task registered as task 0");
}

