#[repr(C, packed)]
pub struct CoreOS_BootInfo {
    pub fb_base: u64,
    pub fb_size: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32, 
}

// Keep the font here as it is a boot-time resource
pub const FONT: &[u8] = include_bytes!("font.psfu");
