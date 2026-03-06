#include <efi.h>
#include <stdint.h>
#include <efilib.h>

#pragma pack(push, 1)
typedef struct {
    uint64_t fb_base;
    uint64_t fb_size;
    uint32_t width;
    uint32_t height;
    uint32_t pitch;
} CoreOS_BootInfo;
#pragma pack(pop)

static CoreOS_BootInfo bInfo;
typedef void (*KernelEntry)(CoreOS_BootInfo*);

EFI_STATUS EFIAPI efi_main(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable) {
    EFI_STATUS status;
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop;
    
    // Define the GUIDs as variables so we can take their addresses
    EFI_GUID gopGuid = EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID;
    EFI_GUID liGuid = EFI_LOADED_IMAGE_PROTOCOL_GUID;
    EFI_GUID sfspGuid = EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID;

    SystemTable->BootServices->SetWatchdogTimer(0, 0, 0, NULL);
    SystemTable->BootServices->LocateProtocol(&gopGuid, NULL, (void**)&gop);

    // 1. Load Kernel
    EFI_LOADED_IMAGE_PROTOCOL *loaded_image;
    SystemTable->BootServices->HandleProtocol(ImageHandle, &liGuid, (void**)&loaded_image);
    
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *fs;
    SystemTable->BootServices->HandleProtocol(loaded_image->DeviceHandle, &sfspGuid, (void**)&fs);
    
EFI_FILE_PROTOCOL *root, *kernel_file;
fs->OpenVolume(fs, &root);
status = root->Open(root, &kernel_file, L"kernel.bin", EFI_FILE_MODE_READ, 0);

/* -------------------------------
   Get kernel file size
--------------------------------*/

EFI_GUID fileInfoGuid = EFI_FILE_INFO_ID;

UINTN infoSize = 0;
kernel_file->GetInfo(kernel_file, &fileInfoGuid, &infoSize, NULL);

EFI_FILE_INFO *fileInfo;
SystemTable->BootServices->AllocatePool(EfiLoaderData, infoSize, (void**)&fileInfo);

kernel_file->GetInfo(kernel_file, &fileInfoGuid, &infoSize, fileInfo);

UINTN fileSize = fileInfo->FileSize;

/* -------------------------------
   Allocate memory for kernel
--------------------------------*/

UINTN pages = (fileSize + 0xFFF) / 0x1000;

EFI_PHYSICAL_ADDRESS kernel_addr = 0x100000;

SystemTable->BootServices->AllocatePages(
    AllocateAddress,
    EfiLoaderData,
    pages,
    &kernel_addr
);

/* -------------------------------
   Read kernel into memory
--------------------------------*/

kernel_file->Read(kernel_file, &fileSize, (void*)kernel_addr);

    // 2. Capture GOP data
    bInfo.fb_base = (uint64_t)gop->Mode->FrameBufferBase;
    bInfo.fb_size = (uint64_t)gop->Mode->FrameBufferSize;
    bInfo.width   = gop->Mode->Info->HorizontalResolution;
    bInfo.height  = gop->Mode->Info->VerticalResolution;
    bInfo.pitch   = gop->Mode->Info->PixelsPerScanLine;

    // 3. Exit Boot Services
    UINTN MapSize = 0, MapKey, DescSize;
    uint32_t DescVer;
    SystemTable->BootServices->GetMemoryMap(&MapSize, NULL, &MapKey, &DescSize, &DescVer);
    status = SystemTable->BootServices->ExitBootServices(ImageHandle, MapKey);
    if (EFI_ERROR(status)) {
        SystemTable->BootServices->GetMemoryMap(&MapSize, NULL, &MapKey, &DescSize, &DescVer);
        SystemTable->BootServices->ExitBootServices(ImageHandle, MapKey);
    }

    // 4. Debug: White Square in C
    uint32_t *fb = (uint32_t*)bInfo.fb_base;
    for(int i = 0; i < 500*500; i++) fb[i] = 0xFFFFFFFF;

    // 5. Jump to Rust
    KernelEntry kStart = (KernelEntry)kernel_addr;
    kStart(&bInfo);
    
    while(1);
    return EFI_SUCCESS;
}
