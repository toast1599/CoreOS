# Variables
IMAGE = coreos.img
# Cargo produces a binary named 'kernel' (no extension) in this specific path
KERNEL_ELF = kernel/target/x86_64-unknown-none/release/kernel
KERNEL_BIN = kernel.bin
LOADER_EFI = BOOTX64.EFI

# Compilation Flags
CFLAGS = -target x86_64-pc-win32 -I/usr/include/efi -I/usr/include/efi/x86_64 -ffreestanding -fshort-wchar -mno-red-zone
LDFLAGS = /subsystem:efi_application /entry:efi_main /base:0x0 /align:4096

.PHONY: all clean run

all: $(IMAGE)

# 1. Build Rust Kernel using Cargo
$(KERNEL_BIN): kernel/src/main.rs kernel/linker.ld
	cd kernel && cargo build --release --target x86_64-unknown-none
	# -S removes debug symbols/tables but keeps all functional code/data sections
	objcopy -O binary $(KERNEL_ELF) $(KERNEL_BIN)
	
# 2. Build C Loader
$(LOADER_EFI): main.c
	@echo "Building UEFI Loader..."
	clang $(CFLAGS) -c main.c -o main.o
	lld-link $(LDFLAGS) /out:$(LOADER_EFI) main.o

# 3. Build Disk Image
$(IMAGE): $(KERNEL_BIN) $(LOADER_EFI)
	@echo "Creating Disk Image..."
	dd if=/dev/zero of=$(IMAGE) bs=1M count=64
	mkfs.fat -F 32 $(IMAGE)
	mmd -i $(IMAGE) ::/EFI
	mmd -i $(IMAGE) ::/EFI/BOOT
	mcopy -i $(IMAGE) $(LOADER_EFI) ::/EFI/BOOT/
	mcopy -i $(IMAGE) $(KERNEL_BIN) ::/

# 4. Run in QEMU
run: $(IMAGE)
	qemu-system-x86_64 -bios /usr/share/ovmf/x64/OVMF.4m.fd \
		-drive file=$(IMAGE),format=raw,if=ide \
		-net none \
		-serial stdio \
		-vga std \
		-m 512M

# 5. Clean up
clean:
	rm -rf *.o *.EFI *.img *.bin
	cd kernel && cargo clean
