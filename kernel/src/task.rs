pub struct Task {
    pub stack: *mut u8,
    pub entry: fn(),
}

static mut TASKS: [Option<Task>; 8] = [None, None, None, None, None, None, None, None];
static mut CURRENT: usize = 0;

pub unsafe fn add_task(entry: fn()) {
    for slot in TASKS.iter_mut() {
        if slot.is_none() {
            *slot = Some(Task {
                stack: core::ptr::null_mut(),
                entry,
            });
            break;
        }
    }
}

pub unsafe fn next_task() -> Option<&'static Task> {
    for _ in 0..TASKS.len() {
        CURRENT = (CURRENT + 1) % TASKS.len();
        if let Some(ref t) = TASKS[CURRENT] {
            return Some(t);
        }
    }
    None
}

pub unsafe fn run_first_task() -> ! {
    for t in TASKS.iter() {
        if let Some(task) = t {
            (task.entry)();
        }
    }

    loop {}
}
