#include "libcshim/libcshim.h"

static int failures = 0;

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

static void check(int cond, const char *name, long detail) {
  if (cond)
    pass(name);
  else
    fail(name, detail);
}

static int str_eq_n(const char *a, const char *b, size_t n) {
  for (size_t i = 0; i < n; i++) {
    if (a[i] != b[i])
      return 0;
  }
  return 1;
}

static unsigned long read_fs_qword0(void) {
  unsigned long value;
  __asm__ volatile("mov %%fs:0, %0" : "=r"(value));
  return value;
}

static void test_arch_prctl(void) {
  unsigned long tls_word = 0x0123456789abcdefUL;
  unsigned long tls_base = 0;

  check(arch_prctl(ARCH_SET_FS, (unsigned long)&tls_word) == 0,
        "arch_prctl_set_fs", 0);
  check(arch_prctl(ARCH_GET_FS, (unsigned long)&tls_base) == 0 &&
            tls_base == (unsigned long)&tls_word,
        "arch_prctl_get_fs", tls_base);
  check(read_fs_qword0() == tls_word, "arch_prctl_fs_read",
        (long)read_fs_qword0());
  check(getpid() > 0 && read_fs_qword0() == tls_word,
        "arch_prctl_fs_persist", (long)read_fs_qword0());
  check(arch_prctl(ARCH_SET_FS, 0) == 0, "arch_prctl_reset_fs", 0);
}

static void test_clock_nanosleep(void) {
  struct timespec t0, target, t1;
  int rc0 = clock_gettime(CLOCK_MONOTONIC, &t0);
  target = t0;
  target.tv_nsec += 20 * 1000 * 1000L;
  if (target.tv_nsec >= 1000000000L) {
    target.tv_sec += 1;
    target.tv_nsec -= 1000000000L;
  }

  int rc1 = clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, 0);
  int rc2 = clock_gettime(CLOCK_MONOTONIC, &t1);
  long delta_ns =
      (t1.tv_sec - t0.tv_sec) * 1000000000L + (t1.tv_nsec - t0.tv_nsec);
  check(rc0 == 0 && rc1 == 0 && rc2 == 0 && delta_ns >= 10 * 1000 * 1000L,
        "clock_nanosleep", delta_ns);
}

static void test_getrandom(void) {
  unsigned char buf[16] = {0};
  long rc = getrandom(buf, sizeof(buf), 0);
  int nonzero = 0;
  for (size_t i = 0; i < sizeof(buf); i++) {
    if (buf[i] != 0) {
      nonzero = 1;
      break;
    }
  }
  check(rc == (long)sizeof(buf) && nonzero, "getrandom", rc);
}

static void test_unlinkat(void) {
  const char *name = "posix_newsys_probe";
  check(sys_touch(name, str_len(name)) == 0, "touch_probe", 0);
  check(unlinkat(AT_FDCWD, name, 0) == 0, "unlinkat", 0);
  check(faccessat(AT_FDCWD, name, F_OK, 0) == -1, "unlinkat_gone", 0);
}

static void test_exit_group(void) {
  int pid = fork();
  if (pid == 0) {
    sys_exit_group(23);
  }
  check(pid > 0, "fork_exit_group_parent", pid);
  if (pid > 0) {
    long code = sys_waitpid(pid);
    check(code == 23, "waitpid_exit_group", code);
  }
}

static void test_pread_pwrite(void) {
  const char *name = "posix_rw_probe";
  check(sys_touch(name, str_len(name)) == 0, "touch_rw_probe", 0);
  check(sys_write_file(name, str_len(name), "abcdef", 6) == 0,
        "seed_rw_probe", 0);

  int fd = sys_open(name, str_len(name));
  check(fd >= 3, "open_rw_probe", fd);
  if (fd < 3)
    return;

  char read_buf[4] = {0};
  ssize_t pr = pread(fd, read_buf, 3, 2);
  check(pr == 3 && str_eq_n(read_buf, "cde", 3), "pread64", pr);

  ssize_t pw = pwrite(fd, "XYZ", 3, 1);
  check(pw == 3, "pwrite64", pw);

  char verify[8] = {0};
  ssize_t vr = pread(fd, verify, 6, 0);
  check(vr == 6 && str_eq_n(verify, "aXYZef", 6), "pwrite64_verify", vr);

  check(sys_close(fd) == 0, "close_rw_probe", 0);
  check(sys_rm(name, str_len(name)) == 0, "rm_rw_probe", 0);
}

static void test_sendfile(void) {
  const char *name = "posix_sendfile_probe";
  check(sys_touch(name, str_len(name)) == 0, "touch_sendfile_probe", 0);
  check(sys_write_file(name, str_len(name), "sendfile-data", 13) == 0,
        "seed_sendfile_probe", 0);

  int fd = sys_open(name, str_len(name));
  check(fd >= 3, "open_sendfile_probe", fd);
  if (fd < 3)
    return;

  int pfds[2];
  check(pipe(pfds) == 0, "pipe_sendfile", 0);
  if (pfds[0] < 0 || pfds[1] < 0)
    return;

  off_t off = 4;
  ssize_t sf = sendfile(pfds[1], fd, &off, 4);
  check(sf == 4 && off == 8, "sendfile", sf);

  char buf[8] = {0};
  ssize_t got = read(pfds[0], buf, 4);
  check(got == 4 && str_eq_n(buf, "file", 4), "sendfile_readback", got);

  check(sys_close(pfds[0]) == 0, "close_sendfile_r", 0);
  check(sys_close(pfds[1]) == 0, "close_sendfile_w", 0);
  check(sys_close(fd) == 0, "close_sendfile_fd", 0);
  check(sys_rm(name, str_len(name)) == 0, "rm_sendfile_probe", 0);
}

static void test_readlink(void) {
  char buf[64];
  ssize_t n = readlink("/proc/self/exe", buf, sizeof(buf));
  check(n > 0 && n < (ssize_t)sizeof(buf), "readlink", n);
  if (n > 0 && n < (ssize_t)sizeof(buf)) {
    check(str_eq_n(buf, "/posix_newsys_test", (size_t)n), "readlink_target", n);
  }

  char buf2[64];
  ssize_t n2 = readlinkat(AT_FDCWD, "/proc/self/exe", buf2, sizeof(buf2));
  check(n2 == n && str_eq_n(buf2, buf, (size_t)n2), "readlinkat", n2);
}

static void test_truncate_ops(void) {
  const char *name = "posix_truncate_probe";
  check(sys_touch(name, str_len(name)) == 0, "touch_truncate_probe", 0);
  check(sys_write_file(name, str_len(name), "truncate-me", 11) == 0,
        "seed_truncate_probe", 0);

  check(truncate(name, 0) == 0, "truncate", 0);
  int fd = sys_open(name, str_len(name));
  check(fd >= 3, "open_truncate_probe", fd);
  if (fd < 3)
    return;

  struct stat st;
  check(fstat(fd, &st) == 0 && st.st_size == 0, "truncate_verify", st.st_size);
  check(pwrite(fd, "abcd", 4, 0) == 4, "truncate_reseed", 0);
  check(ftruncate(fd, 2) == 0, "ftruncate", 0);
  check(fstat(fd, &st) == 0 && st.st_size == 2, "ftruncate_verify", st.st_size);

  check(sys_close(fd) == 0, "close_truncate_probe", 0);
  check(sys_rm(name, str_len(name)) == 0, "rm_truncate_probe", 0);
}

static void test_gettimeofday_call(void) {
  struct timeval tv0, tv1;
  int rc0 = gettimeofday(&tv0, 0);
  struct timespec req = {0, 10 * 1000 * 1000L};
  nanosleep(&req, 0);
  int rc1 = gettimeofday(&tv1, 0);
  long delta_us =
      (tv1.tv_sec - tv0.tv_sec) * 1000000L + (tv1.tv_usec - tv0.tv_usec);
  check(rc0 == 0 && rc1 == 0 && delta_us >= 5000, "gettimeofday", delta_us);
}

static void test_preadv_pwritev(void) {
  const char *name = "posix_iov_probe";
  check(sys_touch(name, str_len(name)) == 0, "touch_iov_probe", 0);
  check(sys_write_file(name, str_len(name), "0123456789", 10) == 0,
        "seed_iov_probe", 0);
  int fd = sys_open(name, str_len(name));
  check(fd >= 3, "open_iov_probe", fd);
  if (fd < 3)
    return;

  char a[3] = {0};
  char b[4] = {0};
  struct iovec riov[2];
  riov[0].iov_base = a;
  riov[0].iov_len = 2;
  riov[1].iov_base = b;
  riov[1].iov_len = 3;
  ssize_t pr = preadv(fd, riov, 2, 3);
  check(pr == 5 && str_eq_n(a, "34", 2) && str_eq_n(b, "567", 3), "preadv", pr);

  struct iovec wiov[2];
  wiov[0].iov_base = "AA";
  wiov[0].iov_len = 2;
  wiov[1].iov_base = "BBB";
  wiov[1].iov_len = 3;
  ssize_t pw = pwritev(fd, wiov, 2, 1);
  check(pw == 5, "pwritev", pw);

  char verify[8] = {0};
  ssize_t vr = pread(fd, verify, 6, 0);
  check(vr == 6 && str_eq_n(verify, "0AABBB", 6), "pwritev_verify", vr);

  check(sys_close(fd) == 0, "close_iov_probe", 0);
  check(sys_rm(name, str_len(name)) == 0, "rm_iov_probe", 0);
}

int main(void) {
  puts("posix newsys test start\n");

  test_arch_prctl();
  test_clock_nanosleep();
  test_getrandom();
  test_unlinkat();
  test_exit_group();
  test_pread_pwrite();
  test_sendfile();
  test_readlink();
  test_truncate_ops();
  test_gettimeofday_call();
  test_preadv_pwritev();

  if (failures == 0) {
    puts("posix newsys test result PASS\n");
    sys_exit(0);
  }

  puts("posix newsys test result FAIL\n");
  sys_exit(1);
  return 0;
}
