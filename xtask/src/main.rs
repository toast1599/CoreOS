mod helper;
use crate::helper::make_bar;

use anyhow::Result;
use std::process::Command;
use xshell::{Shell, cmd};

fn main() -> Result<()> {
    let sh = Shell::new()?;
    let cmd_name = std::env::args().nth(1).unwrap_or_default();

    match cmd_name.as_str() {
        "build-kernel" => build_kernel(&sh)?,
        "build-bootloader" => build_bootloader(&sh)?,
        "gen-syscalls" => gen_syscalls(&sh)?,
        "user" => build_user(&sh)?,
        "image" => build_image(&sh)?,
        "run" => run_qemu(&sh)?,
        "clean" => clean(&sh)?,

        "build" => {
            let steps = 6;
            let pb = make_bar(steps);

            pb.set_message("Generating syscalls");
            gen_syscalls(&sh)?;
            pb.inc(1);

            pb.set_message("Building userland");
            build_user(&sh)?;
            pb.inc(1);

            pb.set_message("Building kernel");
            build_kernel(&sh)?;
            pb.inc(1);

            pb.set_message("Building bootloader");
            build_bootloader(&sh)?;
            pb.inc(1);

            pb.set_message("Creating disk image");
            build_image(&sh)?;
            pb.inc(1);

            pb.finish_with_message("It was succesful.");
        }

        _ => {
            eprintln!("usage: cargo xtask build");
        }
    }

    Ok(())
}

fn build_kernel(sh: &Shell) -> Result<()> {
    // Build kernel
    cmd!(
        sh,
        "cargo build -p kernel --quiet --release --target x86_64-unknown-none"
    )
    .quiet()
    .run()?;

    // Sanity check

    std::fs::create_dir_all("build/esp")?;

    // 1. Define variables in the local scope first
    let input = "target/x86_64-unknown-none/release/kernel";
    let output = "build/esp/kernel.bin";

    // 2. Interpolate them directly in the string
    cmd!(sh, "objcopy -O binary {input} {output}")
        .quiet()
        .run()?;

    Ok(())
}

fn build_bootloader(sh: &Shell) -> Result<()> {
    // Build bootloader
    cmd!(
        sh,
        "cargo build --quiet -p rust-loader --release --target x86_64-unknown-uefi"
    )
    .quiet()
    .run()?;

    // Ensure dirs exist
    std::fs::create_dir_all("build/esp/EFI/BOOT")?;

    // Copy EFI binary
    std::fs::copy(
        "bootloader/target/x86_64-unknown-uefi/release/rust-loader.efi",
        "build/esp/EFI/BOOT/BOOTX64.EFI",
    )?;

    Ok(())
}

fn gen_syscalls(sh: &Shell) -> Result<()> {
    cmd!(sh, "python3 tools/generate_syscalls.py")
        .quiet()
        .run()?;
    Ok(())
}

fn build_user(sh: &Shell) -> Result<()> {
    cmd!(sh, "make -s -C user").quiet().run()?;
    Ok(())
}

fn build_image(sh: &Shell) -> Result<()> {
    let image = "build/coreos.img";

    // Create empty disk
    cmd!(sh, "dd if=/dev/zero status=none of={image} bs=1M count=64")
        .quiet()
        .run()?;

    // Format FAT32
    let output = Command::new("mkfs.fat")
        .args(["-F", "32", image])
        .output()?; // capture stdout + stderr

    if !output.status.success() {
        anyhow::bail!(
            "mkfs.fat failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Create directories
    cmd!(sh, "mmd -i {image} ::/EFI").quiet().run()?;
    cmd!(sh, "mmd -i {image} ::/EFI/BOOT").quiet().run()?;

    // Copy bootloader + kernel
    cmd!(
        sh,
        "mcopy -i {image} build/esp/EFI/BOOT/BOOTX64.EFI ::/EFI/BOOT/"
    )
    .quiet()
    .run()?;
    cmd!(sh, "mcopy -i {image} build/esp/kernel.bin ::/")
        .quiet()
        .run()?;

    // Copy user programs
    cmd!(sh, "mcopy -i {image} user/shell.elf ::/test.elf")
        .quiet()
        .run()?;
    cmd!(
        sh,
        "mcopy -i {image} user/syscall_test.elf ::/syscall_test.elf"
    )
    .quiet()
    .run()?;
    cmd!(
        sh,
        "mcopy -i {image} user/syscall_child.elf ::/syscall_child.elf"
    )
    .quiet()
    .run()?;
    cmd!(
        sh,
        "mcopy -i {image} user/posix_newsys_test.elf ::/posix_newsys_test.elf"
    )
    .quiet()
    .run()?;

    // Assets
    cmd!(sh, "mcopy -i {image} assets/font.psfu ::/")
        .quiet()
        .run()?;

    Ok(())
}

fn run_qemu(sh: &Shell) -> Result<()> {
    let image = "build/coreos.img";
    let tmp = "/tmp/coreos-run.img";

    // Copy image (snapshot-like behavior)
    std::fs::copy(image, tmp)?;

    cmd!(
        sh,
        "qemu-system-x86_64 -snapshot -bios /usr/share/ovmf/x64/OVMF.4m.fd -drive file={tmp},format=raw,if=ide -net none -serial stdio -m 512M -no-reboot -no-shutdown"
    )
    .quiet().run()?;
    std::fs::remove_file(tmp).ok();

    Ok(())
}

fn clean(sh: &Shell) -> Result<()> {
    // Remove build artifacts
    std::fs::remove_dir_all("build").ok();

    // Clean kernel + bootloader via cargo
    cmd!(sh, "cargo clean").quiet().run()?;

    // Clean userland
    cmd!(sh, "make -s -C user clean").quiet().run()?;

    Ok(())
}
