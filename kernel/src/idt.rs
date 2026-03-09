use crate::default_exception;
use core::mem::size_of;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IDTEntry {
    offset_low: u16,
    selector: u16,
    options: u16,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

core::arch::global_asm!(
    r#"
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

    mov rdi, rsp
    call pit_handler

    mov al, 0x20
    out 0x20, al

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

.global keyboard_interrupt
.extern keyboard_handler

keyboard_interrupt:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax

    call keyboard_handler

    mov al, 0x20
    out 0x20, al

    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15
    iretq    
"#
);

impl IDTEntry {
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
        self.selector = 0x38; // kernel code segment
        self.options = 0x8E00; // present, interrupt gate
        self.offset_mid = (addr >> 16) as u16;
        self.offset_high = (addr >> 32) as u32;
        self.zero = 0;
    }
}

#[repr(C, packed)]
struct IDTPointer {
    limit: u16,
    base: u64,
}

static mut IDT: [IDTEntry; 256] = [IDTEntry::missing(); 256];

extern "C" {
    fn keyboard_interrupt();
    fn pit_interrupt();
}

pub unsafe fn init_idt() {
    for i in 0..30 {
        IDT[i].set_handler(default_exception);
    }

    IDT[32].set_handler(pit_interrupt);
    IDT[33].set_handler(keyboard_interrupt);

    let idt_ptr = IDTPointer {
        limit: (size_of::<[IDTEntry; 256]>() - 1) as u16,
        base: &raw const IDT as *const _ as u64,
    };

    core::arch::asm!("lidt [{}]", in(reg) &idt_ptr);
    crate::dbg_log!("IDT", "IDT loaded, {} entries", IDT.len());
    // remap PIC
    core::arch::asm!("mov al, 0x11");
    core::arch::asm!("out 0x20, al");
    core::arch::asm!("out 0xA0, al");

    core::arch::asm!("mov al, 0x20");
    core::arch::asm!("out 0x21, al");

    core::arch::asm!("mov al, 0x28");
    core::arch::asm!("out 0xA1, al");

    core::arch::asm!("mov al, 0x04");
    core::arch::asm!("out 0x21, al");

    core::arch::asm!("mov al, 0x02");
    core::arch::asm!("out 0xA1, al");

    core::arch::asm!("mov al, 0x01");
    core::arch::asm!("out 0x21, al");
    core::arch::asm!("out 0xA1, al");

    // enable timer and keyboard
    core::arch::asm!("mov al, 0xFC");
    core::arch::asm!("out 0x21, al");
    core::arch::asm!("mov al, 0xFF");
    core::arch::asm!("out 0xA1, al");
}
