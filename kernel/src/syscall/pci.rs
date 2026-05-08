use crate::hw::pci;
use crate::syscall::helpers;
use crate::syscall::result::{self, SysError, SysResult};
use alloc::string::String;

pub unsafe fn pci_list(buf_ptr: u64, buf_len: u64) -> u64 {
    result::ret(pci_list_impl(buf_ptr, buf_len))
}

unsafe fn pci_list_impl(buf_ptr: u64, buf_len: u64) -> SysResult {
    pci::enumerate_pci();
    let devices = pci::device_list();

    let mut s = String::new();
    s.push_str("PCI devices:\n");
    if devices.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for dev in devices {
            s.push_str(&alloc::format!("  {}\n", dev));
        }
    }

    let bytes = s.as_bytes();
    let count = (buf_len as usize).min(bytes.len());
    result::ensure(
        crate::usercopy::copy_to_user(buf_ptr, &bytes[..count]).is_ok(),
        SysError::Fault,
    )?;
    result::ok(count as u64)
}

pub unsafe fn pci_test() -> u64 {
    let failures = pci::pci_self_test();
    (failures as u64).into()
}

