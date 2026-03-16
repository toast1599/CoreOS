# CoreOS

CoreOS is an experimental **AMD64 monolithic operating system** written in Rust with a small C-based UEFI bootloader.

The project focuses on low-level OS architecture: bootstrapping, paging, memory allocation, process scheduling, syscall handling, and a small ring 3 userspace.

## Status

Active development.

The current tree boots through UEFI into a Rust `no_std` kernel, runs an embedded syscall test suite in ring 3, and launches a userspace shell from an in-memory filesystem.

## Features

### Boot

* UEFI bootloader written in C
* Custom boot protocol passing framebuffer and memory map
* Loads kernel, font, and initial userspace ELF
* Hands off into a higher-half Rust kernel

### Kernel

* Rust `no_std` kernel
* Physical memory manager (bitmap)
* 4-level paging with direct-map and higher-half kernel mappings
* Slab heap allocator
* Preemptive round-robin scheduler
* Task and process infrastructure
* ELF loader for userspace programs
* Syscall entry path with file, process, memory, and time primitives

### Hardware Support

* PIT timer
* PS/2 keyboard driver
* RTC clock
* Serial debugging (COM1)

### Filesystem

* In-memory RAM filesystem
* VFS-style facade over RamFS
* Basic file operations (`open`, `read`, `write`, `create`, `remove`, `stat`, `lseek`)
* Pipe-backed file descriptors

### Graphics

* Linear framebuffer rendering
* PSF font support
* Kernel header bar and userspace text console

### Userspace

* Ring 3 execution
* ELF program loading
* Interactive shell
* Minimal libc shim
* Syscall regression test program

## Requirements

* Rust nightly
* clang + lld
* QEMU
* OVMF (UEFI firmware)
* mtools (`mcopy`, `mmd`, `mkfs.fat`)

## Build & Run

```sh
make
make run
```

`make` builds the user programs, kernel, UEFI bootloader, and a bootable FAT disk image at `build/coreos.img`.

## Validation

The normal serial boot path currently does the following:

* boots through OVMF/UEFI
* initializes memory, paging, interrupts, syscalls, and tasking
* runs the embedded `syscall_test` userspace binary
* starts the userspace shell

The syscall test covers basic process, FD, pipe, memory, timing, and filesystem operations.

## Project Layout

```text
bootloader/   UEFI loader and boot protocol definitions
kernel/       Rust kernel source
user/         Userspace programs, startup asm, and libc shim
assets/       Fonts and static resources
build/        Generated EFI image and disk image artifacts
Makefile      Top-level build and run targets
```

## Internal Layout

Notable kernel areas:

* `kernel/src/main.rs`: boot orchestration and subsystem bring-up
* `kernel/src/arch/amd64/`: GDT, IDT, paging, and CPU helpers
* `kernel/src/mem/`: PMM and slab allocator
* `kernel/src/proc/`: ELF loading, tasking, process state, FDs, and VM metadata
* `kernel/src/syscall/`: syscall entry and domain-specific syscall handlers
* `kernel/src/drivers/` and `kernel/src/hw/`: framebuffer, serial, PIC, PIT, keyboard, RTC
* `kernel/src/vfs.rs`: VFS-facing wrapper around the in-memory filesystem
