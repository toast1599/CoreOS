#pragma once
#include <stddef.h>
#include <stdint.h>

typedef long ssize_t;
typedef long off_t;
typedef long time_t;

#define PROT_NONE 0
#define PROT_EXEC 1
#define PROT_WRITE 2
#define PROT_READ 4

#define MAP_PRIVATE 0x02
#define MAP_FIXED 0x10
#define MAP_ANONYMOUS 0x20
#define MAP_FAILED ((void *)-1)

#define CLOCK_REALTIME 0
#define CLOCK_MONOTONIC 1
#define AT_FDCWD (-100)
#define O_CLOEXEC 02000000
#define O_APPEND 02000
#define O_NONBLOCK 04000
#define O_RDONLY 0
#define TCGETS 0x5401
#define TIOCGWINSZ 0x5413
#define FD_CLOEXEC 1
#define F_DUPFD 0
#define F_GETFD 1
#define F_SETFD 2
#define F_GETFL 3
#define F_SETFL 4
#define F_DUPFD_CLOEXEC 1030

struct timespec {
  time_t tv_sec;
  long tv_nsec;
};

struct stat {
  uint64_t st_dev;
  uint64_t st_ino;
  uint32_t st_mode;
  uint32_t st_nlink;
  uint32_t st_uid;
  uint32_t st_gid;
  uint64_t st_rdev;
  int64_t st_size;
  int64_t st_blksize;
  int64_t st_blocks;
  int64_t st_atime;
  int64_t st_mtime;
  int64_t st_ctime;
};

struct iovec {
  void *iov_base;
  size_t iov_len;
};

struct winsize {
  unsigned short ws_row;
  unsigned short ws_col;
  unsigned short ws_xpixel;
  unsigned short ws_ypixel;
};

struct termios {
  unsigned int c_iflag;
  unsigned int c_oflag;
  unsigned int c_cflag;
  unsigned int c_lflag;
  unsigned char c_line;
  unsigned char c_cc[32];
  unsigned int c_ispeed;
  unsigned int c_ospeed;
};

ssize_t read(int fd, void *buf, size_t count);
ssize_t write(int fd, const void *buf, size_t count);
ssize_t readv(int fd, const struct iovec *iov, int iovcnt);
ssize_t writev(int fd, const struct iovec *iov, int iovcnt);
off_t lseek(int fd, off_t offset, int whence);
int fstat(int fd, struct stat *st);
int stat(const char *path, struct stat *st);
int getpid(void);
void *mmap(void *addr, size_t len, int prot, int flags, int fd, off_t off);
int munmap(void *addr, size_t len);
int mprotect(void *addr, size_t len, int prot);
int nanosleep(const struct timespec *req, struct timespec *rem);
int clock_gettime(int clockid, struct timespec *tp);
int dup(int fd);
int dup2(int oldfd, int newfd);
int dup3(int oldfd, int newfd, int flags);
int fork(void);
int openat(int dirfd, const char *path, int flags, ...);
int ioctl(int fd, unsigned long req, ...);
int fcntl(int fd, int cmd, ...);
int pipe(int pipefd[2]);
int printf(const char *fmt, ...);
size_t strlen(const char *s);
char *strcpy(char *dst, const char *src);
char *strncpy(char *dst, const char *src, size_t n);
int strcmp(const char *a, const char *b);
int strncmp(const char *a, const char *b, size_t n);
char *strchr(const char *s, int c);
void *memset(void *dst, int c, size_t n);
void *memcpy(void *dst, const void *src, size_t n);
int memcmp(const void *a, const void *b, size_t n);
void *malloc(size_t size);
void free(void *ptr);

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

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

static inline long syscall4(long num, long a1, long a2, long a3, long a4) {
  long ret;
  __asm__ volatile("mov %5, %%r10; syscall"
                   : "=a"(ret)
                   : "0"(num), "D"(a1), "S"(a2), "d"(a3), "r"(a4)
                   : "rcx", "r10", "r11", "memory");
  return ret;
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

static inline int sys_fstat(int fd, struct stat *st) {
  return (int)syscall2(29, fd, (long)st);
}

static inline long sys_lseek(int fd, long offset, int whence) {
  return syscall3(30, fd, offset, whence);
}

static inline long sys_ls(char *buf, size_t buf_len) {
  return syscall2(6, (long)buf, (long)buf_len);
}

static inline int sys_touch(const char *name, size_t name_len) {
  return (int)syscall2(7, (long)name, (long)name_len);
}

static inline int sys_ioctl(int fd, unsigned long req, void *argp) {
  return (int)syscall3(16, fd, (long)req, (long)argp);
}

static inline int sys_fcntl(int fd, int cmd, long arg) {
  return (int)syscall3(72, fd, cmd, arg);
}

static inline ssize_t sys_readv(int fd, const struct iovec *iov, int iovcnt) {
  return syscall3(19, fd, (long)iov, iovcnt);
}

static inline int sys_rm(const char *name, size_t name_len) {
  return (int)syscall2(8, (long)name, (long)name_len);
}

// write_file and push_file pass data via struct to fit in 3 syscall args
typedef struct {
  uint64_t ptr;
  uint64_t len;
} DataArgs;

typedef struct {
  uint64_t addr;
  uint64_t len;
  uint32_t prot;
  uint32_t flags;
  int32_t fd;
  int64_t off;
} MmapArgs;

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

static inline int sys_pipe(int pipefd[2]) {
  return (int)syscall1(22, (long)pipefd);
}

// exec(path, path_len) — spawn RamFS ELF, returns pid or 0
static inline long sys_exec(const char *path, size_t path_len) {
  return syscall2(57, (long)path, (long)path_len);
}

static inline long sys_getpid(void) { return syscall1(39, 0); }
static inline void *sys_mmap(void *addr, size_t len, int prot, int flags, int fd,
                             off_t off) {
  MmapArgs args = {(uint64_t)addr, (uint64_t)len, (uint32_t)prot,
                   (uint32_t)flags, (int32_t)fd, (int64_t)off};
  return (void *)syscall1(31, (long)&args);
}
static inline int sys_mprotect(void *addr, size_t len, int prot) {
  return (int)syscall3(32, (long)addr, (long)len, (long)prot);
}
static inline int sys_munmap(void *addr, size_t len) {
  return (int)syscall2(33, (long)addr, (long)len);
}
static inline int sys_nanosleep(const struct timespec *req,
                                struct timespec *rem) {
  return (int)syscall2(35, (long)req, (long)rem);
}
static inline int sys_dup(int fd) { return (int)syscall1(34, fd); }
static inline int sys_clock_gettime(int clockid, struct timespec *tp) {
  return (int)syscall2(228, (long)clockid, (long)tp);
}
static inline int sys_fork(void) { return (int)syscall1(58, 0); }
static inline int sys_dup3(int oldfd, int newfd, int flags) {
  return (int)syscall3(292, oldfd, newfd, flags);
}
static inline ssize_t sys_writev(int fd, const struct iovec *iov, int iovcnt) {
  return syscall3(20, fd, (long)iov, iovcnt);
}
static inline int sys_openat(int dirfd, const char *path, size_t path_len,
                             int flags) {
  return (int)syscall4(257, dirfd, (long)path, (long)path_len, flags);
}
static inline int sys_fstatat(int dirfd, const char *path, size_t path_len,
                              struct stat *st) {
  return (int)syscall4(262, dirfd, (long)path, (long)path_len, (long)st);
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

static inline long sys_meminfo() { return syscall1(36, 0); }
static inline long sys_uptime() { return syscall1(21, 0); }
static inline long sys_ticks() { return syscall1(37, 0); }
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
