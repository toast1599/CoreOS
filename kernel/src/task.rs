/// Task management — kernel stacks + round-robin scheduler support.
use crate::pmm::{alloc_frame, PAGE_SIZE};

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
    Ready,
    Running,
    Dead,
}

pub struct Task {
    pub rsp: usize,
    pub stack_base: usize,
    pub state: TaskState,
    pub id: usize,
    pub rsp_valid: bool,
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

    let stack_base = alloc_frame();
    if stack_base == 0 {
        crate::dbg_log!("TASK", "OOM allocating stack");
        return false;
    }

    let stack_top = stack_base + PAGE_SIZE;
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

pub unsafe fn current_idx() -> usize {
    CURRENT
}

pub unsafe fn next_task_switch() -> Option<(*mut usize, usize)> {
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

    if let Some(ref mut cur) = TASKS[CURRENT] {
        if cur.state == TaskState::Running {
            cur.state = TaskState::Ready;
        }
    }

    for i in 1..=MAX_TASKS {
        let idx = (CURRENT + i) % MAX_TASKS;
        if let Some(ref t) = TASKS[idx] {
            if t.state == TaskState::Ready && t.rsp_valid {
                let new_rsp = t.rsp;
                let old_rsp_ptr = if let Some(ref mut cur) = TASKS[CURRENT] {
                    cur.rsp_valid = true;
                    &mut cur.rsp as *mut usize
                } else {
                    return None;
                };
                TASKS[idx].as_mut().unwrap().state = TaskState::Running;
                CURRENT = idx;
                return Some((old_rsp_ptr, new_rsp));
            }
        }
    }
    None
}

pub unsafe fn init_main_task(stack_base: usize) {
    TASKS[0] = Some(Task {
        rsp: 0,
        stack_base,
        state: TaskState::Running,
        id: 0,
        rsp_valid: false,
    });
    CURRENT = 0;
    crate::dbg_log!("TASK", "main task registered as task 0");
}

