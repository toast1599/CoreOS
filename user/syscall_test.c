#include "libcshim/libcshim.h"

#define TEST_FILE "syscall_probe"
#define CHILD_FILE "syscall_child"

static int failures = 0;
static int skipped = 0;

static void print_num(long n) {
  if (n < 0) {
    putchar('-');
    n = -n;
  }
  if (n == 0) {
    putchar('0');
    return;
  }
  char buf[32];
  int i = 0;
  while (n > 0) {
    buf[i++] = '0' + (n % 10);
    n /= 10;
  }
  while (i > 0)
    putchar(buf[--i]);
}

static void pass(const char *name) {
  puts("PASS ");
  puts(name);
  puts("\n");
}

static void fail(const char *name, long detail) {
  puts("FAIL ");
  puts(name);
  puts(" detail=");
  print_num(detail);
  puts("\n");
  failures++;
}

static void skip(const char *name) {
  puts("SKIP ");
  puts(name);
  puts("\n");
  skipped++;
}

static int str_eq_n(const char *a, const char *b, size_t n) {
  for (size_t i = 0; i < n; i++) {
    if (a[i] != b[i])
      return 0;
  }
  return 1;
}

static const char *find_substr(const char *haystack, const char *needle) {
  size_t needle_len = str_len(needle);
  if (needle_len == 0)
    return haystack;
  for (size_t i = 0; haystack[i]; i++) {
    if (strncmp(&haystack[i], needle, needle_len) == 0)
      return &haystack[i];
  }
  return 0;
}

static void check(int cond, const char *name, long detail) {
  if (cond)
    pass(name);
  else
    fail(name, detail);
}

static void test_basic_info(void) {
  long pid = getpid();
  check(pid > 0, "getpid", pid);

  long free_mem = sys_meminfo();
  check(free_mem > (64 * 1024 * 1024), "meminfo", free_mem);

  long uptime_before = sys_uptime();
  long ticks_before = sys_ticks();
  sys_sleep(2);
  long uptime_after = sys_uptime();
  long ticks_after = sys_ticks();
  check(uptime_after >= uptime_before, "uptime", uptime_after - uptime_before);
  check(ticks_after > ticks_before, "ticks", ticks_after - ticks_before);

  char boot[256];
  long boot_len = sys_boottime(boot, sizeof(boot) - 1);
  check(boot_len > 0, "boottime", boot_len);
  if (boot_len > 0) {
    boot[boot_len] = '\0';
    puts("boottime: ");
    puts(boot);
  }

  struct timespec t0, t1;
  int rc0 = clock_gettime(CLOCK_MONOTONIC, &t0);
  struct timespec req = {0, 20 * 1000 * 1000};
  int ns = nanosleep(&req, 0);
  int rc1 = clock_gettime(CLOCK_MONOTONIC, &t1);
  long delta_ns = (t1.tv_sec - t0.tv_sec) * 1000000000L + (t1.tv_nsec - t0.tv_nsec);
  check(rc0 == 0 && rc1 == 0, "clock_gettime", rc0 | rc1);
  check(ns == 0 && delta_ns >= 10 * 1000 * 1000L, "nanosleep", delta_ns);

  struct winsize ws;
  struct termios tio;
  check(ioctl(1, TIOCGWINSZ, &ws) == 0 && ws.ws_row > 0 && ws.ws_col > 0,
        "ioctl_tiocgwinsz", ws.ws_col);
  check(ioctl(1, TCGETS, &tio) == 0, "ioctl_tcgets", 0);
}

static void test_brk_and_vm(void) {
  long cur = sys_brk(0);
  check(cur > 0, "brk_get", cur);

  long grown = sys_brk(cur + 4096);
  check(grown >= cur + 4096, "brk_grow", grown);

  long shrunk = sys_brk(cur);
  check(shrunk == cur, "brk_shrink", shrunk);

  char *map = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (map == MAP_FAILED) {
    fail("mmap", -1);
    return;
  }
  map[0] = 'O';
  map[1] = 'K';
  check(map[0] == 'O' && map[1] == 'K', "mmap_write", map[0]);
  check(mprotect(map, 8192, PROT_READ) == 0, "mprotect", 0);
  check(munmap(map, 8192) == 0, "munmap", 0);
}

static void test_fs_and_fd(void) {
  char lsbuf[1024];
  long lslen = sys_ls(lsbuf, sizeof(lsbuf) - 1);
  check(lslen > 0, "ls", lslen);
  if (lslen > 0) {
    lsbuf[lslen] = '\0';
    check(find_substr(lsbuf, "test") != 0, "ls_contains_shell", lslen);
    check(find_substr(lsbuf, "syscall_test") != 0, "ls_contains_test", lslen);
  }

  check(sys_touch(TEST_FILE, str_len(TEST_FILE)) == 0, "touch", 0);
  check(sys_write_file(TEST_FILE, str_len(TEST_FILE), "alpha", 5) == 0, "write_file", 0);
  check(sys_push_file(TEST_FILE, str_len(TEST_FILE), "beta", 4) == 0, "push_file", 0);

  int fd = sys_open(TEST_FILE, str_len(TEST_FILE));
  check(fd >= 3, "open", fd);
  if (fd < 3)
    return;

  int fd_at = openat(AT_FDCWD, TEST_FILE, 0);
  check(fd_at >= 3, "openat", fd_at);

  long size = sys_fsize(fd);
  check(size == 9, "fsize", size);

  struct stat st, st_at;
  check(fstat(fd, &st) == 0 && st.st_size == 9, "fstat", st.st_size);
  check(stat(TEST_FILE, &st_at) == 0 && st_at.st_size == 9, "stat_fstatat", st_at.st_size);

  check(lseek(fd, 0, SEEK_SET) == 0, "lseek_set", 0);
  char buf[16];
  long got = read(fd, buf, 5);
  check(got == 5 && str_eq_n(buf, "alpha", 5), "read", got);

  check(lseek(fd, -4, SEEK_END) == 5, "lseek_end", 0);
  got = read(fd, buf, 4);
  check(got == 4 && str_eq_n(buf, "beta", 4), "read_tail", got);

  struct iovec riov[2];
  check(lseek(fd, 0, SEEK_SET) == 0, "lseek_reset", 0);
  char a[3];
  char b[6];
  riov[0].iov_base = a;
  riov[0].iov_len = 2;
  riov[1].iov_base = b;
  riov[1].iov_len = 5;
  long rv = readv(fd, riov, 2);
  check(rv == 7 && str_eq_n(a, "al", 2) && str_eq_n(b, "phabe", 5), "readv", rv);

  int dupfd = dup(fd);
  check(dupfd >= 3, "dup", dupfd);
  check(fcntl(fd, F_GETFD, 0) == 0, "fcntl_getfd", 0);
  check(fcntl(fd, F_SETFD, FD_CLOEXEC) == 0, "fcntl_setfd", 0);
  check(fcntl(fd, F_GETFD, 0) == FD_CLOEXEC, "fcntl_getfd_after_set", 0);
  check(fcntl(fd, F_GETFL, 0) == O_RDONLY, "fcntl_getfl", 0);
  check(fcntl(fd, F_SETFL, O_APPEND | O_NONBLOCK) == 0, "fcntl_setfl", 0);
  check(fcntl(fd, F_GETFL, 0) == (O_RDONLY | O_APPEND | O_NONBLOCK),
        "fcntl_getfl_after_set", 0);
  int fdup_min = fcntl(fd, F_DUPFD, 12);
  check(fdup_min >= 12, "fcntl_dupfd", fdup_min);
  int fdup_ce = fcntl(fd, F_DUPFD_CLOEXEC, 13);
  check(fdup_ce >= 13, "fcntl_dupfd_cloexec", fdup_ce);
  check(fcntl(fdup_ce, F_GETFD, 0) == FD_CLOEXEC, "fcntl_dupfd_ce_flag", 0);
  check(lseek(fd, 0, SEEK_SET) == 0, "dup_lseek_reset", 0);
  got = read(fd, buf, 2);
  check(got == 2 && str_eq_n(buf, "al", 2), "dup_read_primary", got);
  got = read(dupfd, buf, 2);
  check(got == 2 && str_eq_n(buf, "ph", 2), "dup_shared_offset", got);

  int dup2fd = dup2(fd, 10);
  check(dup2fd == 10, "dup2", dup2fd);
  int dup3fd = dup3(fd, 11, 0);
  check(dup3fd == 11, "dup3", dup3fd);

  struct iovec wiov[2];
  wiov[0].iov_base = "writev ";
  wiov[0].iov_len = 7;
  wiov[1].iov_base = "ok\n";
  wiov[1].iov_len = 3;
  check(writev(1, wiov, 2) == 10, "writev", 10);

  check(sys_close(fd_at) == 0, "close_openat", 0);
  check(sys_close(dupfd) == 0, "close_dup", 0);
  check(sys_close(fdup_min) == 0, "close_fcntl_dupfd", 0);
  check(sys_close(fdup_ce) == 0, "close_fcntl_dupfd_ce", 0);
  check(sys_close(dup2fd) == 0, "close_dup2", 0);
  check(sys_close(dup3fd) == 0, "close_dup3", 0);
  check(sys_close(fd) == 0, "close", 0);

  check(sys_rm(TEST_FILE, str_len(TEST_FILE)) == 0, "rm", 0);
}

static void test_processes(void) {
  int pid = fork();
  if (pid == 0) {
    puts("fork child ran\n");
    sys_exit(42);
  }
  check(pid > 0, "fork_parent", pid);
  if (pid > 0) {
    long code = sys_waitpid(pid);
    check(code == 42, "waitpid_fork", code);
  }

  long exec_pid = sys_exec(CHILD_FILE, str_len(CHILD_FILE));
  check(exec_pid > 0, "exec", exec_pid);
  if (exec_pid > 0) {
    long code = sys_waitpid(exec_pid);
    check(code == 7, "waitpid_exec", code);
  }
}

static void test_pipe(void) {
  int pfds[2];
  check(pipe(pfds) == 0, "pipe", 0);
  if (pfds[0] < 0 || pfds[1] < 0)
    return;

  struct stat st;
  check(fstat(pfds[0], &st) == 0 && ((st.st_mode & 0170000) == 0010000),
        "pipe_fstat", st.st_mode);

  const char *msg = "pipe-data";
  check(write(pfds[1], msg, 9) == 9, "pipe_write", 0);
  char buf[16];
  long got = read(pfds[0], buf, 9);
  check(got == 9 && str_eq_n(buf, msg, 9), "pipe_read", got);

  int dupw = dup(pfds[1]);
  check(dupw >= 3, "pipe_dup_write", dupw);
  check(write(dupw, "xy", 2) == 2, "pipe_dup_write_data", 0);
  got = read(pfds[0], buf, 2);
  check(got == 2 && str_eq_n(buf, "xy", 2), "pipe_dup_read_data", got);

  check(sys_close(dupw) == 0, "pipe_close_dup_write", 0);
  check(sys_close(pfds[1]) == 0, "pipe_close_write", 0);
  got = read(pfds[0], buf, sizeof(buf));
  check(got == 0, "pipe_eof_after_close", got);
  check(sys_close(pfds[0]) == 0, "pipe_close_read", 0);
}

int main(void) {
  puts("syscall test start\n");
  check(write(1, "write ok\n", 9) == 9, "write", 9);

  sys_clear();
  sys_set_font_scale(2);
  sys_set_font_scale(1);
  pass("clear_font");

  test_basic_info();
  test_brk_and_vm();
  test_fs_and_fd();
  test_pipe();
  test_processes();

  skip("read_stdin_blocking");
  skip("panic_destructive");
  skip("reboot_destructive");

  if (failures == 0) {
    puts("syscall test result PASS\n");
    sys_exit(0);
  }
  puts("syscall test result FAIL\n");
  sys_exit(1);
  return 0;
}
