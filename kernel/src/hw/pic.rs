/// 8259 Programmable Interrupt Controller (PIC) driver.
///
/// Two cascaded PICs: master (0x20/0x21) and slave (0xA0/0xA1).
/// We remap IRQ 0–7  → IDT vectors 0x20–0x27 (master)
///         IRQ 8–15 → IDT vectors 0x28–0x2F (slave)
///
/// After init we mask everything except IRQ 0 (PIT) and IRQ 1 (keyboard).

// I/O ports
const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

// Initialisation Control Words
const ICW1_INIT: u8 = 0x11; // start init sequence, ICW4 needed
const ICW4_8086: u8 = 0x01; // 8086/88 mode

// IRQ mask: 0 = enabled, 1 = masked.
// Enable only IRQ 0 (timer) and IRQ 1 (keyboard) on master; mask all slave IRQs.
const MASTER_MASK: u8 = 0b1111_1100; // IRQ0 + IRQ1 unmasked
const SLAVE_MASK: u8 = 0b1111_1111; // all masked

#[inline]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nostack, nomem));
}

/// Remap both PICs and apply IRQ masks.
/// Must be called before `sti`.
pub unsafe fn init() {
    // ICW1 — start init on both PICs
    outb(PIC1_CMD, ICW1_INIT);
    outb(PIC2_CMD, ICW1_INIT);

    // ICW2 — vector offsets
    outb(PIC1_DATA, 0x20); // master IRQs → vectors 0x20–0x27
    outb(PIC2_DATA, 0x28); // slave  IRQs → vectors 0x28–0x2F

    // ICW3 — cascade wiring
    outb(PIC1_DATA, 0x04); // master: slave on IRQ2
    outb(PIC2_DATA, 0x02); // slave:  cascade identity 2

    // ICW4 — 8086 mode
    outb(PIC1_DATA, ICW4_8086);
    outb(PIC2_DATA, ICW4_8086);

    // OCW1 — set IRQ masks
    outb(PIC1_DATA, MASTER_MASK);
    outb(PIC2_DATA, SLAVE_MASK);

    crate::dbg_log!(
        "PIC",
        "remapped; master mask={:#010b} slave mask={:#010b}",
        MASTER_MASK,
        SLAVE_MASK
    );
}

/// Send End-Of-Interrupt to the master PIC (and slave if IRQ >= 8).
#[inline]
pub unsafe fn eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_CMD, 0x20);
    }
    outb(PIC1_CMD, 0x20);
}

