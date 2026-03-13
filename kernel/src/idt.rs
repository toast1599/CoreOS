/// Interrupt Descriptor Table.
///
/// Installs handlers for:
///   - Vectors 0–29: default exception handler (halts)
///   - Vector 32 (IRQ 0): PIT timer
///   - Vector 33 (IRQ 1): PS/2 keyboard
///
/// PIC remapping is handled by `hw::pic::init()`.
use crate::gdt::SEG_KCODE;
use core::mem::size_of;

// ---------------------------------------------------------------------------
// IDT entry
// ---------------------------------------------------------------------------

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    options: u16,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            options: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }

    fn set_handler(&mut self, handler: unsafe extern "C" fn()) {
        let addr = handler as u64;
        self.offset_low = addr as u16;
        self.selector = SEG_KCODE;
        self.options = 0x8E00; // present | interrupt gate | DPL 0
        self.offset_mid = (addr >> 16) as u16;
        self.offset_high = (addr >> 32) as u32;
        self.zero = 0;
    }
}

// ---------------------------------------------------------------------------
// IDT pointer
// ---------------------------------------------------------------------------

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

// ---------------------------------------------------------------------------
// ISR stubs (assembly)
// ---------------------------------------------------------------------------

core::arch::global_asm!(
    r#"
// --- PIT (IRQ 0, vector 32) -------------------------------------------
.global pit_interrupt
.extern pit_handler
pit_interrupt:
    cli
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    mov rdi, rsp        // pass stack pointer to pit_handler
    call pit_handler    // EOI is sent inside pit_handler via hw::pic::eoi(0)

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq

// --- Keyboard (IRQ 1, vector 33) --------------------------------------
.global keyboard_interrupt
.extern keyboard_handler
keyboard_interrupt:
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    call keyboard_handler   // sends EOI internally via hw::pic::eoi

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq
// --- Exceptions -------------------------------------------------------
.global exc0_stub
.global exc6_stub
.global exc13_stub
.global exc14_stub
.extern default_exception

exc0_stub:  push 0; push 0;  jmp exc_common
exc6_stub:  push 0; push 6;  jmp exc_common
exc13_stub:         push 13; jmp exc_common
exc14_stub:         push 14; jmp exc_common

exc_common:
    cli
    mov rdi, [rsp]      // first arg: vector
    // stack currently: [vector], [error_code], [rip], [cs], [rflags]...
    // default_exception(u64 vector)
    call default_exception
    add rsp, 16         // clean up vector and error code
    iretq
"#
);

// ---------------------------------------------------------------------------
// Static IDT
// ---------------------------------------------------------------------------

static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

extern "C" {
    fn keyboard_interrupt();
    fn pit_interrupt();
    fn exc0_stub();
    fn exc6_stub();
    fn exc13_stub();
    fn exc14_stub();
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub unsafe fn init() {
    // Vectors 0–31 are reserved for exceptions.
    // We install specific stubs for the ones we care about,
    // and let others fall through to a generic halt if triggered.
    IDT[0].set_handler(exc0_stub);   // Divide by Zero
    IDT[6].set_handler(exc6_stub);   // Invalid Opcode
    IDT[13].set_handler(exc13_stub); // GPF
    IDT[14].set_handler(exc14_stub); // Page Fault

    // Hardware IRQs (after PIC remapping: IRQ0 → 32, IRQ1 → 33)
    IDT[32].set_handler(pit_interrupt);
    IDT[33].set_handler(keyboard_interrupt);

    let idt_ptr = IdtPointer {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: &raw const IDT as *const _ as u64,
    };

    core::arch::asm!("lidt [{}]", in(reg) &idt_ptr, options(nostack, nomem));
    crate::dbg_log!("IDT", "loaded ({} entries)", IDT.len());

    // Remap PIC and set IRQ masks
    crate::hw::pic::init();
}

