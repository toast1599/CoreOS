#[allow(dead_code)]

/// Minimal x86_64 ELF loader.
///
/// Parses a 64-bit ELF executable from a raw byte slice (e.g. from RamFS),
/// allocates physical pages via the PMM, copies PT_LOAD segments into place,
/// and returns the entry point address.
///
/// Does NOT set up page tables or drop to ring 3 — that is the caller's job.
use crate::mem::pmm::PAGE_SIZE;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    Not64Bit,
    NotLittleEndian,
    NotExecutable,
    NotX86_64,
    NoProgramHeaders,
    #[allow(dead_code)]
    OomLoadingSegment,
}

// ---------------------------------------------------------------------------
// ELF64 header (only the fields we need)
// ---------------------------------------------------------------------------

const ELFMAG: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1; // little-endian
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;

/// Reads a u16 from a byte slice at `offset` (little-endian).
#[inline]
fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}

/// Reads a u32 from a byte slice at `offset` (little-endian).
#[inline]
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

/// Reads a u64 from a byte slice at `offset` (little-endian).
#[inline]
fn read_u64(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}

// ---------------------------------------------------------------------------
// ELF64 header offsets
// ---------------------------------------------------------------------------
//
// Offset  Size  Field
//  0       4    e_ident[EI_MAG0..3]  magic
//  4       1    e_ident[EI_CLASS]    1=32bit 2=64bit
//  5       1    e_ident[EI_DATA]     1=LE 2=BE
//  6       1    e_ident[EI_VERSION]
//  16      2    e_type               ET_EXEC=2
//  18      2    e_machine            EM_X86_64=62
//  24      8    e_entry
//  32      8    e_phoff              program header table offset
//  48      2    e_phentsize          size of one program header
//  50      2    e_phnum              number of program headers

const OFF_CLASS: usize = 4;
const OFF_DATA: usize = 5;
const OFF_TYPE: usize = 16;
const OFF_MACHINE: usize = 18;
const OFF_ENTRY: usize = 24;
const OFF_PHOFF: usize = 32;
const OFF_PHENTSIZE: usize = 54;
const OFF_PHNUM: usize = 56;

// ---------------------------------------------------------------------------
// ELF64 program header offsets (within each phdr entry)
// ---------------------------------------------------------------------------
//
// Offset  Size  Field
//  0       4    p_type    PT_LOAD=1
//  4       4    p_flags
//  8       8    p_offset  offset in file
//  16      8    p_vaddr   virtual address to load at
//  24      8    p_paddr   (ignored)
//  32      8    p_filesz  bytes in file
//  40      8    p_memsz   bytes in memory (>= filesz; gap is zeroed BSS)
//  48      8    p_align

const PH_TYPE: usize = 0;
const PH_OFFSET: usize = 8;
const PH_VADDR: usize = 16;
const PH_FILESZ: usize = 32;
const PH_MEMSZ: usize = 40;

// ---------------------------------------------------------------------------
// Public loader entry point
// ---------------------------------------------------------------------------

/// Load an ELF64 executable from `data` into `pml4`.
///
/// Allocates pages via the PMM for every PT_LOAD segment, copies the file
/// bytes in, and zeroes the BSS gap (memsz > filesz).
///
/// Returns the entry point virtual address on success.
///
pub unsafe fn load_into(pml4: usize, data: &[u8]) -> Result<u64, ElfError> {
    // -----------------------------------------------------------------------
    // 1. Validate header
    // -----------------------------------------------------------------------
    if data.len() < 64 {
        return Err(ElfError::TooSmall);
    }
    crate::serial_fmt!(
        "elf::load magic: {:x} {:x} {:x} {:x}\n",
        data[0],
        data[1],
        data[2],
        data[3]
    );
    if data[0..4] != ELFMAG {
        return Err(ElfError::BadMagic);
    }
    if data[OFF_CLASS] != ELFCLASS64 {
        return Err(ElfError::Not64Bit);
    }
    if data[OFF_DATA] != ELFDATA2LSB {
        return Err(ElfError::NotLittleEndian);
    }
    if read_u16(data, OFF_TYPE) != ET_EXEC {
        return Err(ElfError::NotExecutable);
    }
    if read_u16(data, OFF_MACHINE) != EM_X86_64 {
        return Err(ElfError::NotX86_64);
    }

    let entry = read_u64(data, OFF_ENTRY);
    let phoff = read_u64(data, OFF_PHOFF) as usize;
    let phentsize = read_u16(data, OFF_PHENTSIZE) as usize;
    let phnum = read_u16(data, OFF_PHNUM) as usize;

    if phnum == 0 {
        return Err(ElfError::NoProgramHeaders);
    }

    crate::dbg_log!(
        "ELF",
        "entry={:#x} phoff={:#x} phentsize={} phnum={}",
        entry,
        phoff,
        phentsize,
        phnum
    );

    // -----------------------------------------------------------------------
    // 2. Walk PT_LOAD segments
    // -----------------------------------------------------------------------
    for i in 0..phnum {
        let ph_start = phoff + i * phentsize;

        if ph_start + phentsize > data.len() {
            return Err(ElfError::TooSmall);
        }

        let ph = &data[ph_start..ph_start + phentsize];

        let p_type = read_u32(ph, PH_TYPE);
        let p_offset = read_u64(ph, PH_OFFSET) as usize;
        let p_vaddr = read_u64(ph, PH_VADDR) as usize;
        let p_filesz = read_u64(ph, PH_FILESZ) as usize;
        let p_memsz = read_u64(ph, PH_MEMSZ) as usize;

        if p_type != PT_LOAD || p_memsz == 0 {
            continue;
        }
        if p_offset
            .checked_add(p_filesz)
            .is_none_or(|end| end > data.len())
        {
            return Err(ElfError::TooSmall);
        }

        crate::dbg_log!(
            "ELF",
            "PT_LOAD vaddr={:#x} filesz={:#x} memsz={:#x}",
            p_vaddr,
            p_filesz,
            p_memsz
        );

        // Map each segment page into the target address space and copy through
        // the kernel direct map so we do not depend on the current CR3.
        let page_start = p_vaddr & !(PAGE_SIZE - 1);
        let page_end = (p_vaddr + p_memsz + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let mut page = page_start;

        while page < page_end {
            if crate::arch::paging::translate_page(pml4, page).is_none() {
                let frame = crate::mem::pmm::alloc_frame();
                if frame == 0 {
                    return Err(ElfError::OomLoadingSegment);
                }
                crate::arch::paging::map_page_in(
                    pml4,
                    page,
                    frame,
                    crate::arch::paging::MapFlags {
                        writable: true,
                        user: true,
                        executable: true,
                    },
                );
                core::ptr::write_bytes(crate::arch::paging::p2v(frame) as *mut u8, 0, PAGE_SIZE);
            }
            page += PAGE_SIZE;
        }

        if p_filesz > 0 {
            let src = &data[p_offset..p_offset + p_filesz];
            let mut copied = 0usize;
            let mut cur_page = page_start;

            while copied < p_filesz {
                let page_off = if cur_page == page_start {
                    p_vaddr & (PAGE_SIZE - 1)
                } else {
                    0
                };
                let remaining = p_filesz - copied;
                let chunk = remaining.min(PAGE_SIZE - page_off);
                let frame = crate::arch::paging::translate_page(pml4, cur_page)
                    .ok_or(ElfError::OomLoadingSegment)?;
                let dst = (crate::arch::paging::p2v(frame) + page_off) as *mut u8;
                core::ptr::copy_nonoverlapping(src.as_ptr().add(copied), dst, chunk);
                copied += chunk;
                cur_page += PAGE_SIZE;
            }
        }

        let first4 = if p_filesz == 0 {
            &[][..]
        } else {
            let frame = crate::arch::paging::translate_page(pml4, page_start)
                .ok_or(ElfError::OomLoadingSegment)?;
            core::slice::from_raw_parts(
                (crate::arch::paging::p2v(frame) + (p_vaddr & (PAGE_SIZE - 1))) as *const u8,
                4.min(p_filesz),
            )
        };
        crate::dbg_log!(
            "ELF",
            "segment loaded at vaddr={:#x} first_bytes={:x?}",
            p_vaddr,
            first4
        );
    }

    Ok(entry)
}
