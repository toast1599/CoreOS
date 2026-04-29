#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/utsname.h>
#include <unistd.h>

static int failures = 0;

static void print_line(const char *line) {
  write(STDOUT_FILENO, line, strlen(line));
}

static void check(int cond, const char *name) {
  if (cond) {
    print_line("PASS ");
    print_line(name);
    print_line("\n");
  } else {
    print_line("FAIL ");
    print_line(name);
    print_line("\n");
    failures++;
  }
}

int main(int argc, char **argv) {
  print_line("musl libc smoke start\n");
  check(argc > 0 && argv != NULL && argv[0] != NULL, "argv");

  void *ptr = malloc(1024);
  check(ptr != NULL, "malloc");
  free(ptr);

  int nullfd = open("/dev/null", O_RDWR);
  check(nullfd >= 0, "open_dev_null");
  if (nullfd >= 0) {
    check(write(nullfd, "x", 1) == 1, "write_dev_null");
    char c = 'x';
    check(read(nullfd, &c, 1) == 0, "read_dev_null_eof");
    close(nullfd);
  }

  int zerofd = open("/dev/zero", O_RDONLY);
  check(zerofd >= 0, "open_dev_zero");
  if (zerofd >= 0) {
    unsigned char zeros[8];
    memset(zeros, 0xaa, sizeof(zeros));
    check(read(zerofd, zeros, sizeof(zeros)) == (ssize_t)sizeof(zeros),
          "read_dev_zero");
    int all_zero = 1;
    for (size_t i = 0; i < sizeof(zeros); i++) {
      if (zeros[i] != 0) {
        all_zero = 0;
        break;
      }
    }
    check(all_zero, "dev_zero_payload");
    close(zerofd);
  }

  char exe[64];
  ssize_t exe_len = readlink("/proc/self/exe", exe, sizeof(exe));
  check(exe_len > 0, "readlink_proc_self_exe");
  if (exe_len > 0 && exe_len < (ssize_t)sizeof(exe)) {
    check(strncmp(exe, "/musl_hello", (size_t)exe_len) == 0,
          "proc_self_exe_target");
  }

  int exefd = open("/proc/self/exe", O_RDONLY);
  check(exefd >= 0, "open_proc_self_exe");
  if (exefd >= 0) {
    unsigned char ident[4] = {0};
    check(read(exefd, ident, sizeof(ident)) == 4, "read_proc_self_exe");
    check(ident[0] == 0x7f && ident[1] == 'E' && ident[2] == 'L' &&
              ident[3] == 'F',
          "proc_self_exe_elf");
    close(exefd);
  }

  struct utsname uts;
  check(uname(&uts) == 0, "uname");

  void *map = mmap(NULL, 4096, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  check(map != MAP_FAILED, "mmap_anon");
  if (map != MAP_FAILED) {
    strcpy((char *)map, "mapped");
    check(strcmp((char *)map, "mapped") == 0, "mmap_rw");
    check(munmap(map, 4096) == 0, "munmap");
  }

  int fd = open("/dev/zero", O_RDONLY);
  char buf[8];

  long n = read(fd, buf, sizeof(buf));

  if (n != 8)
    return 1;

  for (int i = 0; i < 8; i++) {
    if (buf[i] != 0)
      return 2;
  }

  if (failures == 0) {
    print_line("musl libc smoke PASS\n");
    return 0;
  }

  print_line("musl libc smoke FAIL\n");
  return 1;
}
