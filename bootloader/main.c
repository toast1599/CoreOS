// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

#include <efi.h>
#include <stdint.h>
#include <efilib.h>

#define MAX_MMAP_ENTRIES 256

#pragma pack(push, 1)
typedef struct {
    uint64_t physical_start;
    uint64_t num_pages;
    uint32_t type;          // EFI_MEMORY_TYPE
    uint32_t _pad;
} CoreOS_MemMapEntry;

typedef struct {
    uint64_t fb_base;
    uint64_t fb_size;
    uint32_t width;
    uint32_t height;
    uint32_t pitch;

    // Memory map
    CoreOS_MemMapEntry mmap[MAX_MMAP_ENTRIES];
    uint32_t           mmap_count;
    uint32_t           _pad;

    // Initial userspace ELF loaded from ESP.
    // user_elf_base == 0 means no binary was found.
    uint64_t           user_elf_base;
    uint64_t           user_elf_size;

uint64_t           font_base;
    uint64_t           font_size;
    uint64_t           tsc_bootloader_start;
} CoreOS_BootInfo;
#pragma pack(pop)

static CoreOS_BootInfo bInfo;
typedef void (*KernelEntry)(CoreOS_BootInfo*);

EFI_STATUS EFIAPI efi_main(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable) {
    EFI_STATUS status;
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop;

    EFI_GUID gopGuid  = EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID;
    EFI_GUID liGuid   = EFI_LOADED_IMAGE_PROTOCOL_GUID;
    EFI_GUID sfspGuid = EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID;

// Record TSC at bootloader entry for boot timing.
    uint64_t tsc_lo, tsc_hi;
    __asm__ volatile("rdtsc" : "=a"(tsc_lo), "=d"(tsc_hi));
    bInfo.tsc_bootloader_start = (tsc_hi << 32) | tsc_lo;
    SystemTable->BootServices->LocateProtocol(&gopGuid, NULL, (void**)&gop);

    // -----------------------------------------------------------------------
    // 1. Load Kernel
    // -----------------------------------------------------------------------
    EFI_LOADED_IMAGE_PROTOCOL *loaded_image;
    SystemTable->BootServices->HandleProtocol(ImageHandle, &liGuid, (void**)&loaded_image);

    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *fs;
    SystemTable->BootServices->HandleProtocol(loaded_image->DeviceHandle, &sfspGuid, (void**)&fs);

    EFI_FILE_PROTOCOL *root, *kernel_file;
    fs->OpenVolume(fs, &root);
    status = root->Open(root, &kernel_file, L"kernel.bin", EFI_FILE_MODE_READ, 0);

    // Get kernel file size
    EFI_GUID fileInfoGuid = EFI_FILE_INFO_ID;
    UINTN infoSize = 0;
    kernel_file->GetInfo(kernel_file, &fileInfoGuid, &infoSize, NULL);

    EFI_FILE_INFO *fileInfo;
    SystemTable->BootServices->AllocatePool(EfiLoaderData, infoSize, (void**)&fileInfo);
    kernel_file->GetInfo(kernel_file, &fileInfoGuid, &infoSize, fileInfo);

    UINTN fileSize = fileInfo->FileSize;

    // Allocate memory for kernel + BSS/heap overhead (128MB)
    UINTN memSize = fileSize + (128ULL * 1024 * 1024);
    UINTN pages   = (memSize + 0xFFF) / 0x1000;

    EFI_PHYSICAL_ADDRESS kernel_addr = 0x100000;
    SystemTable->BootServices->AllocatePages(AllocateAddress, EfiLoaderData, pages, &kernel_addr);

    kernel_file->Read(kernel_file, &fileSize, (void*)kernel_addr);

    // -----------------------------------------------------------------------
    // 2. Capture GOP data
    // -----------------------------------------------------------------------
    bInfo.fb_base = (uint64_t)gop->Mode->FrameBufferBase;
    bInfo.fb_size = (uint64_t)gop->Mode->FrameBufferSize;
    bInfo.width   = gop->Mode->Info->HorizontalResolution;
    bInfo.height  = gop->Mode->Info->VerticalResolution;
    bInfo.pitch   = gop->Mode->Info->PixelsPerScanLine;

    // -----------------------------------------------------------------------
    // 3. Grab memory map BEFORE ExitBootServices
    //    We need two calls: first to get the size, second to get the data.
    // -----------------------------------------------------------------------
    UINTN mapSize   = 0;
    UINTN mapKey    = 0;
    UINTN descSize  = 0;
    uint32_t descVer = 0;
    EFI_MEMORY_DESCRIPTOR *memMap = NULL;

    // First call: get required buffer size
    SystemTable->BootServices->GetMemoryMap(&mapSize, NULL, &mapKey, &descSize, &descVer);
    mapSize += 2 * descSize; // a little extra in case it grows

    SystemTable->BootServices->AllocatePool(EfiLoaderData, mapSize, (void**)&memMap);
    SystemTable->BootServices->GetMemoryMap(&mapSize, memMap, &mapKey, &descSize, &descVer);

    // Copy into BootInfo (compact form, up to MAX_MMAP_ENTRIES)
    UINTN entryCount = mapSize / descSize;
    uint32_t stored  = 0;

    for (UINTN i = 0; i < entryCount && stored < MAX_MMAP_ENTRIES; i++) {
        EFI_MEMORY_DESCRIPTOR *desc =
            (EFI_MEMORY_DESCRIPTOR *)((uint8_t *)memMap + i * descSize);

        bInfo.mmap[stored].physical_start = desc->PhysicalStart;
        bInfo.mmap[stored].num_pages      = desc->NumberOfPages;
        bInfo.mmap[stored].type           = (uint32_t)desc->Type;
        bInfo.mmap[stored]._pad           = 0;
        stored++;
    }
    bInfo.mmap_count = stored;

	// -----------------------------------------------------------------------
	    // 3b. Load user ELF (test.elf) from ESP if present
	    // -----------------------------------------------------------------------
	    bInfo.user_elf_base = 0;
	    bInfo.user_elf_size = 0;
	
	    EFI_FILE_PROTOCOL *elf_file;
	    status = root->Open(root, &elf_file, L"test.elf", EFI_FILE_MODE_READ, 0);
	    if (!EFI_ERROR(status)) {
	        UINTN elfInfoSize = 0;
	        elf_file->GetInfo(elf_file, &fileInfoGuid, &elfInfoSize, NULL);
	
	        EFI_FILE_INFO *elfInfo;
	        SystemTable->BootServices->AllocatePool(EfiLoaderData, elfInfoSize, (void**)&elfInfo);
	        elf_file->GetInfo(elf_file, &fileInfoGuid, &elfInfoSize, elfInfo);
	
	        UINTN elfSize = elfInfo->FileSize;
	        UINTN elfPages = (elfSize + 0xFFF) / 0x1000;
	
	        EFI_PHYSICAL_ADDRESS elf_addr = 0x200000; // load at 2 MB
	        SystemTable->BootServices->AllocatePages(
	            AllocateAddress, EfiLoaderData, elfPages, &elf_addr);
	
	        elf_file->Read(elf_file, &elfSize, (void*)elf_addr);
	
	        bInfo.user_elf_base = (uint64_t)elf_addr;
	        bInfo.user_elf_size = (uint64_t)elfSize;
	    }
	    // If test.elf is missing, user_elf_base stays 0 and the kernel skips it.

// -----------------------------------------------------------------------
    // 3c. Load font.psfu from ESP
    // -----------------------------------------------------------------------
    bInfo.font_base = 0;
    bInfo.font_size = 0;

    EFI_FILE_PROTOCOL *font_file;
    status = root->Open(root, &font_file, L"font.psfu", EFI_FILE_MODE_READ, 0);
    if (!EFI_ERROR(status)) {
        UINTN fontInfoSize = 0;
        font_file->GetInfo(font_file, &fileInfoGuid, &fontInfoSize, NULL);

        EFI_FILE_INFO *fontInfo;
        SystemTable->BootServices->AllocatePool(EfiLoaderData, fontInfoSize, (void**)&fontInfo);
        font_file->GetInfo(font_file, &fileInfoGuid, &fontInfoSize, fontInfo);

        UINTN fontFileSize = fontInfo->FileSize;
        UINTN fontPages = (fontFileSize + 0xFFF) / 0x1000;

        EFI_PHYSICAL_ADDRESS font_addr = 0x2000000; // 32 MB — safely above kernel and ELF
        SystemTable->BootServices->AllocatePages(
            AllocateAddress, EfiLoaderData, fontPages, &font_addr);

        font_file->Read(font_file, &fontFileSize, (void*)font_addr);

        bInfo.font_base = (uint64_t)font_addr;
        bInfo.font_size = (uint64_t)fontFileSize;
    }

    // -----------------------------------------------------------------------
    // 4. Exit Boot Services
    // -----------------------------------------------------------------------
status = SystemTable->BootServices->ExitBootServices(ImageHandle, mapKey);
if (EFI_ERROR(status)) {
    // AllocatePool changes the map, so we must re-fetch size, re-allocate,
    // then re-fetch the full map (with a fresh mapKey) before retrying.
    mapSize = 0;
    SystemTable->BootServices->GetMemoryMap(&mapSize, NULL, &mapKey, &descSize, &descVer);
    mapSize += 2 * descSize;
    SystemTable->BootServices->AllocatePool(EfiLoaderData, mapSize, (void**)&memMap);
    SystemTable->BootServices->GetMemoryMap(&mapSize, memMap, &mapKey, &descSize, &descVer);
    SystemTable->BootServices->ExitBootServices(ImageHandle, mapKey);
}	

    // -----------------------------------------------------------------------
    // 5. Debug: White square drawn in C (sanity check before kernel runs)
    // -----------------------------------------------------------------------
    uint32_t *fb = (uint32_t*)bInfo.fb_base;
    for (int i = 0; i < 500 * 500; i++) fb[i] = 0xFFFFFFFF;

    // -----------------------------------------------------------------------
    // 6. Jump to Rust kernel
    // -----------------------------------------------------------------------
    KernelEntry kStart = (KernelEntry)kernel_addr;
    kStart(&bInfo);

    while (1);
    return EFI_SUCCESS;
}
