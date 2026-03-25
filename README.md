# CoreOS

CoreOS is an experimental AMD64 operating system with a Rust `no_std` kernel, a
small C UEFI bootloader, and a minimal C userspace.

## Current State

The tree boots through UEFI into the kernel, initializes memory management,
interrupts, scheduling, VFS, and syscall handling, runs embedded ring 3 test
ELFs, and then starts a userspace shell from an in-memory filesystem.

Current syscall work is aimed at a future `musl` (libc) port. Several
Linux-numbered syscalls are already implemented and exercised at boot, but the
ABI is not yet Linux-compatible enough for an unmodified `musl` userspace.

## Includes

- UEFI boot path and higher-half kernel handoff
- AMD64 paging, PMM, heap allocation, and task/process support
- Preemptive scheduler and ring 3 ELF loading
- RamFS-backed VFS and pipe-backed file descriptors
- Framebuffer console, PS/2 keyboard, PIT, RTC, and serial output
- Minimal libc shim plus embedded syscall regression tests

## Build

Requirements:

- Rust nightly
- `clang` and `lld`
- QEMU
- OVMF
- `mtools`

Build and run:

```sh
make
make run
```

`make` produces a bootable image at `build/coreos.img`.

## Repository Layout

```text
bootloader/   UEFI loader
kernel/       Rust kernel
user/         Userspace programs and libc shim
assets/       Fonts and static assets
build/        Generated build artifacts
Makefile      Top-level build and run targets
```
