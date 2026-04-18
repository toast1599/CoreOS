use crate::mem::pmm;

pub fn test_pmm_basic() {
    crate::serial_fmt!("[TEST] PMM basic\n");

    let mut pages = [0usize; 128];

    // Allocate pages
    for i in 0..pages.len() {
        let p = pmm::alloc_frame();

        if p == 0 {
            panic!("alloc_frame returned null/0");
        }

        pages[i] = p;
    }

    // Check uniqueness
    for i in 0..pages.len() {
        for j in (i + 1)..pages.len() {
            if pages[i] == pages[j] {
                panic!("PMM returned duplicate frame!");
            }
        }
    }

    // Free all
    for &p in &pages {
        pmm::free_frame(p);
    }

    crate::serial_fmt!("[TEST] PMM basic passed\n");
}
