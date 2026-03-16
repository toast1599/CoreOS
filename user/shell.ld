OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Load the shell above the kernel (which sits at 1 MB) and the
       ELF the bootloader might place at 2 MB. 4 MB is safe. */
    . = 0x400000;

    .text : {
        *(.text._start)
        *(.text .text.*)
    } :text

    .rodata : { *(.rodata .rodata.*) } :text

    .data : { *(.data .data.*) } :data

    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } :data

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
    }
}
