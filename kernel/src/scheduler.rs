static mut COUNTER: u64 = 0;

pub fn tick() {
    unsafe {
        COUNTER += 1;

        // every 200 ticks (~2 seconds at 100Hz)
        if COUNTER % 200 == 0 {
            // placeholder for future task switch
        }
    }
}
