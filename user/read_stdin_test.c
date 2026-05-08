#include "libcshim/libcshim.h"

static void put_uint(unsigned long value) {
  char tmp[32];
  int i = 0;

  if (value == 0) {
    putchar('0');
    return;
  }

  while (value > 0) {
    tmp[i++] = (char)('0' + (value % 10));
    value /= 10;
  }

  while (i > 0) {
    putchar(tmp[--i]);
  }
}

int main(void) {
  char buf[128];

  puts("stdin read syscall test\n");
  puts("type a line and press Enter:\n");

  long n = sys_read(0, buf, sizeof(buf) - 1);
  if (n < 0) {
    puts("read failed\n");
    return 1;
  }

  buf[n] = '\0';
  puts("read returned ");
  put_uint((unsigned long)n);
  puts(" bytes\n");
  puts("buffer: ");
  sys_write(1, buf, (size_t)n);
  if (n == 0 || buf[n - 1] != '\n') {
    puts("\n");
  }

  return 0;
}
