#![no_main]
#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, write_bytes};

use uefi::boot::{self, AllocateType, MemoryType};
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::{
    Directory, File, FileAttribute, FileInfo, FileMode, FileType, RegularFile,
};
use uefi::{cstr16, Error, Status};

const MAX_MMAP_ENTRIES: usize = 256;
const KERNEL_LOAD_ADDR: u64 = 0x0010_0000;
const USER_ELF_LOAD_ADDR: u64 = 0x0020_0000;
const FONT_LOAD_ADDR: u64 = 0x0200_0000;
const KERNEL_EXTRA_BYTES: usize = 128 * 1024 * 1024;
const HIGH_HALF_PT_PAGES: usize = 8;
const KERNEL_VIRT_BASE: u64 = 0xFFFF_FFFF_8000_0000;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct CoreOsMemMapEntry {
    physical_start: u64,
    num_pages: u64,
    mem_type: u32,
    _pad: u32,
}

#[repr(C, packed)]
struct CoreOsBootInfo {
    fb_base: u64,
    fb_size: u64,
    width: u32,
    height: u32,
    pitch: u32,
    mmap: [CoreOsMemMapEntry; MAX_MMAP_ENTRIES],
    mmap_count: u32,
    _pad: u32,
    user_elf_base: u64,
    user_elf_size: u64,
    font_base: u64,
    font_size: u64,
    tsc_bootloader_start: u64,
}

impl CoreOsBootInfo {
    const fn zeroed() -> Self {
        Self {
            fb_base: 0,
            fb_size: 0,
            width: 0,
            height: 0,
            pitch: 0,
            mmap: [CoreOsMemMapEntry {
                physical_start: 0,
                num_pages: 0,
                mem_type: 0,
                _pad: 0,
            }; MAX_MMAP_ENTRIES],
            mmap_count: 0,
            _pad: 0,
            user_elf_base: 0,
            user_elf_size: 0,
            font_base: 0,
            font_size: 0,
            tsc_bootloader_start: 0,
        }
    }
}

#[repr(C)]
struct PageTableLayout {
    pml4: [u64; 512],
    dm_pdpt: [u64; 512],
    dm_pd0: [u64; 512],
    dm_pd1: [u64; 512],
    dm_pd2: [u64; 512],
    dm_pd3: [u64; 512],
    k_pdpt: [u64; 512],
    k_pd0: [u64; 512],
}

type KernelEntry = unsafe extern "win64" fn(*const CoreOsBootInfo) -> !;

static mut BOOT_INFO: CoreOsBootInfo = CoreOsBootInfo::zeroed();
static mut FILE_INFO_BUF: [u8; 512] = [0; 512];

#[entry]
fn efi_main(image_handle: Handle, _system_table: SystemTable<Boot>) -> Status {
    serial::init();
    serial::log("rust bootloader: entry\n");

    let boot_info = unsafe { &mut *addr_of_mut!(BOOT_INFO) };
    boot_info.tsc_bootloader_start = rdtsc();
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

    let (kernel_addr, kernel_size) = load_file_to_address(
        &mut root,
        cstr16!("\\kernel.bin"),
        KERNEL_LOAD_ADDR,
        KERNEL_EXTRA_BYTES,
    )?;
    serial::log_hex("kernel load addr=", kernel_addr);
    serial::log_hex("kernel file size=", kernel_size as u64);

    let mode = gop.current_mode_info();
    let mut frame_buffer = gop.frame_buffer();
    boot_info.fb_base = frame_buffer.as_mut_ptr() as u64;
    boot_info.fb_size = frame_buffer.size() as u64;
    boot_info.width = mode.resolution().0 as u32;
    boot_info.height = mode.resolution().1 as u32;
    boot_info.pitch = mode.stride() as u32;
    serial::log("rust bootloader: framebuffer captured\n");

    capture_memory_map(boot_info)?;
    serial::log_dec("mmap entries=", boot_info.mmap_count as u64);

    match load_optional_file_to_address(&mut root, cstr16!("\\test.elf"), USER_ELF_LOAD_ADDR)? {
        Some((elf_addr, elf_size)) => {
            boot_info.user_elf_base = elf_addr;
            boot_info.user_elf_size = elf_size as u64;
            serial::log_hex("user elf addr=", elf_addr);
            serial::log_hex("user elf size=", elf_size as u64);
        }
        None => serial::log("rust bootloader: test.elf missing\n"),
    }

    match load_optional_file_to_address(&mut root, cstr16!("\\font.psfu"), FONT_LOAD_ADDR)? {
        Some((font_addr, font_size)) => {
            boot_info.font_base = font_addr;
            boot_info.font_size = font_size as u64;
            serial::log_hex("font addr=", font_addr);
            serial::log_hex("font size=", font_size as u64);
        }
        None => serial::log("rust bootloader: font.psfu missing\n"),
    }

    let new_pml4 = setup_page_tables()?;
    serial::log_hex("new pml4=", new_pml4);

    draw_white_square(boot_info);

    serial::log("rust bootloader: exiting boot services\n");
    let _ = unsafe { boot::exit_boot_services(MemoryType::LOADER_DATA) };
    serial::log("rust bootloader: boot services exited\n");

    activate_page_tables(new_pml4);
    serial::log("rust bootloader: cr3 switched\n");

    let entry_addr = kernel_addr.wrapping_add(KERNEL_VIRT_BASE);
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

fn load_optional_file_to_address(
    root: &mut Directory,
    path: &uefi::CStr16,
    load_addr: u64,
) -> uefi::Result<Option<(u64, usize)>> {
    match open_regular_file(root, path) {
        Ok(file) => load_regular_file(file, load_addr, 0).map(Some),
        Err(err) if err.status() == Status::NOT_FOUND => Ok(None),
        Err(err) => Err(err),
    }
}

fn load_file_to_address(
    root: &mut Directory,
    path: &uefi::CStr16,
    load_addr: u64,
    extra_bytes: usize,
) -> uefi::Result<(u64, usize)> {
    let file = open_regular_file(root, path)?;
    load_regular_file(file, load_addr, extra_bytes)
}

fn open_regular_file(root: &mut Directory, path: &uefi::CStr16) -> uefi::Result<RegularFile> {
    serial::log("open file\n");
    let handle = root.open(path, FileMode::Read, FileAttribute::empty())?;
    serial::log("open file ok\n");
    match handle.into_type()? {
        FileType::Regular(file) => Ok(file),
        FileType::Dir(_) => Err(Error::new(Status::UNSUPPORTED, ())),
    }
}

fn load_regular_file(
    mut file: RegularFile,
    load_addr: u64,
    extra_bytes: usize,
) -> uefi::Result<(u64, usize)> {
    serial::log("query file size\n");
    let file_size = file_size(&mut file)?;
    serial::log_hex("queried file size=", file_size as u64);
    let total_size = file_size
        .checked_add(extra_bytes)
        .ok_or_else(|| Error::new(Status::BAD_BUFFER_SIZE, ()))?;
    let alloc_size = match allocate_fixed(load_addr, total_size) {
        Ok(()) => total_size,
        Err(err) if err.status() == Status::NOT_FOUND && extra_bytes != 0 => {
            serial::log("fixed allocation with overhead failed, retrying exact file size\n");
            allocate_fixed(load_addr, file_size)?;
            file_size
        }
        Err(err) => return Err(err),
    };
    serial::log_hex("alloc target=", load_addr);
    serial::log_hex("alloc total bytes=", alloc_size as u64);
    serial::log("allocate pages ok\n");

    let buffer = unsafe { core::slice::from_raw_parts_mut(load_addr as *mut u8, file_size) };
    let read = file.read(buffer)?;
    serial::log_hex("bytes read=", read as u64);
    if read != file_size {
        return Err(Error::new(Status::LOAD_ERROR, ()));
    }
    Ok((load_addr, file_size))
}

fn allocate_fixed(load_addr: u64, size: usize) -> uefi::Result<()> {
    boot::allocate_pages(
        AllocateType::Address(load_addr),
        MemoryType::LOADER_DATA,
        pages_for(size),
    )?;
    Ok(())
}

fn file_size(file: &mut RegularFile) -> uefi::Result<usize> {
    let buf = unsafe { &mut *addr_of_mut!(FILE_INFO_BUF) };
    let info = file
        .get_info::<FileInfo>(buf)
        .map_err(|err| err.to_err_without_payload())?;
    Ok(info.file_size() as usize)
}

fn capture_memory_map(boot_info: &mut CoreOsBootInfo) -> uefi::Result<()> {
    let map = boot::memory_map(MemoryType::LOADER_DATA)?;

    let mut stored = 0usize;
    for desc in map.entries() {
        if stored >= MAX_MMAP_ENTRIES {
            break;
        }
        boot_info.mmap[stored] = CoreOsMemMapEntry {
            physical_start: desc.phys_start,
            num_pages: desc.page_count,
            mem_type: desc.ty.0,
            _pad: 0,
        };
        stored += 1;
    }
    boot_info.mmap_count = stored as u32;
    Ok(())
}

fn pages_for(bytes: usize) -> usize {
    (bytes + 0xFFF) / 0x1000
}

fn setup_page_tables() -> uefi::Result<u64> {
    let base_ptr = boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        HIGH_HALF_PT_PAGES,
    )?;
    let layout_ptr = base_ptr.as_ptr() as *mut PageTableLayout;
    unsafe { write_bytes(layout_ptr.cast::<u8>(), 0, HIGH_HALF_PT_PAGES * 0x1000) };

    let layout = unsafe { &mut *layout_ptr };
    let base = base_ptr.as_ptr() as u64;
    let dm_pdpt = base + 0x1000;
    let dm_pd0 = base + 0x2000;
    let dm_pd1 = base + 0x3000;
    let dm_pd2 = base + 0x4000;
    let dm_pd3 = base + 0x5000;
    let k_pdpt = base + 0x6000;
    let k_pd0 = base + 0x7000;

    layout.pml4[256] = dm_pdpt | 0x3;
    layout.dm_pdpt[0] = dm_pd0 | 0x3;
    layout.dm_pdpt[1] = dm_pd1 | 0x3;
    layout.dm_pdpt[2] = dm_pd2 | 0x3;
    layout.dm_pdpt[3] = dm_pd3 | 0x3;

    for i in 0..4 {
        let table = match i {
            0 => &mut layout.dm_pd0,
            1 => &mut layout.dm_pd1,
            2 => &mut layout.dm_pd2,
            _ => &mut layout.dm_pd3,
        };
        for (j, entry) in table.iter_mut().enumerate() {
            *entry = (((i * 512 + j) as u64) * 0x20_0000) | 0x83;
        }
    }

    layout.pml4[511] = k_pdpt | 0x3;
    layout.k_pdpt[510] = k_pd0 | 0x3;
    for (j, entry) in layout.k_pd0.iter_mut().enumerate() {
        *entry = ((j as u64) * 0x20_0000) | 0x83;
    }

    Ok(base)
}

fn activate_page_tables(new_pml4: u64) {
    let old_pml4 = current_cr3() & !0xFFF;
    let old_entries = old_pml4 as *const u64;
    let new_entries = new_pml4 as *mut u64;

    for i in 0..512 {
        unsafe {
            if new_entries.add(i).read() == 0 {
                new_entries.add(i).write(old_entries.add(i).read());
            }
        }
    }

    unsafe {
        asm!("mov cr3, {}", in(reg) new_pml4, options(nostack, preserves_flags));
    }
}

fn current_cr3() -> u64 {
    let cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
    }
    cr3
}

fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nostack, preserves_flags));
    }
    ((hi as u64) << 32) | lo as u64
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    serial::log("rust bootloader panic\n");
    loop {
        core::hint::spin_loop();
    }
}

mod serial {
    use core::arch::asm;

    const COM1: u16 = 0x3F8;

    pub fn init() {
        unsafe {
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x80);
            outb(COM1 + 0, 0x03);
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x03);
            outb(COM1 + 2, 0xC7);
            outb(COM1 + 4, 0x0B);
        }
    }

    pub fn log(s: &str) {
        for byte in s.bytes() {
            write_byte(byte);
        }
    }

    pub fn log_hex(prefix: &str, value: u64) {
        log(prefix);
        log("0x");
        for shift in (0..16).rev() {
            let nibble = ((value >> (shift * 4)) & 0xF) as u8;
            write_byte(if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + (nibble - 10)
            });
        }
        log("\n");
    }

    pub fn log_dec(prefix: &str, mut value: u64) {
        log(prefix);
        if value == 0 {
            log("0\n");
            return;
        }

        let mut buf = [0u8; 20];
        let mut idx = buf.len();
        while value > 0 {
            idx -= 1;
            buf[idx] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        for byte in &buf[idx..] {
            write_byte(*byte);
        }
        log("\n");
    }

    pub fn log_status(prefix: &str, status: uefi::Status) {
        log_hex(prefix, status.0 as u64);
    }

    fn write_byte(byte: u8) {
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {}
            outb(COM1, byte);
        }
    }

    unsafe fn outb(port: u16, value: u8) {
        asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem, preserves_flags));
    }

    unsafe fn inb(port: u16) -> u8 {
        let value: u8;
        asm!("in al, dx", in("dx") port, out("al") value, options(nostack, nomem, preserves_flags));
        value
    }
}
