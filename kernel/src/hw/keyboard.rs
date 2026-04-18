use crate::hw;

/// Read a scancode from the PS/2 data port.
/// Isolated unsafe boundary.
#[inline(always)]
fn read_scancode() -> u8 {
    unsafe { hw::ps2::read_data() }
}

/// Send End-Of-Interrupt to PIC.
/// Isolated unsafe boundary.
#[inline(always)]
fn send_eoi() {
    unsafe { hw::pic::eoi(1) }
}

/// Process a scancode outside of IRQ-critical logic.
/// This stays safe and testable.
#[inline]
fn handle_scancode(scancode: u8) {
    let event = hw::ps2::decode_scancode(scancode);

    if let Some(c) = hw::ps2::keyevent_to_char(event) {
        hw::kbd_buffer::KEYBUF.push(c);
    }

    // Optional debug logging (disabled by default)
    #[cfg(feature = "kbd_debug")]
    {
        crate::serial_fmt!("[KBD] scancode={:#x}\n", scancode);
    }
}

/// Keyboard interrupt handler (IRQ1).
///
/// Design:
/// - Do the absolute minimum in interrupt context
/// - No heavy operations (like logging)
/// - Unsafe strictly limited to hardware interaction
#[no_mangle]
pub extern "C" fn keyboard_handler() {
    // 1. Read hardware input ASAP
    let scancode = read_scancode();

    // 2. Acknowledge interrupt early
    send_eoi();

    // 3. Handle logic outside of unsafe
    handle_scancode(scancode);
}
