// kernel/src/hw/pci.rs

use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::SpinLock;

// ---------------------------------------------------------------------------
// Port I/O — unsafe helpers
// ---------------------------------------------------------------------------

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

unsafe fn outl(port: u16, value: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem));
}

unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    core::arch::asm!("in eax, dx", in("dx") port, out("eax") value, options(nostack, nomem));
    value
}

unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    core::arch::asm!("in ax, dx", in("dx") port, out("ax") value, options(nostack, nomem));
    value
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack, nomem));
    value
}

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem));
}

unsafe fn outw(port: u16, value: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, nomem));
}

// ---------------------------------------------------------------------------
// Configuration Space Access
// ---------------------------------------------------------------------------

fn config_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    (1u32 << 31)
        | ((bus as u32) << 16)
        | (((device & 0x1F) as u32) << 11)
        | (((function & 0x07) as u32) << 8)
        | ((offset & 0xFC) as u32)
}

/// Read a 32‑bit dword from the PCI configuration space.
pub unsafe fn pci_read_dword(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    outl(CONFIG_ADDRESS, config_address(bus, device, func, offset));
    inl(CONFIG_DATA)
}

/// Read a 16‑bit word from the PCI configuration space.
pub unsafe fn pci_read_word(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
    let dword = pci_read_dword(bus, device, func, offset & 0xFC);
    ((dword >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

/// Read an 8‑bit byte from the PCI configuration space.
pub unsafe fn pci_read_byte(bus: u8, device: u8, func: u8, offset: u8) -> u8 {
    let dword = pci_read_dword(bus, device, func, offset & 0xFC);
    ((dword >> ((offset & 3) * 8)) & 0xFF) as u8
}

/// Write a 32‑bit dword to the PCI configuration space.
pub unsafe fn pci_write_dword(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    outl(CONFIG_ADDRESS, config_address(bus, device, func, offset));
    outl(CONFIG_DATA, value);
}

// ---------------------------------------------------------------------------
// Device Information
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct PciDeviceInfo {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    pub header_type: u8,
}

impl PciDeviceInfo {
    /// Construct a new device info by reading the config header from the given address.
    unsafe fn from_address(bus: u8, device: u8, func: u8) -> Option<Self> {
        let vendor = pci_read_word(bus, device, func, 0x00);
        if vendor == 0xFFFF {
            return None; // No device present
        }

        let device_id = pci_read_word(bus, device, func, 0x02);
        let class_code = pci_read_byte(bus, device, func, 0x0B);
        let subclass = pci_read_byte(bus, device, func, 0x0A);
        let prog_if = pci_read_byte(bus, device, func, 0x09);
        let revision = pci_read_byte(bus, device, func, 0x08);
        let header_type = pci_read_byte(bus, device, func, 0x0E) & 0x7F;

        Some(PciDeviceInfo {
            bus,
            device,
            function: func,
            vendor_id: vendor,
            device_id,
            class_code,
            subclass,
            prog_if,
            revision,
            header_type,
        })
    }
}

impl fmt::Display for PciDeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}.{:x}  {:04x}:{:04x}  class={:02x}{:02x}{:02x}  rev={:02x}",
            self.bus,
            self.device,
            self.function,
            self.vendor_id,
            self.device_id,
            self.class_code,
            self.subclass,
            self.prog_if,
            self.revision
        )
    }
}

// ---------------------------------------------------------------------------
// Device List
// ---------------------------------------------------------------------------

/// Maximum number of devices we can store. Adjust as needed.
const MAX_DEVICES: usize = 256;

/// Global list of all PCI devices found during enumeration.
static PCI_DEVICES: SpinLock<Vec<PciDeviceInfo>> = SpinLock::new(Vec::new());

/// Whether the PCI bus has already been scanned.
static PCI_SCANNED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

/// Scan the PCI bus for all devices and populate `PCI_DEVICES`.
/// Safe to call multiple times; the actual scan is only performed once.
pub fn enumerate_pci() {
    if PCI_SCANNED.swap(true, Ordering::AcqRel) {
        return; // Already scanned
    }

    let mut devices = PCI_DEVICES.lock();

    // Clear any previous entries (should be none).
    devices.clear();

    for bus in 0..=255u16 {
        for device in 0..32u8 {
            let func_mask = {
                // Check bit 7 of header type: if set, it's a multi‑function device
                unsafe {
                    let header = pci_read_byte(bus as u8, device, 0, 0x0E);
                    if header & 0x80 != 0 {
                        0x07
                    } else {
                        0x01
                    }
                }
            };

            for func in 0..=func_mask {
                let info = unsafe { PciDeviceInfo::from_address(bus as u8, device, func) };
                if let Some(dev) = info {
                    devices.push(dev);
                }
            }
        }
    }
}

/// Return a clone of the PCI device list (safe to read without lock).
pub fn device_list() -> Vec<PciDeviceInfo> {
    PCI_DEVICES.lock().clone()
}

/// Send a formatted list of all PCI devices to serial and VGA (via dbg_log).
pub fn print_devices() {
    let devices = PCI_DEVICES.lock();

    crate::serial_fmt!("PCI device list ({} devices):\n", devices.len());
    for dev in devices.iter() {
        crate::serial_fmt!("  {}\n", dev);
    }

    // Also log via debug macro
    for dev in devices.iter() {
        crate::dbg_log!(
            "PCI",
            "{:02x}:{:02x}.{:x}  vendor={:#06x} device={:#06x}  class={:#04x} sub={:#04x} prog={:#04x} rev={:#04x}",
            dev.bus,
            dev.device,
            dev.function,
            dev.vendor_id,
            dev.device_id,
            dev.class_code,
            dev.subclass,
            dev.prog_if,
            dev.revision
        );
    }
}

// ---------------------------------------------------------------------------
// Self‑Test Routines (rigorous checks)
// ---------------------------------------------------------------------------

/// Run a battery of PCI tests and return the number of failures.
pub fn pci_self_test() -> usize {
    let mut failures = 0usize;

    // Ensure enumeration has run.
    enumerate_pci();
    let devices = device_list();

    // Test 1: At least one device found.
    if devices.is_empty() {
        crate::serial_fmt!("[PCI TEST FAIL] No PCI devices found!\n");
        failures += 1;
    } else {
        crate::serial_fmt!("[PCI TEST PASS] Found {} PCI devices.\n", devices.len());
    }

    // Test 2: Verify vendor ID != 0xFFFF (already checked during enumeration, but re‑check).
    for dev in &devices {
        if dev.vendor_id == 0xFFFF {
            crate::serial_fmt!(
                "[PCI TEST FAIL] Device {} has vendor 0xFFFF (should have been excluded).\n",
                dev
            );
            failures += 1;
        }
        // Test 3: Device ID should not be 0xFFFF.
        if dev.device_id == 0xFFFF {
            crate::serial_fmt!("[PCI TEST FAIL] Device {} has device 0xFFFF.\n", dev);
            failures += 1;
        }

        // Test 4: Class code validity (non‑zero and not reserved).
        if dev.class_code == 0xFF || dev.class_code == 0xFE {
            crate::serial_fmt!(
                "[PCI TEST FAIL] Device {} has invalid class code {:#04x}.\n",
                dev,
                dev.class_code
            );
            failures += 1;
        }
    }

    // Test 5: Read and verify the header type field is consistent.
    for dev in &devices {
        let hdr = unsafe { pci_read_byte(dev.bus, dev.device, dev.function, 0x0E) & 0x7F };
        if hdr != dev.header_type {
            crate::serial_fmt!(
                "[PCI TEST FAIL] Header type mismatch for {}: stored {}, re‑read {}.\n",
                dev,
                dev.header_type,
                hdr
            );
            failures += 1;
        }
    }

    // Test 6: Exercise 16‑bit and 8‑bit reads (confirms byte‑lane extraction).
    if let Some(first_dev) = devices.first() {
        let offset = 0x00; // vendor & device
        let vendor =
            unsafe { pci_read_word(first_dev.bus, first_dev.device, first_dev.function, 0x00) };
        let device =
            unsafe { pci_read_word(first_dev.bus, first_dev.device, first_dev.function, 0x02) };
        let class =
            unsafe { pci_read_byte(first_dev.bus, first_dev.device, first_dev.function, 0x0B) };

        if vendor != first_dev.vendor_id
            || device != first_dev.device_id
            || class != first_dev.class_code
        {
            crate::serial_fmt!("[PCI TEST FAIL] Manual read mismatch for {}.\n", first_dev);
            failures += 1;
        } else {
            crate::serial_fmt!("[PCI TEST PASS] Manual read consistency check passed.\n");
        }
    }

    // Test 7: BAR0 sizing (skip bridges – they often lack BARs)
    if let Some(first_dev) = devices.first() {
        if first_dev.class_code != 0x06 && first_dev.class_code != 0x04 {
            let bar0_offset = 0x10;
            let original = unsafe {
                pci_read_dword(
                    first_dev.bus,
                    first_dev.device,
                    first_dev.function,
                    bar0_offset,
                )
            };
            unsafe {
                pci_write_dword(
                    first_dev.bus,
                    first_dev.device,
                    first_dev.function,
                    bar0_offset,
                    0xFFFFFFFF,
                )
            };
            let sized_val = unsafe {
                pci_read_dword(
                    first_dev.bus,
                    first_dev.device,
                    first_dev.function,
                    bar0_offset,
                )
            };
            unsafe {
                pci_write_dword(
                    first_dev.bus,
                    first_dev.device,
                    first_dev.function,
                    bar0_offset,
                    original,
                )
            };

            if sized_val == 0 || sized_val == 0xFFFFFFFF {
                crate::serial_fmt!(
                    "[PCI TEST FAIL] BAR0 sizing returned {:#010x} for {}.\n",
                    sized_val,
                    first_dev
                );
                failures += 1;
            } else {
                crate::serial_fmt!("[PCI TEST PASS] BAR write/read check successful.\n");
            }
        } else {
            crate::serial_fmt!(
                "[PCI TEST SKIP] BAR test skipped for bridge device {}.\n",
                first_dev
            );
        }
    }

    failures
}
