ENTRY(_start)

PHYS_OFFSET = 0xFFFFFFFF80000000;

SECTIONS
{
    . = PHYS_OFFSET + 0x100000;

    .text : AT(ADDR(.text) - PHYS_OFFSET) {
        *(.text._start)
        *(.text .text.*)
    }

    .rodata : AT(ADDR(.rodata) - PHYS_OFFSET) {
        *(.rodata .rodata.*)
    }

    .data : AT(ADDR(.data) - PHYS_OFFSET) {
        *(.data .data.*)
    }

    .bss : AT(ADDR(.bss) - PHYS_OFFSET) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    .stack (NOLOAD) : AT(ADDR(.stack) - PHYS_OFFSET) {
        . = ALIGN(4096);
        __stack_bottom = .;
        . += 0x200000;
        __stack_top = .;
    }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.rel.plt)
        *(.dynsym)
        *(.gnu.hash)
        *(.hash)
        *(.dynamic)
    }
}
