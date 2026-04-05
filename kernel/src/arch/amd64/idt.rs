/// Interrupt Descriptor Table.
///
/// Installs handlers for:
///   - Vectors 0–29: default exception handler (halts)
///   - Vector 32 (IRQ 0): PIT timer
///   - Vector 33 (IRQ 1): PS/2 keyboard
///
/// PIC remapping is handled by `hw::pic::init()`.
use crate::arch::gdt::SEG_KCODE;
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
.extern default_exception

.macro EXC_NOERR num
.global exc\num\()_stub
exc\num\()_stub:
    push 0
    push \num
    push 0
    jmp exc_common
.endm

.macro EXC_ERR num
.global exc\num\()_stub
exc\num\()_stub:
    push \num
    push 0
    jmp exc_common
.endm

EXC_NOERR 0
EXC_NOERR 1
EXC_NOERR 2
EXC_NOERR 3
EXC_NOERR 4
EXC_NOERR 5
EXC_NOERR 6
EXC_NOERR 7
EXC_ERR   8
EXC_NOERR 9
EXC_ERR   10
EXC_ERR   11
EXC_ERR   12
EXC_ERR   13
EXC_ERR   14
EXC_NOERR 15
EXC_NOERR 16
EXC_ERR   17
EXC_NOERR 18
EXC_NOERR 19
EXC_NOERR 20
EXC_ERR   21
EXC_NOERR 22
EXC_NOERR 23
EXC_NOERR 24
EXC_NOERR 25
EXC_NOERR 26
EXC_NOERR 27
EXC_NOERR 28
EXC_NOERR 29
EXC_ERR   30
EXC_NOERR 31

exc_common:
    cli
    // Current stack: [padding], [vector], [error_code], [rip], [cs], [rflags], [rsp], [ss]
    // Save all general purpose registers
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

    mov rdi, rsp        // Pass the pointer to the saved frame as first arg
    call default_exception

    // We shouldn't return from default_exception, but if we do...
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
    add rsp, 24         // Clean up padding, vector, and error code
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
    fn exc1_stub();
    fn exc2_stub();
    fn exc3_stub();
    fn exc4_stub();
    fn exc5_stub();
    fn exc6_stub();
    fn exc7_stub();
    fn exc8_stub();
    fn exc9_stub();
    fn exc10_stub();
    fn exc11_stub();
    fn exc12_stub();
    fn exc13_stub();
    fn exc14_stub();
    fn exc15_stub();
    fn exc16_stub();
    fn exc17_stub();
    fn exc18_stub();
    fn exc19_stub();
    fn exc20_stub();
    fn exc21_stub();
    fn exc22_stub();
    fn exc23_stub();
    fn exc24_stub();
    fn exc25_stub();
    fn exc26_stub();
    fn exc27_stub();
    fn exc28_stub();
    fn exc29_stub();
    fn exc30_stub();
    fn exc31_stub();
}

static EXCEPTION_STUBS: [unsafe extern "C" fn(); 32] = [
    exc0_stub, exc1_stub, exc2_stub, exc3_stub, exc4_stub, exc5_stub, exc6_stub, exc7_stub,
    exc8_stub, exc9_stub, exc10_stub, exc11_stub, exc12_stub, exc13_stub, exc14_stub, exc15_stub,
    exc16_stub, exc17_stub, exc18_stub, exc19_stub, exc20_stub, exc21_stub, exc22_stub,
    exc23_stub, exc24_stub, exc25_stub, exc26_stub, exc27_stub, exc28_stub, exc29_stub,
    exc30_stub, exc31_stub,
];

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub unsafe fn init() {
    for (vector, handler) in EXCEPTION_STUBS.iter().copied().enumerate() {
        IDT[vector].set_handler(handler);
    }

    // Hardware IRQs (after PIC remapping: IRQ0 -> 32, IRQ1 -> 33)
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
