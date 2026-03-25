use crate::{arch, bench, boot, drivers, hw, proc, vfs};
use alloc::vec::Vec;

pub unsafe fn zero_bss() {
    extern "C" {
        static mut __bss_start: u8;
        static mut __bss_end: u8;
    }
    let bss_start = &raw mut __bss_start as *mut u8;
    let bss_end = &raw mut __bss_end as *mut u8;
    core::ptr::write_bytes(bss_start, 0, bss_end as usize - bss_start as usize);
}

pub unsafe fn init_runtime(boot_info: *const boot::CoreOS_BootInfo, stack_top: usize) {
    drivers::vga::init_global(boot_info);

    arch::gdt::init();
    drivers::serial::write_str("gdt ok\n");
    bench::stamp(bench::Phase::GdtDone);

    extern "C" {
        static __stack_bottom: u8;
    }
    arch::gdt::TSS.rsp0 = stack_top as u64;
    arch::gdt::TSS_RSP0 = stack_top as u64;

    arch::idt::init();
    drivers::serial::write_str("idt ok\n");
    bench::stamp(bench::Phase::IdtDone);

    crate::syscall::init();
    drivers::serial::write_str("syscall gate ok\n");
    bench::stamp(bench::Phase::SyscallDone);

    hw::pit::init();
    drivers::serial::write_str("pit ok\n");
    bench::stamp(bench::Phase::PitDone);

    proc::task::init_main_task(&raw const __stack_bottom as usize);

    core::arch::asm!("sti");
    drivers::serial::write_str("sti ok\n");
}

pub unsafe fn heap_smoke_test() {
    let mut v: Vec<u64> = Vec::new();
    v.push(42);
    v.push(1337);
    crate::serial_fmt!("heap smoke-test: {:?} {:?}\n", v[0], v[1]);
    bench::stamp(bench::Phase::HeapDone);
}

pub unsafe fn run_embedded_userspace_test(name: &[char]) {
    let elf_data = vfs::clone_bytes(name);

    if let Some(data) = elf_data {
        crate::serial_fmt!("Running embedded userspace test {:?}\n", name);
        let (pid, slot) = proc::exec::exec_as_task(&data, name);
        if pid == 0 {
            drivers::serial::write_str("Failed to spawn embedded userspace test\n");
            return;
        }
        while proc::is_running_in_slot(slot) {
            core::arch::asm!("hlt");
        }
        if let Some(code) = proc::reap_slot(slot) {
            crate::serial_fmt!(
                "Embedded userspace test pid {} exited with code {}\n",
                pid,
                code
            );
        }
    }
}
