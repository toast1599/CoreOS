ENTRY(_start)

SECTIONS
{
    . = 0x100000;
    .text : AT(0x100000) {
        *(.text._start)
        *(.text .text.*)
    }

    .rodata : { *(.rodata .rodata.*) }
    .data   : { *(.data .data.*) }
    .bss    : { *(.bss .bss.*) }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)  /* ADD THIS LINE */
        *(.rel.plt)
        *(.dynsym)
        *(.gnu.hash)
        *(.hash)
        *(.dynamic)
    }
}
