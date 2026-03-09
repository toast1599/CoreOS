ENTRY(_start)

SECTIONS
{
    . = 0x100000;

    .text : AT(0x100000) {
        *(.text._start)
        *(.text .text.*)
    }

    .rodata : { *(.rodata .rodata.*) }

    .data : { *(.data .data.*) }

    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    .stack (NOLOAD) : {
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
