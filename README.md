# CoreOS

CoreOS is a hobby **AMD64 monolithic operating system** written in Rust with a small C-based UEFI bootloader.

The project focuses on exploring low-level OS architecture, including memory management, process scheduling, and userspace execution.

## Status

Active development.

The kernel currently supports **ring 3 userspace programs**, a preemptive scheduler, and a minimal syscall interface. A small libc layer and additional userspace utilities are in progress.

## Features

### Boot

* UEFI bootloader written in C
* Custom boot protocol passing framebuffer and memory map
* Loads kernel and initial userspace ELF

### Kernel

* Rust `no_std` kernel
* Physical memory manager (bitmap)
* 4-level paging (identity mapped, first 4 GB)
* Slab heap allocator
* Preemptive round-robin scheduler
* Task and process infrastructure
* Basic syscall interface

### Hardware Support

* PIT timer
* PS/2 keyboard driver
* RTC clock
* Serial debugging (COM1)

### Filesystem

* In-memory RAM filesystem
* Basic file operations (create, read, write, remove)

### Graphics

* Linear framebuffer rendering
* PSF font support
* Text console

### Userspace

* Ring 3 execution
* ELF program loading
* Interactive shell
* libc shim (in progress)

## Requirements

* Rust nightly
* clang + lld
* QEMU
* OVMF (UEFI firmware)
* mtools (`mcopy`, `mmd`, `mkfs.fat`)

## Build & Run

```
make
make run
```

## Project Layout

```
bootloader/   UEFI loader responsible for boot services and kernel loading
kernel/       Rust kernel implementation
user/         Userspace programs and libc shim
assets/       Fonts and static resources
Makefile      Build system and disk image generation
```
