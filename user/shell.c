// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only
//
// CoreOS userspace shell.
// Compiled with clang -ffreestanding, linked against libcshim + start.asm.

#include "libcshim/libcshim.h"

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#define STDOUT 1

static void print(const char *s) { puts_fd(STDOUT, s); }

static void println(const char *s) {
  print(s);
  print("\n");
}

// Print a decimal integer
static void print_int(long n) {
  if (n < 0) {
    putchar('-');
    n = -n;
  }
  if (n == 0) {
    putchar('0');
    return;
  }
  char buf[20];
  int i = 0;
  while (n > 0) {
    buf[i++] = '0' + (n % 10);
    n /= 10;
  }
  while (i--)
    putchar(buf[i]); // NB: i is decremented THEN used
}

// Skip leading whitespace (spaces and carriage returns); return pointer to
// first non-space
static const char *skip_spaces(const char *s) {
  while (*s == ' ' || *s == '\r')
    s++;
  return s;
}

// Return pointer to first space or end of string
static const char *next_space(const char *s) {
  while (*s && *s != ' ')
    s++;
  return s;
}

// Copy at most `n-1` chars from src to dst, null-terminate.
static void strncpy_s(char *dst, const char *src, int n) {
  int i = 0;
  while (i < n - 1 && src[i]) {
    dst[i] = src[i];
    i++;
  }
  dst[i] = '\0';
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

#define CMD_BUF 128
#define ARG_BUF 64

// help
static void cmd_help(void) {
  println("Available commands:");
  println("  help          - show this message");
  println("  echo <text>   - print text");
  println("  ls            - list RamFS files");
  println("  cat <file>    - print file contents");
  println("  touch <name>  - create empty file");
  println("  rm <name>     - delete file");
  println("  write <f> <t> - overwrite file");
  println("  push <f> <t>  - append to file");
  println("  exec <file>   - run ELF from RamFS");
  println("  meminfo       - show free memory");
  println("  uptime        - system uptime");
  println("  ticks         - kernel ticks");
  println("  sleep <sec>   - sleep for N seconds");
  println("  font <+/->    - change font size");
  println("  boottime      - show boot timing");
  println("  clear         - clear terminal");
  println("  panic         - trigger kernel panic");
  println("  reboot        - reboot system");
  println("  exit [code]   - exit the shell");
}

static void cmd_meminfo(void) {
  long bytes = sys_meminfo();
  print("Free RAM: ");
  print_int(bytes);
  println(" bytes");

  long mb = bytes >> 20;
  print("Approx:   ");
  print_int(mb);
  println(" MB");
}

static void cmd_uptime(void) {
  long s = sys_uptime();
  print("Uptime: ");
  print_int(s);
  println(" seconds");
}

static void cmd_ticks(void) {
  long t = sys_ticks();
  print("Kernel ticks: ");
  print_int(t);
  print("\n");
}

static void cmd_sleep(const char *arg) {
  long n = 0;
  while (*arg >= '0' && *arg <= '9') {
    n = n * 10 + (*arg - '0');
    arg++;
  }
  if (n > 0)
    sys_sleep(n * 100); // 100 ticks per second
}

static void cmd_font(const char *arg) {
  if (*arg == '+') {
    sys_set_font_scale(2); // for now just toggle or set
  } else if (*arg == '-') {
    sys_set_font_scale(1);
  }
}

static void cmd_boottime(void) {
  char buf[2048];
  long n = sys_boottime(buf, sizeof(buf) - 1);
  if (n > 0) {
    buf[n] = '\0';
    print(buf);
  }
}

// echo
static void cmd_echo(const char *args) { println(args); }

// ls — open each named file; we use a probe strategy since we have no
// directory syscall yet: try names by opening them. Instead, we use a
// side-channel: open() with an empty name returns MAX on first miss and
// we can't iterate. For now we rely on a convention: the kernel pre-loads
// "test" into RamFS. We list what we can open from a known set, plus
// anything the kernel tells us via a special fd=255 trick.
//
// Better approach: add a sys_ls syscall later. For now, just try "test"
// and a few other well-known names.
static void cmd_ls(void) {
  char buf[1024];
  long total = sys_ls(buf, sizeof(buf));
  if (total <= 0) {
    puts("(empty)\n");
    return;
  }
  sys_write(STDOUT, buf, (size_t)total);
  if (total == 0 || buf[total - 1] != '\n') {
    puts("\n");
  }
}

static void cmd_touch(const char *arg) {
  if (!*arg) {
    puts("usage: touch <name>\n");
    return;
  }
  int fd = open(arg, O_CREAT | O_EXCL | O_RDONLY, 0644);
  if (fd >= 0 && sys_close(fd) == 0)
    puts("ok\n");
  else
    puts("error: could not create file\n");
}

static void cmd_rm(const char *arg) {
  if (!*arg) {
    puts("usage: rm <name>\n");
    return;
  }
  if (unlinkat(AT_FDCWD, arg, 0) == 0)
    puts("ok\n");
  else
    puts("error: file not found\n");
}

static void cmd_write(const char *arg) {
  // usage: write <name> <content>
  char name[64];
  int i = 0;
  while (*arg && *arg != ' ' && i < 63)
    name[i++] = *arg++;
  name[i] = '\0';
  if (*arg == ' ')
    arg++;
  if (!name[0]) {
    puts("usage: write <name> <content>\n");
    return;
  }
  int fd = open(name, O_CREAT | O_TRUNC | O_WRONLY, 0644);
  if (fd >= 0 && write(fd, arg, str_len(arg)) == (ssize_t)str_len(arg) &&
      sys_close(fd) == 0)
    puts("ok\n");
  else
    puts("error\n");
}

static void cmd_push(const char *arg) {
  // usage: push <name> <content>
  char name[64];
  int i = 0;
  while (*arg && *arg != ' ' && i < 63)
    name[i++] = *arg++;
  name[i] = '\0';
  if (*arg == ' ')
    arg++;
  if (!name[0]) {
    puts("usage: push <name> <content>\n");
    return;
  }
  int fd = open(name, O_CREAT | O_APPEND | O_WRONLY, 0644);
  if (fd >= 0 && write(fd, arg, str_len(arg)) == (ssize_t)str_len(arg) &&
      sys_close(fd) == 0)
    puts("ok\n");
  else
    puts("error\n");
}

// cat
static void cmd_cat(const char *filename) {
  if (!filename || !*filename) {
    println("usage: cat <file>");
    return;
  }
  int fd = sys_open(filename, O_RDONLY, 0);
  if (fd == -1 || (long)fd == -1L) {
    print("cat: file not found: ");
    println(filename);
    return;
  }
  long size = sys_fsize(fd);
  if (size <= 0) {
    sys_close(fd);
    println("(empty file)");
    return;
  }

  // Read in chunks of 256 bytes and write to stdout
  char buf[256];
  long remaining = size;
  while (remaining > 0) {
    long chunk = remaining < 256 ? remaining : 256;
    long got = sys_read(fd, buf, (size_t)chunk);
    if (got <= 0)
      break;
    sys_write(STDOUT, buf, (size_t)got);
    remaining -= got;
  }
  print("\n");
  sys_close(fd);
}

// exec
static void cmd_exec(const char *filename) {
  if (!filename || !*filename) {
    println("usage: exec <file>");
    return;
  }
  print("exec: spawning ");
  println(filename);

  long pid = sys_exec(filename);
  if (pid == 0) {
    println("exec: failed to spawn process");
    return;
  }
  print("exec: pid=");
  print_int(pid);
  print("\n");

  long code = sys_waitpid(pid);
  print("exec: process exited with code ");
  print_int(code);
  print("\n");
}

// ---------------------------------------------------------------------------
// Parse and dispatch a command line
// ---------------------------------------------------------------------------

static void dispatch(const char *line) {
  // Skip leading whitespace
  line = skip_spaces(line);
  if (!*line)
    return;

  // Find end of first word (the command)
  const char *cmd_end = next_space(line);
  int cmd_len = (int)(cmd_end - line);

  // Args start after the command and any spaces
  const char *args = skip_spaces(cmd_end);

  // Copy command word for comparison
  char cmd[32];
  strncpy_s(cmd, line, cmd_len + 1 < 32 ? cmd_len + 1 : 32);
  cmd[cmd_len] = '\0';

  if (str_eq(cmd, "help")) {
    cmd_help();
  } else if (str_eq(cmd, "echo")) {
    cmd_echo(args);
  } else if (str_eq(cmd, "ls")) {
    cmd_ls();
  } else if (str_eq(cmd, "touch")) {
    cmd_touch(args);
  } else if (str_eq(cmd, "rm")) {
    cmd_rm(args);
  } else if (str_eq(cmd, "write")) {
    cmd_write(args);
  } else if (str_eq(cmd, "push")) {
    cmd_push(args);
  } else if (str_eq(cmd, "cat")) {
    cmd_cat(args);
  } else if (str_eq(cmd, "exec")) {
    char arg[ARG_BUF];
    const char *arg_end = next_space(args);
    size_t len = (size_t)(arg_end - args);
    if (len >= ARG_BUF)
      len = ARG_BUF - 1;
    strncpy_s(arg, args, (int)len + 1); // correct: (dst, src, n)
    arg[len] = '\0';
    cmd_exec(arg);
  } else if (str_eq(cmd, "meminfo")) {
    cmd_meminfo();
  } else if (str_eq(cmd, "uptime")) {
    cmd_uptime();
  } else if (str_eq(cmd, "ticks")) {
    cmd_ticks();
  } else if (str_eq(cmd, "sleep")) {
    cmd_sleep(args);
  } else if (str_eq(cmd, "font")) {
    cmd_font(args);
  } else if (str_eq(cmd, "boottime")) {
    cmd_boottime();
  } else if (str_eq(cmd, "clear")) {
    sys_clear();
  } else if (str_eq(cmd, "panic")) {
    sys_panic();
  } else if (str_eq(cmd, "reboot")) {
    sys_reboot();
  } else if (str_eq(cmd, "exit")) {
    int code = 0;
    while (*args >= '0' && *args <= '9') {
      code = code * 10 + (*args - '0');
      args++;
    }
    sys_exit(code);
  } else {
    print("unknown command: ");
    println(cmd);
    println("type 'help' for available commands");
  }
}

// ---------------------------------------------------------------------------
// Main shell loop
// ---------------------------------------------------------------------------

int main(void) {
  println("CoreOS userspace shell");
  println("type 'help' for commands\n");

  char line[CMD_BUF];
  while (1) {
    print("> ");
    readline(line, CMD_BUF);
    dispatch(line);
  }
  return 0;
}
