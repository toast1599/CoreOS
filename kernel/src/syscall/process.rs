use crate::proc;
use crate::proc::scheduler;
use crate::proc::task;
use crate::mem::pmm;
use crate::arch::paging;
use crate::syscall::fs;

pub unsafe fn syscall_exec(path_ptr: u64, path_len: u64) -> u64 {
    if path_len == 0 || path_len > 64 {
        return 0;
    }
    let path_bytes = core::slice::from_raw_parts(path_ptr as *const u8, path_len as usize);
    let mut name_buf = ['\0'; 64];
    for (i, &b) in path_bytes.iter().enumerate() {
        name_buf[i] = b as char;
    }
    let name = &name_buf[..path_len as usize];
    let elf_bytes = match fs::fs_clone_by_name(name) {
        Some(v) => v,
        None => {
            crate::dbg_log!("SYSCALL", "exec: file not found");
            return 0;
        }
    };
    let (pid, _slot) = crate::proc::exec::exec_as_task(elf_bytes.as_slice());
    pid as u64
}

pub unsafe fn syscall_waitpid(pid: u64) -> u64 {
    let target_pid = pid as usize;
    let mut slot = None;
    for i in 0..8 {
        if let Some(ref p) = proc::PROCESSES[i] {
            if p.pid == target_pid {
                slot = Some(i);
                break;
            }
        }
    }

    let slot = match slot {
        Some(s) => s,
        None => return u64::MAX,
    };

    loop {
        if !proc::is_running_in_slot(slot) {
            break;
        }
        scheduler::yield_now();
        core::hint::spin_loop();
    }
    match proc::reap_slot(slot) {
        Some(code) => code as u64,
        None => u64::MAX,
    }
}

pub unsafe fn syscall_brk(addr: u64) -> u64 {
    let current_brk = proc::current_brk();
    if addr == 0 {
        return current_brk as u64;
    }
    let new_brk = addr as usize;
    if new_brk <= current_brk {
        proc::set_brk(new_brk);
        return new_brk as u64;
    }

    let mut page = (current_brk + 0xFFF) & !0xFFF;
    let end = (new_brk + 0xFFF) & !0xFFF;

    while page < end {
        let frame = pmm::alloc_frame();
        if frame == 0 {
            crate::dbg_log!("BRK", "OOM");
            return current_brk as u64;
        }
        paging::map_page(page, frame);
        page += 0x1000;
    }

    proc::set_brk(new_brk);
    new_brk as u64
}

pub unsafe fn syscall_boottime(buf_ptr: u64, buf_len: u64) -> u64 {
    let report = crate::bench::report();
    let bytes = report.as_bytes();
    let count = (buf_len as usize).min(bytes.len());
    let buf = buf_ptr as *mut u8;
    core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, count);
    count as u64
}

pub unsafe fn syscall_exit(code: u64) -> ! {
    crate::dbg_log!("SYSCALL", "exit({})", code);
    proc::exit(code as i64);
    if let Some(slot) = task::current_task_slot() {
        task::kill_task(slot);
    }
    core::arch::asm!("sti");
    loop {
        core::arch::asm!("hlt");
    }
}
