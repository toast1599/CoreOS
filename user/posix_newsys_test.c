#include "libcshim/libcshim.h"

static int failures = 0;
static volatile int signal_hits = 0;
static volatile int signal_value = 0;
static volatile int thread_shared_flag = 0;
static volatile int thread_parent_tid = -1;
static volatile int thread_child_tid = -1;
static volatile int thread_signal_hits = 0;
static volatile int thread_signal_tid = -1;
static volatile int thread_signal_ready = 0;
static volatile int thread_signal_exit = 0;
static volatile unsigned long thread_tls_word = 0xfeedfacecafebeefUL;
static unsigned char signal_stack[2048];

__attribute__((naked)) static void signal_restorer(void) {
  __asm__ volatile(
      "mov %[nr], %%rax\n\t"
      "syscall\n\t"
      :
      : [nr] "i"(COREOS_SYS_RT_SIGRETURN)
      : "rax", "rcx", "r11", "memory");
}

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

static void usr1_handler(int sig) {
  signal_hits++;
  signal_value = sig;
  thread_signal_hits++;
  thread_signal_tid = gettid();
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

static void test_signals(void) {
  stack_t ss = {(void *)signal_stack, 0, sizeof(signal_stack)};
  stack_t old_ss;
  check(sigaltstack(&ss, &old_ss) == 0, "sigaltstack_set", old_ss.ss_flags);

  stack_t cur_ss;
  check(sigaltstack(0, &cur_ss) == 0 &&
            cur_ss.ss_sp == (void *)signal_stack &&
            cur_ss.ss_size == sizeof(signal_stack) &&
            (cur_ss.ss_flags & SS_DISABLE) == 0,
        "sigaltstack_query", cur_ss.ss_flags);

  struct sigaction act;
  memset(&act, 0, sizeof(act));
  act.sa_handler = usr1_handler;
  act.sa_flags = SA_RESTORER | SA_ONSTACK;
  act.sa_restorer = signal_restorer;
  sigemptyset(&act.sa_mask);
  check(sigaction(SIGUSR1, &act, 0) == 0, "sigaction_usr1", 0);

  sigset_t block;
  sigemptyset(&block);
  sigaddset(&block, SIGUSR1);
  signal_hits = 0;
  signal_value = 0;
  check(sigprocmask(SIG_BLOCK, &block, 0) == 0, "sigprocmask_block", 0);
  check(kill(getpid(), SIGUSR1) == 0, "kill_usr1", 0);
  check(getpid() > 0 && signal_hits == 0, "signal_blocked", signal_hits);
  check(sigprocmask(SIG_UNBLOCK, &block, 0) == 0, "sigprocmask_unblock", 0);
  check(getpid() > 0 && signal_hits == 1 && signal_value == SIGUSR1,
        "signal_delivered", signal_value);
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

  int fd = sys_open(name, O_RDWR, 0);
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

static void test_open_write_flags(void) {
  const char *name = "posix_open_flags";
  int fd = openat(AT_FDCWD, name, O_CREAT | O_TRUNC | O_RDWR, 0644);
  check(fd >= 3, "openat_create_rdwr", fd);
  if (fd < 3)
    return;

  check(write(fd, "abc", 3) == 3, "write_regular", 0);
  check(lseek(fd, 0, SEEK_SET) == 0, "lseek_regular_reset", 0);

  char buf[8] = {0};
  check(read(fd, buf, 3) == 3 && str_eq_n(buf, "abc", 3), "read_regular", buf[0]);
  check(sys_close(fd) == 0, "close_regular_rdwr", 0);

  int append_fd = open(name, O_WRONLY | O_APPEND, 0);
  check(append_fd >= 3, "open_append", append_fd);
  if (append_fd >= 3) {
    check(write(append_fd, "def", 3) == 3, "write_append", 0);
    check(read(append_fd, buf, 1) == -1, "read_writeonly_fails", 0);
    check(sys_close(append_fd) == 0, "close_append", 0);
  }

  int rdonly_fd = open(name, O_RDONLY, 0);
  check(rdonly_fd >= 3, "open_rdonly", rdonly_fd);
  if (rdonly_fd >= 3) {
    char verify[8] = {0};
    check(read(rdonly_fd, verify, 6) == 6 && str_eq_n(verify, "abcdef", 6),
          "append_verify", verify[0]);
    check(write(rdonly_fd, "!", 1) == -1, "write_rdonly_fails", 0);
    check(sys_close(rdonly_fd) == 0, "close_rdonly", 0);
  }

  check(open(name, O_CREAT | O_EXCL | O_RDONLY, 0644) == -1, "open_excl_exists", 0);
  check(unlinkat(AT_FDCWD, name, 0) == 0, "unlink_open_flags", 0);
}

static void test_clone_raw(void) {
  int pid = sys_clone(SIGCHLD, 0, 0, 0, 0);
  if (pid == 0) {
    sys_exit(41);
  }
  check(pid > 0, "clone_parent", pid);
  if (pid > 0) {
    check(sys_waitpid(pid) == 41, "clone_waitpid", pid);
  }
}

static void test_clone_thread_shared(void) {
  char *stack = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  check(stack != MAP_FAILED, "clone_thread_stack", 0);
  if (stack == MAP_FAILED)
    return;

  thread_shared_flag = 0;
  thread_parent_tid = -1;
  thread_child_tid = -1;
  int pid = getpid();
  void *stack_top = stack + 8192 - 16;
  unsigned long flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND |
                        CLONE_THREAD | CLONE_SETTLS | CLONE_PARENT_SETTID |
                        CLONE_CHILD_CLEARTID | CLONE_CHILD_SETTID;
  int tid = sys_clone(flags, stack_top, (int *)&thread_parent_tid,
                      (int *)&thread_child_tid, (unsigned long)&thread_tls_word);
  if (tid == 0) {
    unsigned long fs_word = read_fs_qword0();
    if (getpid() == pid && gettid() != pid && fs_word == thread_tls_word) {
      thread_shared_flag = 1;
    }
    sys_exit(0);
  }

  check(tid > 0, "clone_thread_parent", tid);
  if (tid > 0) {
    check(thread_parent_tid == tid, "clone_thread_parent_tid", thread_parent_tid);
    check(thread_child_tid == tid, "clone_thread_child_tid_init", thread_child_tid);
    while (thread_child_tid != 0) {
      sys_futex((unsigned int *)&thread_child_tid, FUTEX_WAIT, (unsigned int)tid, 0);
    }
    check(thread_shared_flag == 1, "clone_thread_shared_vm", thread_shared_flag);
    check(thread_child_tid == 0, "clone_thread_child_tid_clear", thread_child_tid);
  }

  check(munmap(stack, 8192) == 0, "clone_thread_unmap", 0);
}

struct robust_mutex_node {
  struct robust_list list;
  unsigned int futex_word;
};

static struct robust_list_head robust_head;
static struct robust_mutex_node robust_node;
static volatile int robust_child_tid = -1;
static volatile int robust_ready = 0;

static void test_tgkill_thread_signal(void) {
  char *stack = mmap(0, 8192, PROT_READ | PROT_WRITE,
                     MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  check(stack != MAP_FAILED, "tgkill_stack", 0);
  if (stack == MAP_FAILED)
    return;

  thread_signal_hits = 0;
  thread_signal_tid = -1;
  thread_signal_ready = 0;
  thread_signal_exit = 0;
  thread_child_tid = -1;

  unsigned long flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND |
                        CLONE_THREAD | CLONE_CHILD_CLEARTID |
                        CLONE_CHILD_SETTID;
  int tgid = getpid();
  int tid = sys_clone(flags, stack + 8192 - 16, 0, (int *)&thread_child_tid, 0);
  if (tid == 0) {
    thread_signal_ready = 1;
    while (!thread_signal_exit) {
      (void)getpid();
    }
    sys_exit(0);
  }

  check(tid > 0, "tgkill_clone", tid);
  if (tid > 0) {
    while (!thread_signal_ready) {
      nanosleep(&(struct timespec){0, 1000000L}, 0);
    }
    check(sys_tgkill(tgid, tid, SIGUSR1) == 0, "tgkill_send", tid);
    for (int i = 0; i < 100 && thread_signal_hits == 0; i++) {
      nanosleep(&(struct timespec){0, 1000000L}, 0);
    }
    check(thread_signal_hits > 0, "tgkill_delivered", thread_signal_hits);
    check(thread_signal_tid == tid, "tgkill_target_tid", thread_signal_tid);
    thread_signal_exit = 1;
    while (thread_child_tid != 0) {
      sys_futex((unsigned int *)&thread_child_tid, FUTEX_WAIT,
                (unsigned int)tid, 0);
    }
  }

  check(munmap(stack, 8192) == 0, "tgkill_unmap", 0);
}

static void test_robust_list_exit(void) {
  char *stack = mmap(0, 8192, PROT_READ | PROT_WRITE,
                     MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  check(stack != MAP_FAILED, "robust_stack", 0);
  if (stack == MAP_FAILED)
    return;

  robust_child_tid = -1;
  robust_ready = 0;
  memset(&robust_head, 0, sizeof(robust_head));
  memset(&robust_node, 0, sizeof(robust_node));
  robust_head.list.next = &robust_head.list;
  robust_head.futex_offset =
      (long)((char *)&robust_node.futex_word - (char *)&robust_node.list);
  robust_head.list_op_pending = 0;

  unsigned long flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND |
                        CLONE_THREAD | CLONE_CHILD_CLEARTID |
                        CLONE_CHILD_SETTID;
  int tid = sys_clone(flags, stack + 8192 - 16, 0, (int *)&robust_child_tid, 0);
  if (tid == 0) {
    struct robust_list_head *head_ptr = 0;
    size_t head_len = 0;
    sys_set_robust_list(&robust_head, sizeof(robust_head));
    check(sys_get_robust_list(0, &head_ptr, &head_len) == 0 &&
              head_ptr == &robust_head &&
              head_len == sizeof(robust_head),
          "robust_get_self", head_len);
    robust_node.futex_word = (unsigned int)gettid();
    robust_node.list.next = &robust_head.list;
    robust_head.list.next = &robust_node.list;
    robust_ready = 1;
    sys_exit(0);
  }

  check(tid > 0, "robust_clone", tid);
  if (tid > 0) {
    while (!robust_ready) {
      nanosleep(&(struct timespec){0, 1000000L}, 0);
    }
    while (robust_child_tid != 0) {
      sys_futex((unsigned int *)&robust_child_tid, FUTEX_WAIT,
                (unsigned int)tid, 0);
    }
    check((robust_node.futex_word & FUTEX_OWNER_DIED) != 0, "robust_owner_died",
          robust_node.futex_word);
    check((robust_node.futex_word & 0x3fffffffU) == 0, "robust_owner_cleared",
          robust_node.futex_word);
  }

  check(munmap(stack, 8192) == 0, "robust_unmap", 0);
}

static void test_futex_basics(void) {
  unsigned int word = 2;
  check(sys_futex(&word, FUTEX_WAIT, 1, 0) == -1, "futex_wait_mismatch", word);
  check(sys_futex(&word, FUTEX_WAKE, 1, 0) == 0, "futex_wake_empty", word);
}

static void test_file_backed_mmap(void) {
  const char *name = "posix_mmap_file";
  int fd = openat(AT_FDCWD, name, O_CREAT | O_TRUNC | O_RDWR, 0644);
  check(fd >= 3, "mmap_file_open", fd);
  if (fd < 3)
    return;

  check(write(fd, "mmap-data", 9) == 9, "mmap_file_seed", 0);
  char *map = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
  check(map != MAP_FAILED, "mmap_file_map", 0);
  if (map != MAP_FAILED) {
    check(str_eq_n(map, "mmap-data", 9), "mmap_file_bytes", map[0]);
    map[0] = 'Z';
    char verify[10] = {0};
    check(pread(fd, verify, 9, 0) == 9 && str_eq_n(verify, "mmap-data", 9),
          "mmap_file_private", verify[0]);
    check(munmap(map, 4096) == 0, "mmap_file_unmap", 0);
  }

  check(sys_close(fd) == 0, "mmap_file_close", 0);
  check(unlinkat(AT_FDCWD, name, 0) == 0, "mmap_file_unlink", 0);
}

static void test_sendfile(void) {
  const char *name = "posix_sendfile_probe";
  check(sys_touch(name, str_len(name)) == 0, "touch_sendfile_probe", 0);
  check(sys_write_file(name, str_len(name), "sendfile-data", 13) == 0,
        "seed_sendfile_probe", 0);

  int fd = sys_open(name, O_RDWR, 0);
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
  int fd = sys_open(name, O_RDWR, 0);
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
  int fd = sys_open(name, O_RDWR, 0);
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
  test_signals();
  test_unlinkat();
  test_exit_group();
  test_open_write_flags();
  test_clone_raw();
  test_clone_thread_shared();
  test_tgkill_thread_signal();
  test_futex_basics();
  test_robust_list_exit();
  test_pread_pwrite();
  test_sendfile();
  test_file_backed_mmap();
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
