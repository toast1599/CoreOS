# =========================
# Directories
# =========================

BUILD_DIR = build
ESP_DIR   = $(BUILD_DIR)/esp
EFI_BOOT  = $(ESP_DIR)/EFI/BOOT
SYSCALL_DEF = abi/syscalls.def
SYSCALL_GEN = tools/generate_syscalls.py
SYSCALL_OUT = kernel/src/syscall/nr.rs kernel/src/syscall/dispatch.rs user/libcshim/sysnr.h

IMAGE     = $(BUILD_DIR)/coreos.img
USER_ELFS = user/shell.elf user/syscall_test.elf user/syscall_child.elf user/posix_newsys_test.elf

# =========================
# Kernel (Rust)
# =========================

KERNEL_ELF = kernel/target/x86_64-unknown-none/release/kernel
KERNEL_BIN = $(ESP_DIR)/kernel.bin

# =========================
# Bootloader (Rust UEFI)
# =========================

LOADER_CRATE = bootloader
LOADER_BUILD_EFI = $(LOADER_CRATE)/target/x86_64-unknown-uefi/release/rust-loader.efi
LOADER_EFI = $(EFI_BOOT)/BOOTX64.EFI

.PHONY: all clean run dirs syscall-api

all: $(IMAGE)

syscall-api: $(SYSCALL_OUT)

$(SYSCALL_OUT): $(SYSCALL_DEF) $(SYSCALL_GEN)
	python3 $(SYSCALL_GEN)

# =========================
# Create Build Directories
# =========================

dirs:
	mkdir -p $(EFI_BOOT)

# =========================
# Build Rust Kernel
# =========================

$(KERNEL_BIN): dirs $(SYSCALL_OUT) $(USER_ELFS)
	cd kernel && \
	cargo +nightly build --release \
	    -Zbuild-std=core,alloc \
	    --target x86_64-unknown-none
	objcopy -O binary $(KERNEL_ELF) $(KERNEL_BIN)

# =========================
# Build UEFI Bootloader
# =========================

$(LOADER_EFI): dirs $(LOADER_CRATE)/Cargo.toml $(wildcard $(LOADER_CRATE)/src/*.rs)
	cd $(LOADER_CRATE) && \
	cargo build --release --target x86_64-unknown-uefi --locked --offline
	cp $(LOADER_BUILD_EFI) $(LOADER_EFI)

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
	mcopy -i $(IMAGE) user/posix_newsys_test.elf ::/posix_newsys_test.elf
	mcopy -i $(IMAGE) assets/font.psfu ::/

$(USER_ELFS): dirs $(SYSCALL_OUT)
	$(MAKE) -C user
		
# =========================
# Run in QEMU
# =========================

run: $(IMAGE)
	tmp_img=/tmp/coreos-run-$$$$.img; \
	cp $(IMAGE) $$tmp_img; \
	qemu-system-x86_64 \
		-snapshot \
		-bios /usr/share/ovmf/x64/OVMF.4m.fd \
		-drive file=$$tmp_img,format=raw,if=ide \
		-net none \
		-serial stdio \
		-display none \
		-m 512M \
		-no-reboot -no-shutdown; \
	rm -f $$tmp_img

run-gfx: $(IMAGE)
	tmp_img=/tmp/coreos-run-gfx-$$$$.img; \
	cp $(IMAGE) $$tmp_img; \
	qemu-system-x86_64 \
		-bios /usr/share/ovmf/x64/OVMF.4m.fd \
		-drive file=$$tmp_img,format=raw,if=ide \
		-net none \
		-serial stdio \
		-vga std \
		-m 512M \
		-no-reboot -no-shutdown; \
	rm -f $$tmp_img
# =========================
# Project Context Dump
# =========================

CONTEXT_FILE = ../.project_context.md

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

	@find .. \
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
	-name "*.sh" -o \
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
