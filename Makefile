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
# Project Context Dump
# =========================

CONTEXT_FILE = .project_context.txt

context:
	@echo "Generating project context..."
	@echo "# PROJECT CONTEXT DUMP - $(shell date)" > $(CONTEXT_FILE)
	@echo "## Directory Tree" >> $(CONTEXT_FILE)
	@tree -I 'target|build|.git' >> $(CONTEXT_FILE)
	@echo "\n## Makefile" >> $(CONTEXT_FILE)
	@cat Makefile >> $(CONTEXT_FILE)
	@echo "\n## Bootloader Source" >> $(CONTEXT_FILE)
	@cat $(LOADER_SRC) >> $(CONTEXT_FILE)
	@echo "\n## Kernel Configuration" >> $(CONTEXT_FILE)
	@cat kernel/Cargo.toml >> $(CONTEXT_FILE)
	@cat kernel/linker.ld >> $(CONTEXT_FILE)
	@echo "\n## Kernel Source" >> $(CONTEXT_FILE)
	@find kernel/src -name "*.rs" -exec echo "\n--- File: {} ---" \; -exec cat {} \; >> $(CONTEXT_FILE)
	@echo "Context written to $(CONTEXT_FILE)"

copy: context
	@cat $(CONTEXT_FILE) | wl-copy
	@echo "Context copied to clipboard."

# =========================
# Clean
# =========================

clean:
	rm -rf $(BUILD_DIR)
	cd kernel && cargo clean
