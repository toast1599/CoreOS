#pragma once
#include <stdint.h>

#define MAX_MMAP_ENTRIES 256

#pragma pack(push, 1)

typedef struct {
    uint64_t physical_start;
    uint64_t num_pages;
    uint32_t type;
    uint32_t _pad;
} CoreOS_MemMapEntry;

typedef struct {
    uint64_t fb_base;
    uint64_t fb_size;
    uint32_t width;
    uint32_t height;
    uint32_t pitch;

    CoreOS_MemMapEntry mmap[MAX_MMAP_ENTRIES];
    uint32_t mmap_count;
    uint32_t _pad;

    uint64_t user_elf_base;
    uint64_t user_elf_size;

    uint64_t font_base;
    uint64_t font_size;
    uint64_t tsc_bootloader_start;
} CoreOS_BootInfo;

#pragma pack(pop)
