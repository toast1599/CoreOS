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

core::arch::global_asm!(r#"
.intel_syntax noprefix
.global keyboard_interrupt
.extern keyboard_handler

keyboard_interrupt:
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    call keyboard_handler

    mov al, 0x20
    out 0x20, al

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq
"#);

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

    fn set_handler(&mut self, handler: extern "C" fn()) {
        let addr = handler as u64;
        self.offset_low = addr as u16;
        self.selector = 0x08; // kernel code segment
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
}

pub unsafe fn init_idt() {
    IDT[33].set_handler(keyboard_interrupt);

    let idt_ptr = IDTPointer {
        limit: (size_of::<[IDTEntry; 256]>() - 1) as u16,
        base: &IDT as *const _ as u64,
    };

    core::arch::asm!("lidt [{}]", in(reg) &idt_ptr);
    core::arch::asm!("mov al, 0xFF");
    core::arch::asm!("out 0x21, al"); // mask master PIC
    core::arch::asm!("out 0xA1, al"); // mask slave PIC
}
