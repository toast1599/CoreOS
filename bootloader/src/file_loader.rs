use core::ptr::addr_of_mut;

use uefi::boot::{self, AllocateType, MemoryType};
use uefi::proto::media::file::{
    Directory, File, FileAttribute, FileInfo, FileMode, FileType, RegularFile,
};
use uefi::{CStr16, Error, Status};

use crate::serial;

const KERNEL_MAX_PHYS_ADDR: u64 = 0x3FFF_F000;
const ASSET_MAX_PHYS_ADDR: u64 = 0xFFFF_F000;

pub struct LoadSpec {
    max_address: u64,
    extra_bytes: usize,
}

impl LoadSpec {
    pub const fn kernel_with_extra(extra_bytes: usize) -> Self {
        Self {
            max_address: KERNEL_MAX_PHYS_ADDR,
            extra_bytes,
        }
    }

    pub const fn asset() -> Self {
        Self {
            max_address: ASSET_MAX_PHYS_ADDR,
            extra_bytes: 0,
        }
    }
}

pub struct LoadedFile {
    pub addr: u64,
    pub file_size: usize,
    pub allocation_size: usize,
}

static mut FILE_INFO_BUF: [u8; 512] = [0; 512];

pub fn load_optional_file(
    root: &mut Directory,
    path: &CStr16,
    spec: LoadSpec,
) -> uefi::Result<Option<LoadedFile>> {
    match open_regular_file(root, path) {
        Ok(file) => load_regular_file(file, spec).map(Some),
        Err(err) if err.status() == Status::NOT_FOUND => Ok(None),
        Err(err) => Err(err),
    }
}

pub fn load_required_file(
    root: &mut Directory,
    path: &CStr16,
    spec: LoadSpec,
) -> uefi::Result<LoadedFile> {
    let file = open_regular_file(root, path)?;
    load_regular_file(file, spec)
}

fn open_regular_file(root: &mut Directory, path: &CStr16) -> uefi::Result<RegularFile> {
    serial::log("open file\n");
    let handle = root.open(path, FileMode::Read, FileAttribute::empty())?;
    serial::log("open file ok\n");
    match handle.into_type()? {
        FileType::Regular(file) => Ok(file),
        FileType::Dir(_) => Err(Error::new(Status::UNSUPPORTED, ())),
    }
}

fn load_regular_file(mut file: RegularFile, spec: LoadSpec) -> uefi::Result<LoadedFile> {
    serial::log("query file size\n");
    let file_size = file_size(&mut file)?;
    serial::log_hex("queried file size=", file_size as u64);

    let requested_size = file_size
        .checked_add(spec.extra_bytes)
        .ok_or_else(|| Error::new(Status::BAD_BUFFER_SIZE, ()))?;

    let allocation = match allocate_max(requested_size, spec.max_address) {
        Ok(addr) => (addr, requested_size),
        Err(err) if err.status() == Status::OUT_OF_RESOURCES && spec.extra_bytes != 0 => {
            serial::log("allocation with overhead failed, retrying exact file size\n");
            let addr = allocate_max(file_size, spec.max_address)?;
            (addr, file_size)
        }
        Err(err) => return Err(err),
    };

    serial::log_hex("alloc target=", allocation.0);
    serial::log_hex("alloc total bytes=", allocation.1 as u64);
    serial::log("allocate pages ok\n");

    let buffer = unsafe { core::slice::from_raw_parts_mut(allocation.0 as *mut u8, file_size) };
    let read = file.read(buffer)?;
    serial::log_hex("bytes read=", read as u64);
    if read != file_size {
        return Err(Error::new(Status::LOAD_ERROR, ()));
    }

    Ok(LoadedFile {
        addr: allocation.0,
        file_size,
        allocation_size: allocation.1,
    })
}

fn file_size(file: &mut RegularFile) -> uefi::Result<usize> {
    let buf = unsafe { &mut *addr_of_mut!(FILE_INFO_BUF) };
    let info = file
        .get_info::<FileInfo>(buf)
        .map_err(|err| err.to_err_without_payload())?;
    Ok(info.file_size() as usize)
}

fn allocate_max(size: usize, max_address: u64) -> uefi::Result<u64> {
    let ptr = boot::allocate_pages(
        AllocateType::MaxAddress(max_address),
        MemoryType::LOADER_DATA,
        pages_for(size),
    )?;
    Ok(ptr.as_ptr() as u64)
}

fn pages_for(bytes: usize) -> usize {
    bytes.div_ceil(0x1000)
}
