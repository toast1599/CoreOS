use super::{Process, Thread, PROCESSES, THREADS};

/// Small facade over global process/thread tables.
///
/// This is intentionally minimal in the first refactor step so call-sites can
/// migrate away from direct static access without changing behavior.
pub struct KernelState;

impl KernelState {
    #[inline]
    pub unsafe fn process(slot: usize) -> Option<&'static Process> {
        PROCESSES.get(slot)?.as_ref()
    }

    #[inline]
    pub unsafe fn process_mut(slot: usize) -> Option<&'static mut Process> {
        PROCESSES.get_mut(slot)?.as_mut()
    }

    #[inline]
    pub unsafe fn thread(slot: usize) -> Option<&'static Thread> {
        THREADS.get(slot)?.as_ref()
    }

    #[inline]
    pub unsafe fn thread_mut(slot: usize) -> Option<&'static mut Thread> {
        THREADS.get_mut(slot)?.as_mut()
    }

    #[inline]
    pub unsafe fn clear_thread(slot: usize) {
        if let Some(thread) = THREADS.get_mut(slot) {
            *thread = None;
        }
    }
}

