use crate::hw;

#[no_mangle]
pub extern "C" fn keyboard_handler() {
    unsafe {
        hw::pic::eoi(1);
        let scancode = hw::ps2::read_data();
        crate::serial_fmt!("[KBD] scancode={:#x}\n", scancode);
        let c = hw::ps2::scancode_to_char(scancode);
        if c != '\0' {
            hw::kbd_buffer::KEYBUF.push(c);
        }
    }
}
