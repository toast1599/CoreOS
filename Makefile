# =========================
# Directories
# =========================

BUILD_DIR = build
ESP_DIR   = $(BUILD_DIR)/esp
EFI_BOOT  = $(ESP_DIR)/EFI/BOOT

IMAGE     = $(BUILD_DIR)/coreos.img
USER_ELFS = user/shell.elf user/syscall_test.elf user/syscall_child.elf

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
          /base:0x0

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

$(KERNEL_BIN): dirs $(USER_ELFS)
	cd kernel && \
	cargo +nightly build --release \
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

$(IMAGE): $(KERNEL_BIN) $(LOADER_EFI) $(USER_ELFS)
	dd if=/dev/zero of=$(IMAGE) bs=1M count=64
	mkfs.fat -F 32 $(IMAGE)
	mmd -i $(IMAGE) ::/EFI
	mmd -i $(IMAGE) ::/EFI/BOOT
	mcopy -i $(IMAGE) $(LOADER_EFI) ::/EFI/BOOT/
	mcopy -i $(IMAGE) $(KERNEL_BIN) ::/
	mcopy -i $(IMAGE) user/shell.elf ::/test.elf
	mcopy -i $(IMAGE) user/syscall_test.elf ::/syscall_test.elf
	mcopy -i $(IMAGE) user/syscall_child.elf ::/syscall_child.elf
	mcopy -i $(IMAGE) assets/font.psfu ::/

$(USER_ELFS): dirs
	$(MAKE) -C user
		
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
		-m 512M \
		-d cpu_reset \
		-no-reboot -no-shutdown \
# =========================
# Project Context Dump
# =========================

CONTEXT_FILE = .project_context.md

context:
	@echo "Generating project context..."
	@echo "# PROJECT CONTEXT DUMP - $$(date)" > $(CONTEXT_FILE)

	@echo "\n## Directory Tree" >> $(CONTEXT_FILE)
	@eza --tree -a --git-ignore \
	--ignore-glob='.git|target|build|node_modules|*.tmp|*.temp|*.log|*.cache|*.swp|*.bak|*.o|*.out|*.class|*.pyc|*.lock|*.bin|*.img|*.iso|__pycache__' \
	>> $(CONTEXT_FILE)

	@echo "\n## Makefile" >> $(CONTEXT_FILE)
	@echo '```make' >> $(CONTEXT_FILE)
	@cat Makefile >> $(CONTEXT_FILE)
	@echo '```' >> $(CONTEXT_FILE)

	@echo "\n## Source Files" >> $(CONTEXT_FILE)

	@find . \
	-type d \( -name .git -o -name build -o -name target -o -name node_modules -o -name __pycache__ \) -prune -o \
	-type f \( \
	-name "*.c" -o \
	-name "*.h" -o \
	-name "*.cpp" -o \
	-name "*.hpp" -o \
	-name "*.rs" -o \
	-name "*.asm" -o \
	-name "*.s" -o \
	-name "*.S" -o \
	-name "*.py" -o \
	-name "*.go" -o \
	-name "*.js" -o \
	-name "*.ts" -o \
	-name "*.java" -o \
	-name "*.zig" -o \
	-name "*.lua" \
	\) | while read file; do \
		ext=$${file##*.}; \
		echo "\n### File: $$file" >> $(CONTEXT_FILE); \
		echo "\`\`\`$$ext" >> $(CONTEXT_FILE); \
		cat "$$file" >> $(CONTEXT_FILE); \
		echo "\`\`\`" >> $(CONTEXT_FILE); \
	done

	@echo "\n---" >> $(CONTEXT_FILE)
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
	$(MAKE) -C user clean
