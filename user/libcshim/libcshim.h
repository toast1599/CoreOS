#pragma once
#include <stddef.h>
#include <stdint.h>

typedef long ssize_t;

// ---------------------------------------------------------------------------
// Raw syscall wrappers
// ---------------------------------------------------------------------------

static inline long syscall1(long num, long a1) {
  long ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "0"(num), "D"(a1)
                   : "rcx", "r11", "memory");
  return ret;
}

static inline long syscall3(long num, long a1, long a2, long a3) {
  long ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "0"(num), "D"(a1), "S"(a2), "d"(a3)
                   : "rcx", "r11", "memory");
  return ret;
}

static inline long syscall2(long num, long a1, long a2) {
  return syscall3(num, a1, a2, 0);
}

// ---------------------------------------------------------------------------
// Syscall interface
// ---------------------------------------------------------------------------

// read(fd, buf, count) — fd=0 blocks until data is available
static inline long sys_read(int fd, void *buf, size_t count) {
  return syscall3(0, fd, (long)buf, (long)count);
}

// write(fd, buf, count) — fd=1 goes to serial
static inline long sys_write(int fd, const void *buf, size_t count) {
  return syscall3(1, fd, (long)buf, (long)count);
}

// open(path, path_len) — returns fd >= 3 or -1
static inline int sys_open(const char *path, size_t path_len) {
  return (int)syscall2(3, (long)path, (long)path_len);
}

// close(fd)
static inline int sys_close(int fd) { return (int)syscall1(4, fd); }

// fsize(fd) — total byte size of open file
static inline long sys_fsize(int fd) { return syscall1(5, fd); }

static inline long sys_ls(char *buf, size_t buf_len) {
  return syscall2(6, (long)buf, (long)buf_len);
}

static inline int sys_touch(const char *name, size_t name_len) {
  return (int)syscall2(7, (long)name, (long)name_len);
}

static inline int sys_rm(const char *name, size_t name_len) {
  return (int)syscall2(8, (long)name, (long)name_len);
}

// write_file and push_file pass data via struct to fit in 3 syscall args
typedef struct {
  uint64_t ptr;
  uint64_t len;
} DataArgs;

static inline int sys_write_file(const char *name, size_t name_len,
                                 const void *data, size_t data_len) {
  DataArgs args = {(uint64_t)data, (uint64_t)data_len};
  return (int)syscall3(9, (long)name, (long)name_len, (long)&args);
}

static inline int sys_push_file(const char *name, size_t name_len,
                                const void *data, size_t data_len) {
  DataArgs args = {(uint64_t)data, (uint64_t)data_len};
  return (int)syscall3(10, (long)name, (long)name_len, (long)&args);
}

// exec(path, path_len) — spawn RamFS ELF, returns pid or 0
static inline long sys_exec(const char *path, size_t path_len) {
  return syscall2(57, (long)path, (long)path_len);
}

// waitpid(pid) — block until child exits, returns exit code
static inline long sys_waitpid(long pid) { return syscall1(61, pid); }

// exit(code)
static inline void sys_exit(int code) {
  syscall1(60, code);
  __builtin_unreachable();
}

// brk(addr) — returns new break
static inline long sys_brk(long addr) { return syscall1(12, addr); }

static inline long sys_meminfo() { return syscall1(20, 0); }
static inline long sys_uptime() { return syscall1(21, 0); }
static inline long sys_ticks() { return syscall1(22, 0); }
static inline void sys_reboot() { syscall1(23, 0); }
static inline void sys_panic() { syscall1(24, 0); }
static inline long sys_boottime(char *buf, int len) {
  return syscall3(25, (long)buf, (long)len, 0);
}
static inline void sys_clear() { syscall1(26, 0); }
static inline void sys_sleep(long ticks) { syscall1(27, ticks); }
static inline void sys_set_font_scale(int scale) { syscall1(28, (long)scale); }

// ---------------------------------------------------------------------------
// Higher-level helpers
// ---------------------------------------------------------------------------

// Write a null-terminated string to stdout
static inline void puts_fd(int fd, const char *s) {
  size_t len = 0;
  while (s[len])
    len++;
  sys_write(fd, s, len);
}

#define puts(s) puts_fd(1, s)
#define putchar(c)                                                             \
  do {                                                                         \
    char _c = (c);                                                             \
    sys_write(1, &_c, 1);                                                      \
  } while (0)

// Read a single character from stdin (blocking)
static inline char getchar(void) {
  char c;
  sys_read(0, &c, 1);
  return c;
}

// Read a line from stdin into buf (up to buf_len-1 chars).
// Echoes characters as they are typed.
// Handles backspace. Returns number of chars read (not including '\0').
static inline int readline(char *buf, int buf_len) {
  int i = 0;
  while (i < buf_len - 1) {
    char c = getchar();
    if (c == '\0') {
      // Discard null characters to prevent string mangling
      continue;
    }
    if (c == '\n' || c == '\r') {
      putchar('\n');
      break;
    } else if (c == '\x08') {
      // backspace
      if (i > 0) {
        i--;
        // Overwrite on terminal: backspace + space + backspace
        puts("\x08 \x08");
      }
    } else {
      buf[i++] = c;
      putchar(c);
    }
  }
  buf[i] = '\0';
  return i;
}

// Simple strlen
static inline size_t str_len(const char *s) {
  size_t n = 0;
  while (s[n])
    n++;
  return n;
}

// Simple strcmp: returns 0 if equal
static inline int str_eq(const char *a, const char *b) {
  while (*a && *b && *a == *b) {
    a++;
    b++;
  }
  return *a == *b;
}

// Does string `s` start with `prefix`?
static inline int str_starts(const char *s, const char *prefix) {
  while (*prefix) {
    if (*s++ != *prefix++)
      return 0;
  }
  return 1;
}
