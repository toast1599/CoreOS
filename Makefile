# =========================
# Directories
# =========================

BUILD_DIR = build
ESP_DIR   = $(BUILD_DIR)/esp
EFI_BOOT  = $(ESP_DIR)/EFI/BOOT

IMAGE     = $(BUILD_DIR)/coreos.img

# =========================
# Kernel (Rust)
# =========================

KERNEL_ELF = kernel/target/x86_64-unknown-none/release/kernel
KERNEL_BIN = $(ESP_DIR)/kernel.bin

# =========================
# Bootloader (C UEFI)
# =========================

LOADER_SRC = bootloader/main.c
LOADER_EFI = $(EFI_BOOT)/BOOTX64.EFI

CFLAGS  = -target x86_64-pc-win32 \
          -I/usr/include/efi \
          -I/usr/include/efi/x86_64 \
          -ffreestanding -fshort-wchar -mno-red-zone

LDFLAGS = /subsystem:efi_application \
          /entry:efi_main \
          /base:0x0 \
          /align:4096

.PHONY: all clean run dirs

all: $(IMAGE)

# =========================
# Create Build Directories
# =========================

dirs:
	mkdir -p $(EFI_BOOT)

# =========================
# Build Rust Kernel
# =========================

$(KERNEL_BIN): dirs
	cd kernel && \
	rustup override set nightly && \
	cargo build --release \
	    -Zbuild-std=core,alloc \
	    --target x86_64-unknown-none
	objcopy -O binary $(KERNEL_ELF) $(KERNEL_BIN)

# =========================
# Build UEFI Bootloader
# =========================

$(LOADER_EFI): dirs $(LOADER_SRC)
	clang $(CFLAGS) -c $(LOADER_SRC) -o $(BUILD_DIR)/main.o
	lld-link $(LDFLAGS) /out:$(LOADER_EFI) $(BUILD_DIR)/main.o
	rm $(BUILD_DIR)/main.o

# =========================
# Build Disk Image
# =========================

$(IMAGE): $(KERNEL_BIN) $(LOADER_EFI)
	dd if=/dev/zero of=$(IMAGE) bs=1M count=64
	mkfs.fat -F 32 $(IMAGE)
	mmd -i $(IMAGE) ::/EFI
	mmd -i $(IMAGE) ::/EFI/BOOT
	mcopy -i $(IMAGE) $(LOADER_EFI) ::/EFI/BOOT/
	mcopy -i $(IMAGE) $(KERNEL_BIN) ::/

# =========================
# Run in QEMU
# =========================

run: $(IMAGE)
	qemu-system-x86_64 \
		-bios /usr/share/ovmf/x64/OVMF.4m.fd \
		-drive file=$(IMAGE),format=raw,if=ide \
		-net none \
		-serial stdio \
		-vga std \
		-m 512M

# =========================
# Clean
# =========================

clean:
	rm -rf $(BUILD_DIR)
	cd kernel && cargo clean
