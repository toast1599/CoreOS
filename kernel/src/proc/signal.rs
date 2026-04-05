use crate::proc;
use crate::syscall::types::{SigAction, SigSet};
use crate::syscall::SyscallFrame;

pub unsafe fn check_signals(frame_ptr: u64) {
    let slot = match proc::task::current_task_slot() {
        Some(s) => s,
        None => return,
    };

    let mut lock = proc::PROCESSES.lock();
    let process = match lock[slot].as_mut() {
        Some(p) => p,
        None => return,
    };

    // Find the first pending signal that is not masked
    let mut sig = 0;
    for i in 0..16 {
        let pending = process.sig_pending.bits[i] & !process.sig_mask.bits[i];
        if pending != 0 {
            sig = i * 64 + (pending.trailing_zeros() as usize) + 1;
            break;
        }
    }

    if sig == 0 || sig > 64 {
        return;
    }

    // Clear the pending bit
    let word = (sig - 1) / 64;
    let bit = (sig - 1) % 64;
    process.sig_pending.bits[word] &= !(1 << bit);

    let action = process.sig_handlers[sig];
    if action.handler == 0 { // SIG_DFL
        // Default action: most result in termination
        if matches!(sig, 1 | 2 | 3 | 6 | 8 | 9 | 11 | 13 | 14 | 15) {
            crate::dbg_log!("SIGNAL", "pid={} terminated by signal {}", process.pid, sig);
            process.state = proc::ProcessState::Zombie;
            process.exit_code = (128 + sig as i64) << 8;
            drop(lock);
            proc::task::kill_task(slot);
            return;
        }
        return;
    }

    if action.handler == 1 { // SIG_IGN
        return;
    }

    // Deliver signal: redirect user task to handler
    deliver_signal(process, sig, action, frame_ptr);
}

unsafe fn deliver_signal(process: &mut proc::Process, sig: usize, action: SigAction, frame_ptr: u64) {
    let frame = &mut *(frame_ptr as *mut SyscallFrame);
    
    let mut ustack = frame.rsp;
    
    // Align stack and save red zone
    ustack -= 128; 
    ustack &= !15;
    
    // Save current frame on user stack so sigreturn can restore it
    ustack -= core::mem::size_of::<SyscallFrame>() as u64;
    // let _saved_frame_ptr = ustack as *mut SyscallFrame;
    if crate::usercopy::copy_to_user(ustack, core::slice::from_raw_parts(frame_ptr as *const u8, core::mem::size_of::<SyscallFrame>())).is_err() {
        crate::dbg_log!("SIGNAL", "failed to push signal frame to user stack");
        return;
    }
    
    // Prepare handler entry
    frame.rip = action.handler;
    frame.rdi = sig as u64;
    // Standard Linux doesn't push the restorer to the stack manually if SA_RESTORER is not used,
    // it relies on libc providing it. But we'll follow SA_RESTORER if present.
    if action.restorer != 0 {
        ustack -= 8;
        if crate::usercopy::copy_to_user(ustack, &action.restorer.to_ne_bytes()).is_err() {
             return;
        }
    }
    
    frame.rsp = ustack;
    
    crate::dbg_log!("SIGNAL", "delivered signal {} to pid={} handler={:#x}", sig, process.pid, action.handler);
}
