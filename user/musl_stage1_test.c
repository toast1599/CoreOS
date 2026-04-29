#include "libcshim/libcshim.h"

typedef struct {
  uint64_t a_type;
  uint64_t a_val;
} AuxEntry;

#define AT_NULL 0
#define AT_PHDR 3
#define AT_PHENT 4
#define AT_PHNUM 5
#define AT_PAGESZ 6
#define AT_ENTRY 9

static void raw_puts(const char *s) {
  size_t len = 0;
  if (!s) return;
  while (s[len]) len++;
  syscall3(COREOS_SYS_WRITE, 1, (long)s, (long)len);
}

static void print_hex(uint64_t val) {
  char buf[19];
  buf[0] = '0'; buf[1] = 'x';
  for (int i = 0; i < 16; i++) {
    int nibble = (val >> (60 - i * 4)) & 0xF;
    buf[i + 2] = (nibble < 10) ? ('0' + nibble) : ('a' + nibble - 10);
  }
  buf[18] = '\0';
  raw_puts(buf);
}

static void print_int(long n) {
  if (n == 0) {
    raw_puts("0");
    return;
  }
  if (n < 0) {
    raw_puts("-");
    n = -n;
  }
  char buf[21];
  int i = 0;
  while (n > 0) {
    buf[i++] = '0' + (n % 10);
    n /= 10;
  }
  char out[2];
  out[1] = '\0';
  while (i--) {
    out[0] = buf[i];
    raw_puts(out);
  }
}

int main(int argc, char **argv, char **envp) {
  raw_puts("--- MUSL Stage 1 Test ---\n");

  raw_puts("argc: "); print_int(argc); raw_puts("\n");
  if (argc > 0 && argv[0]) {
    raw_puts("argv[0]: "); raw_puts(argv[0]); raw_puts("\n");
  }

  // Find AuxV
  char **e = envp;
  while (*e) e++;
  AuxEntry *auxv = (AuxEntry *)(e + 1);

  raw_puts("Auxiliary Vector:\n");
  for (AuxEntry *a = auxv; a->a_type != AT_NULL; a++) {
    switch (a->a_type) {
      case AT_PHDR:   raw_puts("  AT_PHDR:   "); print_hex(a->a_val); raw_puts("\n"); break;
      case AT_PHENT:  raw_puts("  AT_PHENT:  "); print_int(a->a_val); raw_puts("\n"); break;
      case AT_PHNUM:  raw_puts("  AT_PHNUM:  "); print_int(a->a_val); raw_puts("\n"); break;
      case AT_PAGESZ: raw_puts("  AT_PAGESZ: "); print_int(a->a_val); raw_puts("\n"); break;
      case AT_ENTRY:  raw_puts("  AT_ENTRY:  "); print_hex(a->a_val); raw_puts("\n"); break;
    }
  }

  raw_puts("\nTesting arch_prctl(ARCH_SET_FS)...\n");
  uint64_t original_fs = 0;
  if (syscall2(COREOS_SYS_ARCH_PRCTL, ARCH_GET_FS, (long)&original_fs) != 0) {
    raw_puts("arch_prctl(ARCH_GET_FS) preflight failed\n");
    return 1;
  }
  uint64_t my_tls_data = 0x12345678deadbeef;
  uint64_t *tls_ptr = &my_tls_data;
  
  if (syscall2(COREOS_SYS_ARCH_PRCTL, ARCH_SET_FS, (long)tls_ptr) == 0) {
    raw_puts("arch_prctl(ARCH_SET_FS) success\n");
    
    uint64_t fetched_fs = 0;
    if (syscall2(COREOS_SYS_ARCH_PRCTL, ARCH_GET_FS, (long)&fetched_fs) == 0) {
      raw_puts("arch_prctl(ARCH_GET_FS) returned: "); print_hex(fetched_fs); raw_puts("\n");
      if (fetched_fs == (uint64_t)tls_ptr) {
        raw_puts("MATCH: FS_BASE is correct!\n");
      } else {
        raw_puts("ERROR: FS_BASE mismatch!\n");
      }
    }

    uint64_t val_from_fs = 0;
    __asm__ volatile("movq %%fs:0, %0" : "=r"(val_from_fs));
    raw_puts("Value at %%fs:0: "); print_hex(val_from_fs); raw_puts("\n");
    if (val_from_fs == my_tls_data) {
       raw_puts("SUCCESS: Read from %%fs:0 matched TLS data!\n");
    } else {
       raw_puts("ERROR: Read from %%fs:0 failed!\n");
    }
    if (syscall2(COREOS_SYS_ARCH_PRCTL, ARCH_SET_FS, (long)original_fs) == 0) {
      raw_puts("arch_prctl(ARCH_SET_FS restore) success\n");
    } else {
      raw_puts("arch_prctl(ARCH_SET_FS restore) failed\n");
      return 1;
    }
  } else {
    raw_puts("arch_prctl(ARCH_SET_FS) failed\n");
  }

  raw_puts("\n--- Stage 1 Test Complete ---\n");
  return 0;
}
