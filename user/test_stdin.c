#include <stdio.h>
#include <unistd.h>

int main() {
  char buf[64];
  // Write directly to stdout to avoid buffered IO issues for now
  write(1, "Type something: ", 16);

  // This should now trigger your read_from_fd -> read_stdin logic
  int n = read(0, buf, 64);

  if (n > 0) {
    write(1, "\nReceived: ", 11);
    write(1, buf, n);
    write(1, "\n", 1);
  } else {
    write(1, "\nRead failed or empty.\n", 23);
  }

  return 0;
}
