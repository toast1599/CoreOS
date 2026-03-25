#pragma once
#include <stddef.h>
#include <stdint.h>
#include "sysnr.h"

typedef long ssize_t;
typedef long off_t;
typedef long time_t;
typedef unsigned long sigset_t;

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
#define TIMER_ABSTIME 1
#define AT_FDCWD (-100)
#define AT_REMOVEDIR 0x200
#define F_OK 0
#define X_OK 1
#define W_OK 2
#define R_OK 4
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
#define ARCH_SET_FS 0x1002
#define ARCH_GET_FS 0x1003
#define GRND_NONBLOCK 0x0001
#define GRND_RANDOM 0x0002

struct timespec {
  time_t tv_sec;
  long tv_nsec;
};

struct timeval {
  time_t tv_sec;
  long tv_usec;
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

struct rlimit {
  uint64_t rlim_cur;
  uint64_t rlim_max;
};

typedef struct {
  void *ss_sp;
  int ss_flags;
  size_t ss_size;
} stack_t;

struct sysinfo {
  long uptime;
  unsigned long loads[3];
  unsigned long totalram;
  unsigned long freeram;
  unsigned long sharedram;
  unsigned long bufferram;
  unsigned long totalswap;
  unsigned long freeswap;
  unsigned short procs;
  unsigned long totalhigh;
  unsigned long freehigh;
  unsigned int mem_unit;
  char _pad[8];
};

ssize_t read(int fd, void *buf, size_t count);
ssize_t write(int fd, const void *buf, size_t count);
ssize_t readv(int fd, const struct iovec *iov, int iovcnt);
ssize_t writev(int fd, const struct iovec *iov, int iovcnt);
ssize_t preadv(int fd, const struct iovec *iov, int iovcnt, off_t offset);
ssize_t pwritev(int fd, const struct iovec *iov, int iovcnt, off_t offset);
off_t lseek(int fd, off_t offset, int whence);
int fstat(int fd, struct stat *st);
int stat(const char *path, struct stat *st);
int getpid(void);
int getppid(void);
int getuid(void);
int geteuid(void);
int getgid(void);
int getegid(void);
int gettid(void);
int setuid(int uid);
int setgid(int gid);
int kill(int pid, int sig);
int getpgrp(void);
int setpgid(int pid, int pgid);
int getpgid(int pid);
int getsid(int pid);
int getresuid(int *ruid, int *euid, int *suid);
int getresgid(int *rgid, int *egid, int *sgid);
char *getcwd(char *buf, size_t size);
int chdir(const char *path);
int truncate(const char *path, off_t length);
int ftruncate(int fd, off_t length);
int getrlimit(int resource, struct rlimit *rlim);
int gettimeofday(struct timeval *tv, void *tz);
int sigaltstack(const stack_t *ss, stack_t *old_ss);
unsigned int umask(unsigned int mask);
ssize_t pread(int fd, void *buf, size_t count, off_t offset);
ssize_t pwrite(int fd, const void *buf, size_t count, off_t offset);
ssize_t sendfile(int out_fd, int in_fd, off_t *offset, size_t count);
int pipe2(int pipefd[2], int flags);
int faccessat(int dirfd, const char *path, int mode, int flags);
int sysinfo(struct sysinfo *info);
void *mmap(void *addr, size_t len, int prot, int flags, int fd, off_t off);
int munmap(void *addr, size_t len);
int mprotect(void *addr, size_t len, int prot);
int nanosleep(const struct timespec *req, struct timespec *rem);
int clock_gettime(int clockid, struct timespec *tp);
int clock_nanosleep(int clockid, int flags, const struct timespec *req, struct timespec *rem);
int dup(int fd);
int dup2(int oldfd, int newfd);
int dup3(int oldfd, int newfd, int flags);
int fork(void);
int openat(int dirfd, const char *path, int flags, ...);
int unlinkat(int dirfd, const char *path, int flags);
ssize_t readlink(const char *path, char *buf, size_t bufsiz);
ssize_t readlinkat(int dirfd, const char *path, char *buf, size_t bufsiz);
long getrandom(void *buf, size_t len, unsigned int flags);
int arch_prctl(int code, unsigned long addr);
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
  return syscall3(COREOS_SYS_READ, fd, (long)buf, (long)count);
}

// write(fd, buf, count) — fd=1 goes to serial
static inline long sys_write(int fd, const void *buf, size_t count) {
  return syscall3(COREOS_SYS_WRITE, fd, (long)buf, (long)count);
}

// open(path, path_len) — returns fd >= 3 or -1
static inline int sys_open(const char *path, size_t path_len) {
  return (int)syscall2(COREOS_SYS_OPEN, (long)path, (long)path_len);
}

// close(fd)
static inline int sys_close(int fd) { return (int)syscall1(COREOS_SYS_CLOSE, fd); }

// fsize(fd) — total byte size of open file
static inline long sys_fsize(int fd) { return syscall1(COREOS_SYS_FSIZE, fd); }

static inline int sys_fstat(int fd, struct stat *st) {
  return (int)syscall2(COREOS_SYS_FSTAT, fd, (long)st);
}

static inline long sys_lseek(int fd, long offset, int whence) {
  return syscall3(COREOS_SYS_LSEEK, fd, offset, whence);
}

static inline long sys_ls(char *buf, size_t buf_len) {
  return syscall2(COREOS_SYS_LS, (long)buf, (long)buf_len);
}

static inline int sys_touch(const char *name, size_t name_len) {
  return (int)syscall2(COREOS_SYS_TOUCH, (long)name, (long)name_len);
}

static inline int sys_ioctl(int fd, unsigned long req, void *argp) {
  return (int)syscall3(COREOS_SYS_IOCTL, fd, (long)req, (long)argp);
}

static inline int sys_fcntl(int fd, int cmd, long arg) {
  return (int)syscall3(COREOS_SYS_FCNTL, fd, cmd, arg);
}

static inline ssize_t sys_pread64(int fd, void *buf, size_t count, off_t offset) {
  return syscall4(COREOS_SYS_PREAD64, fd, (long)buf, (long)count, (long)offset);
}

static inline ssize_t sys_pwrite64(int fd, const void *buf, size_t count, off_t offset) {
  return syscall4(COREOS_SYS_PWRITE64, fd, (long)buf, (long)count, (long)offset);
}

static inline ssize_t sys_sendfile(int out_fd, int in_fd, off_t *offset, size_t count) {
  return syscall4(COREOS_SYS_SENDFILE, out_fd, in_fd, (long)offset, (long)count);
}

static inline ssize_t sys_readv(int fd, const struct iovec *iov, int iovcnt) {
  return syscall3(COREOS_SYS_READV, fd, (long)iov, iovcnt);
}

static inline ssize_t sys_preadv(int fd, const struct iovec *iov, int iovcnt,
                                 off_t offset) {
  return syscall4(COREOS_SYS_PREADV, fd, (long)iov, iovcnt, (long)offset);
}

static inline ssize_t sys_pwritev(int fd, const struct iovec *iov, int iovcnt,
                                  off_t offset) {
  return syscall4(COREOS_SYS_PWRITEV, fd, (long)iov, iovcnt, (long)offset);
}

static inline int sys_rm(const char *name, size_t name_len) {
  return (int)syscall2(COREOS_SYS_RM, (long)name, (long)name_len);
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
  return (int)syscall3(COREOS_SYS_WRITE_FILE, (long)name, (long)name_len, (long)&args);
}

static inline int sys_push_file(const char *name, size_t name_len,
                                const void *data, size_t data_len) {
  DataArgs args = {(uint64_t)data, (uint64_t)data_len};
  return (int)syscall3(COREOS_SYS_PUSH_FILE, (long)name, (long)name_len, (long)&args);
}

static inline int sys_pipe(int pipefd[2]) {
  return (int)syscall1(COREOS_SYS_PIPE, (long)pipefd);
}
static inline int sys_pipe2(int pipefd[2], int flags) {
  return (int)syscall2(COREOS_SYS_PIPE2, (long)pipefd, flags);
}

// exec(path, path_len) — spawn RamFS ELF, returns pid or 0
static inline long sys_exec(const char *path, size_t path_len) {
  return syscall2(COREOS_SYS_EXEC, (long)path, (long)path_len);
}

static inline long sys_getpid(void) { return syscall1(COREOS_SYS_GETPID, 0); }
static inline long sys_getppid(void) { return syscall1(COREOS_SYS_GETPPID, 0); }
static inline long sys_getuid(void) { return syscall1(COREOS_SYS_GETUID, 0); }
static inline long sys_geteuid(void) { return syscall1(COREOS_SYS_GETEUID, 0); }
static inline long sys_getgid(void) { return syscall1(COREOS_SYS_GETGID, 0); }
static inline long sys_getegid(void) { return syscall1(COREOS_SYS_GETEGID, 0); }
static inline long sys_gettid(void) { return syscall1(COREOS_SYS_GETTID, 0); }
static inline long sys_getpgrp(void) { return syscall1(COREOS_SYS_GETPGRP, 0); }
static inline long sys_getpgid(int pid) { return syscall1(COREOS_SYS_GETPGID, pid); }
static inline long sys_getsid(int pid) { return syscall1(COREOS_SYS_GETSID, pid); }
static inline int sys_setuid(int uid) { return (int)syscall1(COREOS_SYS_SETUID, uid); }
static inline int sys_setgid(int gid) { return (int)syscall1(COREOS_SYS_SETGID, gid); }
static inline int sys_setpgid(int pid, int pgid) {
  return (int)syscall2(COREOS_SYS_SETPGID, pid, pgid);
}
static inline int sys_kill(int pid, int sig) {
  return (int)syscall2(COREOS_SYS_KILL, pid, sig);
}
static inline int sys_getresuid(int *ruid, int *euid, int *suid) {
  return (int)syscall3(COREOS_SYS_GETRESUID, (long)ruid, (long)euid, (long)suid);
}
static inline int sys_getresgid(int *rgid, int *egid, int *sgid) {
  return (int)syscall3(COREOS_SYS_GETRESGID, (long)rgid, (long)egid, (long)sgid);
}
static inline int sys_rt_sigaction(int sig, const void *act, void *oldact,
                                   size_t sigsetsize) {
  return (int)syscall4(COREOS_SYS_RT_SIGACTION, sig, (long)act, (long)oldact,
                       (long)sigsetsize);
}
static inline int sys_rt_sigprocmask(int how, const void *set, void *oldset,
                                     size_t sigsetsize) {
  return (int)syscall4(COREOS_SYS_RT_SIGPROCMASK, how, (long)set, (long)oldset,
                       (long)sigsetsize);
}
static inline char *sys_getcwd(char *buf, size_t size) {
  return (char *)syscall2(COREOS_SYS_GETCWD, (long)buf, (long)size);
}
static inline int sys_chdir(const char *path, size_t path_len) {
  return (int)syscall2(COREOS_SYS_CHDIR, (long)path, (long)path_len);
}
static inline int sys_truncate(const char *path, size_t path_len) {
  return (int)syscall2(COREOS_SYS_TRUNCATE, (long)path, (long)path_len);
}
static inline int sys_ftruncate(int fd, off_t len) {
  return (int)syscall2(COREOS_SYS_FTRUNCATE, fd, (long)len);
}
static inline int sys_getrlimit(int resource, struct rlimit *rlim) {
  return (int)syscall2(COREOS_SYS_GETRLIMIT, resource, (long)rlim);
}
static inline int sys_gettimeofday(struct timeval *tv, void *tz) {
  return (int)syscall2(COREOS_SYS_GETTIMEOFDAY, (long)tv, (long)tz);
}
static inline int sys_sigaltstack(const stack_t *ss, stack_t *old_ss) {
  return (int)syscall2(COREOS_SYS_SIGALTSTACK, (long)ss, (long)old_ss);
}
static inline unsigned int sys_umask(unsigned int mask) {
  return (unsigned int)syscall1(COREOS_SYS_UMASK, mask);
}
static inline int sys_faccessat(int dirfd, const char *path, size_t path_len, int mode) {
  return (int)syscall4(COREOS_SYS_FACCESSAT, dirfd, (long)path, (long)path_len, mode);
}
static inline int sys_sysinfo(struct sysinfo *info) {
  return (int)syscall1(COREOS_SYS_SYSINFO, (long)info);
}
static inline int sys_set_tid_address(int *tidptr) {
  return (int)syscall1(COREOS_SYS_SET_TID_ADDRESS, (long)tidptr);
}
static inline void *sys_mmap(void *addr, size_t len, int prot, int flags, int fd,
                             off_t off) {
  MmapArgs args = {(uint64_t)addr, (uint64_t)len, (uint32_t)prot,
                   (uint32_t)flags, (int32_t)fd, (int64_t)off};
  return (void *)syscall1(COREOS_SYS_MMAP, (long)&args);
}
static inline int sys_mprotect(void *addr, size_t len, int prot) {
  return (int)syscall3(COREOS_SYS_MPROTECT, (long)addr, (long)len, (long)prot);
}
static inline int sys_munmap(void *addr, size_t len) {
  return (int)syscall2(COREOS_SYS_MUNMAP, (long)addr, (long)len);
}
static inline int sys_nanosleep(const struct timespec *req,
                                struct timespec *rem) {
  return (int)syscall2(COREOS_SYS_NANOSLEEP, (long)req, (long)rem);
}
static inline int sys_dup(int fd) { return (int)syscall1(COREOS_SYS_DUP, fd); }
static inline int sys_clock_gettime(int clockid, struct timespec *tp) {
  return (int)syscall2(COREOS_SYS_CLOCK_GETTIME, (long)clockid, (long)tp);
}
static inline int sys_clock_nanosleep(int clockid, int flags,
                                      const struct timespec *req,
                                      struct timespec *rem) {
  return (int)syscall4(COREOS_SYS_CLOCK_NANOSLEEP, clockid, flags, (long)req,
                       (long)rem);
}
static inline int sys_fork(void) { return (int)syscall1(COREOS_SYS_FORK, 0); }
static inline int sys_dup3(int oldfd, int newfd, int flags) {
  return (int)syscall3(COREOS_SYS_DUP3, oldfd, newfd, flags);
}
static inline ssize_t sys_writev(int fd, const struct iovec *iov, int iovcnt) {
  return syscall3(COREOS_SYS_WRITEV, fd, (long)iov, iovcnt);
}
static inline int sys_openat(int dirfd, const char *path, size_t path_len,
                             int flags) {
  return (int)syscall4(COREOS_SYS_OPENAT, dirfd, (long)path, (long)path_len, flags);
}
static inline int sys_unlinkat(int dirfd, const char *path, size_t path_len,
                               int flags) {
  return (int)syscall4(COREOS_SYS_UNLINKAT, dirfd, (long)path, (long)path_len, flags);
}
static inline ssize_t sys_readlink(const char *path, char *buf, size_t bufsiz) {
  return syscall3(COREOS_SYS_READLINK, (long)path, (long)buf, (long)bufsiz);
}
static inline ssize_t sys_readlinkat(int dirfd, const char *path, char *buf,
                                     size_t bufsiz) {
  return syscall4(COREOS_SYS_READLINKAT, dirfd, (long)path, (long)buf, (long)bufsiz);
}
static inline int sys_fstatat(int dirfd, const char *path, size_t path_len,
                              struct stat *st) {
  return (int)syscall4(COREOS_SYS_FSTATAT, dirfd, (long)path, (long)path_len, (long)st);
}
static inline long sys_getrandom(void *buf, size_t len, unsigned int flags) {
  return syscall3(COREOS_SYS_GETRANDOM, (long)buf, (long)len, flags);
}
static inline int sys_arch_prctl(int code, unsigned long addr) {
  return (int)syscall2(COREOS_SYS_ARCH_PRCTL, code, addr);
}

// waitpid(pid) — block until child exits, returns exit code
static inline long sys_waitpid(long pid) { return syscall1(COREOS_SYS_WAITPID, pid); }

// exit(code)
static inline void sys_exit(int code) {
  syscall1(COREOS_SYS_EXIT, code);
  __builtin_unreachable();
}

static inline void sys_exit_group(int code) {
  syscall1(COREOS_SYS_EXIT_GROUP, code);
  __builtin_unreachable();
}

// brk(addr) — returns new break
static inline long sys_brk(long addr) { return syscall1(COREOS_SYS_BRK, addr); }

static inline long sys_meminfo() { return syscall1(COREOS_SYS_FREE_BYTES, 0); }
static inline long sys_uptime() { return syscall1(COREOS_SYS_UPTIME_SECONDS, 0); }
static inline long sys_ticks() { return syscall1(COREOS_SYS_TICKS, 0); }
static inline void sys_reboot() { syscall1(COREOS_SYS_REBOOT, 0); }
static inline void sys_panic() { syscall1(COREOS_SYS_PANIC, 0); }
static inline long sys_boottime(char *buf, int len) {
  return syscall3(COREOS_SYS_BOOTTIME, (long)buf, (long)len, 0);
}
static inline void sys_clear() { syscall1(COREOS_SYS_CLEAR_TERMINAL, 0); }
static inline void sys_sleep(long ticks) { syscall1(COREOS_SYS_SLEEP_TICKS, ticks); }
static inline void sys_set_font_scale(int scale) { syscall1(COREOS_SYS_SET_FONT_SCALE, (long)scale); }

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
