use crate::{arch, boot, drivers, vfs};

static EMBEDDED_SHELL: &[u8] = include_bytes!("../../../user/shell.elf");
static EMBEDDED_SYSCALL_TEST: &[u8] = include_bytes!("../../../user/syscall_test.elf");
static EMBEDDED_SYSCALL_CHILD: &[u8] = include_bytes!("../../../user/syscall_child.elf");
static EMBEDDED_POSIX_NEWSYS_TEST: &[u8] = include_bytes!("../../../user/posix_newsys_test.elf");
static mut ELF_BUF: [u8; 64 * 1024] = [0u8; 64 * 1024];
static mut FONT_BUF: [u8; 16 * 1024] = [0u8; 16 * 1024];

pub struct BootAssets {
    pub elf_len: usize,
    pub font_len: usize,
}

fn preload_ramfs_file(name: &[char], bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    if vfs::find(name).is_none() && vfs::create(name) {
        let _ = vfs::append_all(name, bytes);
    }
}

pub unsafe fn boot_info_high_half(
    boot_info_phys: *const boot::CoreOS_BootInfo,
) -> *const boot::CoreOS_BootInfo {
    static mut BOOT_INFO_DATA: boot::CoreOS_BootInfo = boot::CoreOS_BootInfo {
        fb_base: 0,
        fb_size: 0,
        width: 0,
        height: 0,
        pitch: 0,
        mmap: [boot::MemMapEntry {
            physical_start: 0,
            num_pages: 0,
            mem_type: 0,
            _pad: 0,
        }; 256],
        mmap_count: 0,
        _pad: 0,
        kernel_phys_base: 0,
        kernel_alloc_size: 0,
        user_elf_base: 0,
        user_elf_size: 0,
        font_base: 0,
        font_size: 0,
        tsc_bootloader_start: 0,
    };

    BOOT_INFO_DATA = core::ptr::read_unaligned(boot_info_phys);
    BOOT_INFO_DATA.fb_base = arch::amd64::paging::p2v(BOOT_INFO_DATA.fb_base as usize) as u64;
    if BOOT_INFO_DATA.font_base != 0 {
        BOOT_INFO_DATA.font_base =
            arch::amd64::paging::p2v(BOOT_INFO_DATA.font_base as usize) as u64;
    }
    if BOOT_INFO_DATA.user_elf_base != 0 {
        BOOT_INFO_DATA.user_elf_base =
            arch::amd64::paging::p2v(BOOT_INFO_DATA.user_elf_base as usize) as u64;
    }
    core::ptr::addr_of!(BOOT_INFO_DATA)
}

pub unsafe fn cache_boot_assets(boot_info: *const boot::CoreOS_BootInfo) -> BootAssets {
    let mut assets = BootAssets {
        elf_len: 0,
        font_len: 0,
    };

    let font_base = (*boot_info).font_base;
    let font_size = (*boot_info).font_size as usize;
    if font_base != 0 && font_size > 0 && font_size <= FONT_BUF.len() {
        let src = core::slice::from_raw_parts(font_base as *const u8, font_size);
        FONT_BUF[..font_size].copy_from_slice(src);
        assets.font_len = font_size;
        drivers::serial::write_str("font loaded into static buffer\n");
    }

    let elf_base = (*boot_info).user_elf_base;
    let elf_size = (*boot_info).user_elf_size as usize;
    if elf_base != 0 && elf_size > 0 && elf_size <= ELF_BUF.len() {
        let src = core::slice::from_raw_parts(elf_base as *const u8, elf_size);
        ELF_BUF[..elf_size].copy_from_slice(src);
        assets.elf_len = elf_size;
        drivers::serial::write_str("elf bytes saved to static buffer\n");
    }

    if assets.font_len > 0 {
        (*boot_info.cast_mut()).font_base = FONT_BUF.as_ptr() as u64;
        (*boot_info.cast_mut()).font_size = assets.font_len as u64;
    }

    assets
}

pub unsafe fn install_ramfs_payloads(assets: &BootAssets) {
    vfs::init();

    preload_ramfs_file(&['t', 'e', 's', 't'], EMBEDDED_SHELL);
    preload_ramfs_file(
        &['s', 'y', 's', 'c', 'a', 'l', 'l', '_', 't', 'e', 's', 't'],
        EMBEDDED_SYSCALL_TEST,
    );
    preload_ramfs_file(
        &[
            's', 'y', 's', 'c', 'a', 'l', 'l', '_', 'c', 'h', 'i', 'l', 'd',
        ],
        EMBEDDED_SYSCALL_CHILD,
    );
    preload_ramfs_file(
        &[
            'p', 'o', 's', 'i', 'x', '_', 'n', 'e', 'w', 's', 'y', 's', '_',
            't', 'e', 's', 't',
        ],
        EMBEDDED_POSIX_NEWSYS_TEST,
    );

    if assets.elf_len > 0 {
        crate::serial_fmt!(
            "ELF_BUF addr={:#x} magic at copy time: {:x} {:x} {:x} {:x}\n",
            ELF_BUF.as_ptr() as usize,
            ELF_BUF[0],
            ELF_BUF[1],
            ELF_BUF[2],
            ELF_BUF[3]
        );
        let name: &[char] = &['b', 'o', 'o', 't', '_', 't', 'e', 's', 't'];
        if vfs::create(name) && vfs::append_all(name, &ELF_BUF[..assets.elf_len]) {
            if let Some(file) = vfs::find(name) {
                crate::serial_fmt!(
                    "user ELF loaded: {} bytes, magic: {:x} {:x} {:x} {:x}\n",
                    file.data.len(),
                    file.data[0],
                    file.data[1],
                    file.data[2],
                    file.data[3]
                );
            }
        }
    }
}
