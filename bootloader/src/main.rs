#![no_main]
#![no_std]

use core::panic::PanicInfo;
use core::ptr::addr_of_mut;

use uefi::boot::{self, MemoryType};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::Status;

mod bootinfo;
mod file_loader;
mod memory;
mod paging;
mod serial;

use bootinfo::{CoreOsBootInfo, KernelEntry};
use file_loader::{load_optional_file, load_required_file, LoadSpec};

static mut BOOT_INFO: CoreOsBootInfo = CoreOsBootInfo::zeroed();

#[entry]
fn efi_main(image_handle: Handle, _system_table: SystemTable<Boot>) -> Status {
    serial::init();
    serial::log("rust bootloader: entry\n");

    let boot_info = unsafe { &mut *addr_of_mut!(BOOT_INFO) };
    boot_info.tsc_bootloader_start = memory::rdtsc();
    serial::log("rust bootloader: tsc captured\n");

    match run(image_handle, boot_info) {
        Ok(()) => Status::SUCCESS,
        Err(err) => {
            serial::log("rust bootloader: failed\n");
            serial::log_status("status=", err.status());
            err.status()
        }
    }
}

fn run(image_handle: Handle, boot_info: &mut CoreOsBootInfo) -> uefi::Result<()> {
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>()?;
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)?;
    serial::log("rust bootloader: GOP open ok\n");

    let mut fs = boot::get_image_file_system(image_handle)?;
    let mut root = fs.open_volume()?;
    serial::log("rust bootloader: ESP root open ok\n");

    let kernel = load_required_file(
        &mut root,
        cstr16!("\\kernel.bin"),
        LoadSpec::kernel_with_extra(128 * 1024 * 1024),
    )?;
    boot_info.kernel_phys_base = kernel.addr;
    boot_info.kernel_alloc_size = kernel.allocation_size as u64;
    serial::log_hex("kernel load addr=", kernel.addr);
    serial::log_hex("kernel file size=", kernel.file_size as u64);

    let mode = gop.current_mode_info();
    let mut frame_buffer = gop.frame_buffer();
    boot_info.fb_base = frame_buffer.as_mut_ptr() as u64;
    boot_info.fb_size = frame_buffer.size() as u64;
    boot_info.width = mode.resolution().0 as u32;
    boot_info.height = mode.resolution().1 as u32;
    boot_info.pitch = mode.stride() as u32;
    serial::log("rust bootloader: framebuffer captured\n");

    match load_optional_file(&mut root, cstr16!("\\test.elf"), LoadSpec::asset())? {
        Some(user_elf) => {
            boot_info.user_elf_base = user_elf.addr;
            boot_info.user_elf_size = user_elf.file_size as u64;
            serial::log_hex("user elf addr=", user_elf.addr);
            serial::log_hex("user elf size=", user_elf.file_size as u64);
        }
        None => serial::log("rust bootloader: test.elf missing\n"),
    }

    match load_optional_file(&mut root, cstr16!("\\font.psfu"), LoadSpec::asset())? {
        Some(font) => {
            boot_info.font_base = font.addr;
            boot_info.font_size = font.file_size as u64;
            serial::log_hex("font addr=", font.addr);
            serial::log_hex("font size=", font.file_size as u64);
        }
        None => serial::log("rust bootloader: font.psfu missing\n"),
    }

    let new_pml4 = paging::setup_page_tables(kernel.addr, kernel.allocation_size)?;
    serial::log_hex("new pml4=", new_pml4);

    memory::capture_memory_map(boot_info)?;
    serial::log_dec("mmap entries=", boot_info.mmap_count as u64);

    draw_white_square(boot_info);

    serial::log("rust bootloader: exiting boot services\n");
    let _ = unsafe { boot::exit_boot_services(MemoryType::LOADER_DATA) };
    serial::log("rust bootloader: boot services exited\n");

    paging::activate_page_tables(new_pml4);
    serial::log("rust bootloader: cr3 switched\n");

    let entry_addr = paging::KERNEL_ENTRY_VIRT_ADDR;
    serial::log_hex("kernel entry=", entry_addr);
    let entry: KernelEntry = unsafe { core::mem::transmute(entry_addr as usize) };
    unsafe { entry(boot_info as *const CoreOsBootInfo) }
}

fn draw_white_square(boot_info: &CoreOsBootInfo) {
    let fb = boot_info.fb_base as *mut u32;
    for i in 0..(500 * 500) {
        unsafe { fb.add(i).write_volatile(0xFFFF_FFFF) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    serial::log("rust bootloader panic\n");
    loop {
        core::hint::spin_loop();
    }
}
