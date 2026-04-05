// libcshim.c — minimal libc implementation for CoreOS userspace
#include "libcshim.h"
#include <stdarg.h>

// ---------------------------------------------------------------------------
// I/O (thin wrappers — the real implementations are inline in libcshim.h)
// ---------------------------------------------------------------------------

ssize_t read(int fd, void *buf, size_t count) {
    return sys_read(fd, buf, count);
}

ssize_t write(int fd, const void *buf, size_t count) {
    return sys_write(fd, buf, count);
}

int ioctl(int fd, unsigned long req, ...) {
    va_list ap;
    void *argp;
    va_start(ap, req);
    argp = va_arg(ap, void *);
    va_end(ap);
    return sys_ioctl(fd, req, argp);
}

int fcntl(int fd, int cmd, ...) {
    va_list ap;
    long arg = 0;
    va_start(ap, cmd);
    arg = va_arg(ap, long);
    va_end(ap);
    return sys_fcntl(fd, cmd, arg);
}

int pipe(int pipefd[2]) {
    return sys_pipe(pipefd);
}

int pipe2(int pipefd[2], int flags) {
    return sys_pipe2(pipefd, flags);
}

ssize_t readv(int fd, const struct iovec *iov, int iovcnt) {
    return sys_readv(fd, iov, iovcnt);
}

ssize_t writev(int fd, const struct iovec *iov, int iovcnt) {
    return sys_writev(fd, iov, iovcnt);
}

ssize_t preadv(int fd, const struct iovec *iov, int iovcnt, off_t offset) {
    return sys_preadv(fd, iov, iovcnt, offset);
}

ssize_t pwritev(int fd, const struct iovec *iov, int iovcnt, off_t offset) {
    return sys_pwritev(fd, iov, iovcnt, offset);
}

off_t lseek(int fd, off_t offset, int whence) {
    return (off_t)sys_lseek(fd, (long)offset, whence);
}

int fstat(int fd, struct stat *st) {
    return sys_fstat(fd, st);
}

int getpid(void) {
    return (int)sys_getpid();
}

int getppid(void) {
    return (int)sys_getppid();
}

int getuid(void) {
    return (int)sys_getuid();
}

int geteuid(void) {
    return (int)sys_geteuid();
}

int getgid(void) {
    return (int)sys_getgid();
}

int getegid(void) {
    return (int)sys_getegid();
}

int gettid(void) {
    return (int)sys_gettid();
}

int setuid(int uid) {
    return sys_setuid(uid);
}

int setgid(int gid) {
    return sys_setgid(gid);
}

int kill(int pid, int sig) {
    return sys_kill(pid, sig);
}

int getpgrp(void) {
    return (int)sys_getpgrp();
}

int setpgid(int pid, int pgid) {
    return sys_setpgid(pid, pgid);
}

int getpgid(int pid) {
    return (int)sys_getpgid(pid);
}

int getsid(int pid) {
    return (int)sys_getsid(pid);
}

int getresuid(int *ruid, int *euid, int *suid) {
    return sys_getresuid(ruid, euid, suid);
}

int getresgid(int *rgid, int *egid, int *sgid) {
    return sys_getresgid(rgid, egid, sgid);
}

char *getcwd(char *buf, size_t size) {
    return sys_getcwd(buf, size);
}

int chdir(const char *path) {
    return sys_chdir(path);
}

int truncate(const char *path, off_t length) {
    return sys_truncate(path, length);
}

int ftruncate(int fd, off_t length) {
    return sys_ftruncate(fd, length);
}

int getrlimit(int resource, struct rlimit *rlim) {
    return sys_getrlimit(resource, rlim);
}

int gettimeofday(struct timeval *tv, void *tz) {
    return sys_gettimeofday(tv, tz);
}

int sigaltstack(const stack_t *ss, stack_t *old_ss) {
    return sys_sigaltstack(ss, old_ss);
}

int sigaction(int sig, const struct sigaction *act, struct sigaction *oldact) {
    return sys_rt_sigaction(sig, act, oldact, sizeof(sigset_t));
}

int sigprocmask(int how, const sigset_t *set, sigset_t *oldset) {
    return sys_rt_sigprocmask(how, set, oldset, sizeof(sigset_t));
}

unsigned int umask(unsigned int mask) {
    return sys_umask(mask);
}

ssize_t pread(int fd, void *buf, size_t count, off_t offset) {
    return sys_pread64(fd, buf, count, offset);
}

ssize_t pwrite(int fd, const void *buf, size_t count, off_t offset) {
    return sys_pwrite64(fd, buf, count, offset);
}

ssize_t sendfile(int out_fd, int in_fd, off_t *offset, size_t count) {
    return sys_sendfile(out_fd, in_fd, offset, count);
}

int faccessat(int dirfd, const char *path, int mode, int flags) {
    return sys_faccessat(dirfd, path, mode, flags);
}

int sysinfo(struct sysinfo *info) {
    return sys_sysinfo(info);
}

void *mmap(void *addr, size_t len, int prot, int flags, int fd, off_t off) {
    return sys_mmap(addr, len, prot, flags, fd, off);
}

int munmap(void *addr, size_t len) {
    return sys_munmap(addr, len);
}

int mprotect(void *addr, size_t len, int prot) {
    return sys_mprotect(addr, len, prot);
}

int nanosleep(const struct timespec *req, struct timespec *rem) {
    return sys_nanosleep(req, rem);
}

int clock_gettime(int clockid, struct timespec *tp) {
    return sys_clock_gettime(clockid, tp);
}

int clock_nanosleep(int clockid, int flags, const struct timespec *req,
                    struct timespec *rem) {
    return sys_clock_nanosleep(clockid, flags, req, rem);
}

int dup(int fd) {
    return sys_dup(fd);
}

int dup2(int oldfd, int newfd) {
    return sys_dup3(oldfd, newfd, 0);
}

int dup3(int oldfd, int newfd, int flags) {
    return sys_dup3(oldfd, newfd, flags);
}

int fork(void) {
    return sys_fork();
}

int open(const char *path, int flags, ...) {
    va_list ap;
    int mode = 0;
    va_start(ap, flags);
    if (flags & O_CREAT) {
        mode = va_arg(ap, int);
    }
    va_end(ap);
    return sys_open(path, flags, mode);
}

int openat(int dirfd, const char *path, int flags, ...) {
    va_list ap;
    int mode = 0;
    va_start(ap, flags);
    if (flags & O_CREAT) {
        mode = va_arg(ap, int);
    }
    va_end(ap);
    return sys_openat(dirfd, path, flags, mode);
}

int unlinkat(int dirfd, const char *path, int flags) {
    return sys_unlinkat(dirfd, path, flags);
}

ssize_t readlink(const char *path, char *buf, size_t bufsiz) {
    return sys_readlink(path, buf, bufsiz);
}

ssize_t readlinkat(int dirfd, const char *path, char *buf, size_t bufsiz) {
    return sys_readlinkat(dirfd, path, buf, bufsiz);
}

long getrandom(void *buf, size_t len, unsigned int flags) {
    return sys_getrandom(buf, len, flags);
}

int arch_prctl(int code, unsigned long addr) {
    return sys_arch_prctl(code, addr);
}

int stat(const char *path, struct stat *st) {
    return sys_fstatat(AT_FDCWD, path, st, 0);
}

// ---------------------------------------------------------------------------
// printf — lightweight, no malloc needed
// ---------------------------------------------------------------------------

static void _puts_noln(const char *s) {
    sys_write(1, s, str_len(s));
}

static int _fmt_uint(char *buf, unsigned long long v, int base) {
    static const char hex[] = "0123456789abcdef";
    char tmp[24];
    int i = 0;
    if (v == 0) { buf[0] = '0'; buf[1] = '\0'; return 1; }
    while (v > 0) { tmp[i++] = hex[v % base]; v /= base; }
    int len = i;
    for (int j = 0; j < len; j++) buf[j] = tmp[len - 1 - j];
    buf[len] = '\0';
    return len;
}

int printf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int total = 0;
    char numbuf[24];

    for (const char *p = fmt; *p; p++) {
        if (*p != '%') {
            char c = *p;
            sys_write(1, &c, 1);
            total++;
            continue;
        }
        p++;
        int is_long = 0;
        if (*p == 'l') { is_long = 1; p++; }

        switch (*p) {
            case '%': { char c = '%'; sys_write(1, &c, 1); total++; break; }
            case 'c': {
                char c = (char)va_arg(ap, int);
                sys_write(1, &c, 1); total++; break;
            }
            case 's': {
                const char *s = va_arg(ap, const char *);
                if (!s) s = "(null)";
                size_t l = str_len(s);
                _puts_noln(s); total += (int)l; break;
            }
            case 'd': {
                long long v = is_long ? va_arg(ap, long) : (long long)va_arg(ap, int);
                if (v < 0) { char c = '-'; sys_write(1, &c, 1); total++; v = -v; }
                int l = _fmt_uint(numbuf, (unsigned long long)v, 10);
                _puts_noln(numbuf); total += l; break;
            }
            case 'u': {
                unsigned long long v = is_long
                    ? (unsigned long long)va_arg(ap, unsigned long)
                    : (unsigned long long)va_arg(ap, unsigned int);
                int l = _fmt_uint(numbuf, v, 10);
                _puts_noln(numbuf); total += l; break;
            }
            case 'x': {
                unsigned long long v = is_long
                    ? (unsigned long long)va_arg(ap, unsigned long)
                    : (unsigned long long)va_arg(ap, unsigned int);
                int l = _fmt_uint(numbuf, v, 16);
                _puts_noln(numbuf); total += l; break;
            }
            default: {
                char c = '%'; sys_write(1, &c, 1);
                sys_write(1, p, 1);
                total += 2; break;
            }
        }
    }
    va_end(ap);
    return total;
}

// ---------------------------------------------------------------------------
// String / memory
// ---------------------------------------------------------------------------

size_t strlen(const char *s) { return str_len(s); }

char *strcpy(char *dst, const char *src) {
    char *d = dst;
    while ((*d++ = *src++));
    return dst;
}

char *strncpy(char *dst, const char *src, size_t n) {
    size_t i;
    for (i = 0; i < n && src[i]; i++) dst[i] = src[i];
    for (; i < n; i++) dst[i] = '\0';
    return dst;
}

int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (a[i] != b[i]) return (unsigned char)a[i] - (unsigned char)b[i];
        if (!a[i]) break;
    }
    return 0;
}

char *strchr(const char *s, int c) {
    while (*s) {
        if (*s == (char)c) return (char *)s;
        s++;
    }
    return (c == '\0') ? (char *)s : NULL;
}

void *memset(void *dst, int c, size_t n) {
    unsigned char *d = dst;
    while (n--) *d++ = (unsigned char)c;
    return dst;
}

void *memcpy(void *dst, const void *src, size_t n) {
    unsigned char *d = dst;
    const unsigned char *s = src;
    while (n--) *d++ = *s++;
    return dst;
}

int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *p = a, *q = b;
    for (size_t i = 0; i < n; i++) {
        if (p[i] != q[i]) return (int)p[i] - (int)q[i];
    }
    return 0;
}

// ---------------------------------------------------------------------------
// Heap — bump allocator over brk()
//
// FIX: was calling _sys_brk() (undefined) instead of sys_brk().
//      sys_brk(0) returns the current break; sys_brk(addr) sets it.
// ---------------------------------------------------------------------------

#define HEAP_CHUNK (64 * 1024)

static char *heap_ptr = NULL;

void *malloc(size_t size) {
    size = (size + 15) & ~(size_t)15;  // align to 16 bytes

    if (!heap_ptr) {
        // First call: get current break as heap base.
        // sys_brk(0) returns current brk without changing it.
        heap_ptr = (char *)sys_brk(0);
    }

    char *result = heap_ptr;
    char *new_ptr = heap_ptr + size;

    // Extend the break if the new pointer exceeds what the kernel has mapped.
    // We extend in HEAP_CHUNK increments to amortise syscall cost.
    // sys_brk(addr) returns the NEW break (may be <= addr on OOM).
    char *current_end = (char *)sys_brk(0);
    if (new_ptr > current_end) {
        size_t extra = (size_t)(new_ptr - current_end);
        extra = (extra + HEAP_CHUNK - 1) & ~(size_t)(HEAP_CHUNK - 1);
        char *requested = current_end + extra;
        char *got = (char *)sys_brk((long)requested);
        if (got < new_ptr) {
            // OOM — kernel didn't extend far enough.
            return NULL;
        }
    }

    heap_ptr = new_ptr;
    return result;
}

void free(void *ptr) {
    (void)ptr;  // bump allocator — no-op until musl lands
}

// ---------------------------------------------------------------------------
// Process
// ---------------------------------------------------------------------------

void exit(int code) {
    sys_exit(code);
}
