pub const USER_TOP: usize = 0x0000_8000_0000_0000;

#[inline]
pub fn user_range_ok(ptr: u64, len: usize) -> bool {
    let start = ptr as usize;
    start < USER_TOP && start.checked_add(len).is_some_and(|end| end <= USER_TOP)
}

pub unsafe fn copy_from_user(dst: &mut [u8], src: u64) -> Result<(), ()> {
    if !user_range_ok(src, dst.len()) {
        return Err(());
    }
    core::ptr::copy_nonoverlapping(src as *const u8, dst.as_mut_ptr(), dst.len());
    Ok(())
}

pub unsafe fn copy_to_user(dst: u64, src: &[u8]) -> Result<(), ()> {
    if !user_range_ok(dst, src.len()) {
        return Err(());
    }
    core::ptr::copy_nonoverlapping(src.as_ptr(), dst as *mut u8, src.len());
    Ok(())
}
