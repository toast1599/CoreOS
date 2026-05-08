#include <unistd.h>

static void write_all(int fd, const char *buf, size_t len) {
  while (len > 0) {
    ssize_t written = write(fd, buf, len);
    if (written <= 0) {
      return;
    }
    buf += written;
    len -= (size_t)written;
  }
}

static void write_cstr(int fd, const char *s) {
  size_t len = 0;
  while (s[len] != '\0') {
    len++;
  }
  write_all(fd, s, len);
}

static void write_u64(int fd, unsigned long long value) {
  char tmp[32];
  size_t i = 0;

  if (value == 0) {
    write_all(fd, "0", 1);
    return;
  }

  while (value > 0) {
    tmp[i++] = (char)('0' + (value % 10));
    value /= 10;
  }

  while (i > 0) {
    i--;
    write_all(fd, &tmp[i], 1);
  }
}

int main(void) {
  char buf[128];

  write_cstr(1, "musl stdin read test\n");
  write_cstr(1, "type a line and press Enter:\n");

  ssize_t n = read(0, buf, sizeof(buf) - 1);
  if (n < 0) {
    write_cstr(2, "read failed\n");
    return 1;
  }

  buf[n] = '\0';
  write_cstr(1, "read returned ");
  write_u64(1, (unsigned long long)n);
  write_cstr(1, " bytes\n");
  write_cstr(1, "buffer: ");
  write_all(1, buf, (size_t)n);
  if (n == 0 || buf[n - 1] != '\n') {
    write_cstr(1, "\n");
  }
  return 0;
}
