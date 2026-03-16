#include "libcshim/libcshim.h"

int main(void) {
  puts("syscall_child: exec path ok\n");
  sys_exit(7);
  return 0;
}
