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
        crate::serial_fmt!(
            "[KBD] scancode=0x{:02x} char='{}'\n",
            scancode,
            c.escape_default()
        );
        hw::tty::TTY0.input_char(c as u8);
    } else {
        crate::serial_fmt!("[KBD] scancode=0x{:02x} no-char\n", scancode);
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
