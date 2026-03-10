# CoreOS

A new AMD64 operating system written in Rust with a C UEFI bootloader (included).

## Status
Early kernel — ring 3 userspace + libc port in progress.

## Features
- UEFI bootloader (C)
- Physical memory manager (bitmap)
- 4-level paging (identity mapped, 4GB)
- Slab heap allocator
- Preemptive round-robin scheduler
- PS/2 keyboard, PIT timer, RTC
- RAM filesystem
- VGA framebuffer + PSF font rendering
- Serial debug output
- Interactive shell
- Syscall gate (ring 3 in progress)

## Requirements
- Rust nightly
- clang + lld
- OVMF (UEFI firmware)
- QEMU
- mtools (mcopy, mmd, mkfs.fat)

## Build & Run
\`\`\`
make        # build disk image
make run    # launch in QEMU
\`\`\`

## Architecture
- `bootloader/` — UEFI app, loads kernel.bin, passes framebuffer + memory map
- `kernel/` — Rust no_std kernel
- `Makefile` — builds bootloader, kernel, and FAT32 disk image
```
