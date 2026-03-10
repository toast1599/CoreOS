/// GDT + TSS
///
/// Layout (matches x86_64 System V ABI expectations):
///   Index 0  offset 0x00  null descriptor
///   Index 1  offset 0x08  kernel code  (ring 0, 64-bit)
///   Index 2  offset 0x10  kernel data  (ring 0)
///   Index 3  offset 0x18  user data    (ring 3)
///   Index 4  offset 0x20  user code    (ring 3, 64-bit)
///   Index 5  offset 0x28  TSS low      (16-byte system descriptor)
///   Index 6  offset 0x30  TSS high
///
/// Segment selectors used elsewhere:
///   KCODE = 0x08
///   KDATA = 0x10
///   UDATA = 0x18 | 3  = 0x1B
///   UCODE = 0x20 | 3  = 0x23
///   TSS   = 0x28
use core::mem::size_of;

// ---------------------------------------------------------------------------
// Selectors (exported so idt.rs / syscall stubs can use them)
// ---------------------------------------------------------------------------
pub const SEG_KCODE: u16 = 0x08;
pub const SEG_KDATA: u16 = 0x10;
pub const SEG_UDATA: u16 = 0x18 | 3;
pub const SEG_UCODE: u16 = 0x20 | 3;
pub const SEG_TSS: u16 = 0x28;

// ---------------------------------------------------------------------------
// Raw descriptor types
// ---------------------------------------------------------------------------

/// A normal 8-byte GDT entry.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_mid: u8,
    access: u8,
    granul: u8, // [limit_high(4) | flags(4)]
    base_high: u8,
}

impl GdtEntry {
    const fn null() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_mid: 0,
            access: 0,
            granul: 0,
            base_high: 0,
        }
    }

    /// Build a 64-bit code or data descriptor.
    /// `access` byte layout:
    ///   7   = present
    ///   6:5 = DPL
    ///   4   = descriptor type (1 = code/data)
    ///   3   = executable
    ///   1   = readable/writable
    const fn new(access: u8, flags: u8) -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access,
            // upper nibble = flags (L=1 for 64-bit code, G=1), lower = limit high 0xF
            granul: (flags << 4) | 0x0F,
            base_high: 0,
        }
    }
}

/// The 16-byte TSS system descriptor (two consecutive GDT slots).
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TssDescriptor {
    limit_low: u16,
    base_0_15: u16,
    base_16_23: u8,
    access: u8,      // 0x89 = present, 64-bit TSS available
    limit_flags: u8, // limit[19:16] | flags
    base_24_31: u8,
    base_32_63: u32,
    reserved: u32,
}

impl TssDescriptor {
    fn new(base: u64, limit: u32) -> Self {
        Self {
            limit_low: (limit & 0xFFFF) as u16,
            base_0_15: (base & 0xFFFF) as u16,
            base_16_23: ((base >> 16) & 0xFF) as u8,
            access: 0x89, // present | type=TSS-available
            limit_flags: ((limit >> 16) & 0xF) as u8,
            base_24_31: ((base >> 24) & 0xFF) as u8,
            base_32_63: (base >> 32) as u32,
            reserved: 0,
        }
    }
}

/// 64-bit Task State Segment (only rsp0 matters for us right now).
#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    pub rsp0: u64, // kernel stack pointer used on ring-3 → ring-0 transition
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iopb: u16,
}

impl Tss {
    const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iopb: size_of::<Tss>() as u16,
        }
    }
}

// ---------------------------------------------------------------------------
// Static GDT  (7 slots: 5 × 8-byte + 1 × 16-byte TSS descriptor)
// ---------------------------------------------------------------------------

/// We store the GDT as raw u64 words so the TSS descriptor (16 bytes = 2 slots)
/// fits naturally.
#[repr(C, align(16))]
struct RawGdt([u64; 7]);

static mut GDT: RawGdt = RawGdt([0u64; 7]);
pub static mut TSS: Tss = Tss::new();

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub unsafe fn init() {
    // Helper: encode a GdtEntry as u64
    fn encode(e: GdtEntry) -> u64 {
        let bytes: [u8; 8] = unsafe { core::mem::transmute(e) };
        u64::from_le_bytes(bytes)
    }

    // access byte constants
    const PRESENT: u8 = 0x80;
    const DPL0: u8 = 0x00;
    const DPL3: u8 = 0x60;
    const DTYPE: u8 = 0x10; // code/data (not system)
    const EXEC: u8 = 0x08;
    const RW: u8 = 0x02;

    // flag nibble constants (written into upper nibble of granul byte)
    const LONG: u8 = 0x2; // L bit — 64-bit code segment
    const DB: u8 = 0x4; // D/B bit — 32-bit data
    const GRAN: u8 = 0x8; // G bit — 4 KB granularity

    GDT.0[0] = 0; // null

    // kernel code: present, DPL0, code/data type, executable, readable, 64-bit, 4K gran
    GDT.0[1] = encode(GdtEntry::new(
        PRESENT | DPL0 | DTYPE | EXEC | RW,
        LONG | GRAN,
    ));

    // kernel data: present, DPL0, code/data type, writable, 32-bit D/B, 4K gran
    GDT.0[2] = encode(GdtEntry::new(PRESENT | DPL0 | DTYPE | RW, DB | GRAN));

    // user data: same as kernel data but DPL3
    GDT.0[3] = encode(GdtEntry::new(PRESENT | DPL3 | DTYPE | RW, DB | GRAN));

    // user code: same as kernel code but DPL3
    GDT.0[4] = encode(GdtEntry::new(
        PRESENT | DPL3 | DTYPE | EXEC | RW,
        LONG | GRAN,
    ));

    // TSS descriptor (16 bytes across slots 5 and 6)
    let tss_base = &raw const TSS as u64;
    let tss_limit = (size_of::<Tss>() - 1) as u32;
    let tss_desc = TssDescriptor::new(tss_base, tss_limit);
    let tss_bytes: [u8; 16] = core::mem::transmute(tss_desc);
    GDT.0[5] = u64::from_le_bytes(tss_bytes[0..8].try_into().unwrap());
    GDT.0[6] = u64::from_le_bytes(tss_bytes[8..16].try_into().unwrap());

    // Load the GDT
    let gdtp = GdtPointer {
        limit: (size_of::<RawGdt>() - 1) as u16,
        base: &raw const GDT as u64,
    };

    core::arch::asm!(
        // lgdt
        "lgdt [{gdtp}]",
        // Reload CS via a far return
        "push {kcode}",
        "lea  rax, [rip + 2f]",
        "push rax",
        "retfq",
        "2:",
        // Reload data segments
        "mov ax, {kdata}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        gdtp  = in(reg) &gdtp,
        kcode = const SEG_KCODE as u64,
        kdata = const SEG_KDATA as u16,
        options(nostack),
    );

    // Load TSS
    core::arch::asm!(
        "ltr ax",
        in("ax") SEG_TSS,
        options(nostack, nomem)
    );

    crate::dbg_log!("GDT", "loaded (TSS base={:#x})", tss_base);
}
