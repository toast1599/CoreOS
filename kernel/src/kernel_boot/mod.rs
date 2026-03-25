mod assets;
mod runtime;

use crate::{arch, bench, boot, drivers, mem, proc};

pub use assets::{boot_info_high_half, cache_boot_assets, install_ramfs_payloads};
pub use runtime::{heap_smoke_test, init_runtime, run_embedded_userspace_test, zero_bss};

pub unsafe fn boot_kernel(
    boot_info_phys: *const boot::CoreOS_BootInfo,
    stack_top: usize,
    background_task: fn(),
) -> *const boot::CoreOS_BootInfo {
    let boot_info = boot_info_high_half(boot_info_phys);

    bench::stamp(bench::Phase::KernelEntry);
    let bl_tsc = core::ptr::read_unaligned(core::ptr::addr_of!((*boot_info).tsc_bootloader_start));
    bench::set_bootloader_tsc(bl_tsc);

    let kernel_end = stack_top - 0xFFFFFFFF80000000 + 0x200000;
    drivers::serial::write_str("calling pmm::init\n");
    mem::pmm::init(boot_info, kernel_end);
    drivers::serial::write_str("pmm ok\n");
    bench::stamp(bench::Phase::PmmDone);

    let assets = cache_boot_assets(boot_info);

    arch::paging::init(boot_info);
    drivers::serial::write_str("paging ok\n");
    bench::stamp(bench::Phase::PagingDone);

    install_ramfs_payloads(&assets);
    init_runtime(boot_info, stack_top);
    heap_smoke_test();

    proc::task::add_task(background_task);
    drivers::serial::write_str("task added\n");
    bench::stamp(bench::Phase::RamfsDone);

    run_embedded_userspace_test(&[
        'p', 'o', 's', 'i', 'x', '_', 'n', 'e', 'w', 's', 'y', 's', '_',
        't', 'e', 's', 't',
    ]);

    boot_info
}
