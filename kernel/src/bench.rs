/// bench.rs — Boot-time performance stamps.
///
/// Records TSC cycle counts at key points during kernel initialisation.
/// Results are displayed via the `boottime` shell command.
// ---------------------------------------------------------------------------
// TSC helper
// ---------------------------------------------------------------------------
extern crate alloc;

/// Read the CPU timestamp counter (cycles since reset).
#[inline]
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "lfence",   // serialise so TSC reflects all prior work
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nostack, nomem),
        );
    }
    ((hi as u64) << 32) | lo as u64
}

// ---------------------------------------------------------------------------
// Phase labels
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum Phase {
    KernelEntry,
    PmmDone,
    PagingDone,
    GdtDone,
    IdtDone,
    SyscallDone,
    PitDone,
    HeapDone,
    RamfsDone,
    ShellReady,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::KernelEntry => "kernel entry",
            Phase::PmmDone => "pmm init",
            Phase::PagingDone => "paging init",
            Phase::GdtDone => "gdt init",
            Phase::IdtDone => "idt init",
            Phase::SyscallDone => "syscall init",
            Phase::PitDone => "pit init",
            Phase::HeapDone => "heap smoke-test",
            Phase::RamfsDone => "ramfs + elf load",
            Phase::ShellReady => "shell ready",
        }
    }

    fn index(self) -> usize {
        match self {
            Phase::KernelEntry => 0,
            Phase::PmmDone => 1,
            Phase::PagingDone => 2,
            Phase::GdtDone => 3,
            Phase::IdtDone => 4,
            Phase::SyscallDone => 5,
            Phase::PitDone => 6,
            Phase::HeapDone => 7,
            Phase::RamfsDone => 8,
            Phase::ShellReady => 9,
        }
    }
}

const NUM_PHASES: usize = 10;

// ---------------------------------------------------------------------------
// Timestamp storage
// ---------------------------------------------------------------------------

/// TSC value recorded by the bootloader at `efi_main` entry.
/// Passed via `CoreOS_BootInfo.tsc_bootloader_start`.
static mut BOOTLOADER_TSC: u64 = 0;

/// Per-phase TSC stamps recorded during kernel init.
static mut STAMPS: [u64; NUM_PHASES] = [0u64; NUM_PHASES];

/// Record the current TSC for a given phase.
pub fn stamp(phase: Phase) {
    unsafe {
        STAMPS[phase.index()] = rdtsc();
    }
}

/// Store the bootloader TSC (read from BootInfo at kernel entry).
pub fn set_bootloader_tsc(tsc: u64) {
    unsafe {
        BOOTLOADER_TSC = tsc;
    }
}

// ---------------------------------------------------------------------------
// Report — called by `boottime` shell command
// ---------------------------------------------------------------------------

/// Returns a formatted boot timing report as an alloc::string::String.
pub fn report() -> alloc::string::String {
    use alloc::format;
    use alloc::string::String;

    let mut out = String::new();

    let bl = unsafe { BOOTLOADER_TSC };
    let stamps = unsafe { &STAMPS };

    // Header
    out.push_str("Boot timing (TSC cycles)\n");
    out.push_str("------------------------\n");

    // Bootloader → kernel entry
    let kernel_entry = stamps[Phase::KernelEntry.index()];
    if bl > 0 && kernel_entry > bl {
        out.push_str(&format!(
            "bootloader → kernel entry : {} cycles\n",
            kernel_entry - bl
        ));
    }

    // Each consecutive kernel phase
    for i in 0..NUM_PHASES {
        if stamps[i] == 0 {
            continue;
        }

        // Delta from previous recorded stamp
        let prev = if i == 0 {
            if bl > 0 {
                bl
            } else {
                stamps[i]
            }
        } else {
            // Find last non-zero stamp before this one
            let mut p = stamps[i];
            for j in (0..i).rev() {
                if stamps[j] > 0 {
                    p = stamps[j];
                    break;
                }
            }
            p
        };

        let delta = if stamps[i] >= prev {
            stamps[i] - prev
        } else {
            0
        };
        let label = match i {
            0 => Phase::KernelEntry.label(),
            1 => Phase::PmmDone.label(),
            2 => Phase::PagingDone.label(),
            3 => Phase::GdtDone.label(),
            4 => Phase::IdtDone.label(),
            5 => Phase::SyscallDone.label(),
            6 => Phase::PitDone.label(),
            7 => Phase::HeapDone.label(),
            8 => Phase::RamfsDone.label(),
            9 => Phase::ShellReady.label(),
            _ => "unknown",
        };

        out.push_str(&format!("{:<22}: {} cycles\n", label, delta));
    }

    // Total kernel init time
    let first = stamps[Phase::KernelEntry.index()];
    let last_idx = (0..NUM_PHASES).rev().find(|&i| stamps[i] > 0);
    if let Some(li) = last_idx {
        if stamps[li] > first && first > 0 {
            out.push_str("------------------------\n");
            out.push_str(&format!(
                "total kernel init         : {} cycles\n",
                stamps[li] - first
            ));
        }
    }

    out
}
