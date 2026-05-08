// Copyright (c) 2026 toast1599
// SPDX-License-Identifier: GPL-3.0-only

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define LINE_BUF 256
#define MAX_ARGS 16
#define MAX_ENV 16
#define NAME_LEN 32
#define VALUE_LEN 128

#define SYS_PCI_LIST 1067
#define SYS_PCI_TEST 1068

/* Raw syscall interface – self-contained, no external dependencies */
static long syscall0(long nr) {
  long ret;
  __asm__ volatile("syscall" : "=a"(ret) : "0"(nr) : "rcx", "r11", "memory");
  return ret;
}

static long syscall1(long nr, long a1) {
  long ret;
  register long r10 __asm__("r10");
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "0"(nr), "D"(a1)
                   : "rcx", "r11", "memory");
  return ret;
}

static long syscall2(long nr, long a1, long a2) {
  long ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "0"(nr), "D"(a1), "S"(a2)
                   : "rcx", "r11", "memory");
  return ret;
}

static long sys_pci_list(char *buf, size_t len) {
  return syscall2(SYS_PCI_LIST, (long)buf, (long)len);
}

static long sys_pci_test(void) { return syscall0(SYS_PCI_TEST); }

struct shell_var {
  int used;
  char name[NAME_LEN];
  char value[VALUE_LEN];
};

static struct shell_var shell_vars[MAX_ENV];
static int last_status = 0;

static int streq(const char *a, const char *b) { return strcmp(a, b) == 0; }

static size_t strlcpy_local(char *dst, const char *src, size_t dst_len) {
  size_t len = strlen(src);
  if (dst_len != 0) {
    size_t copy = len < dst_len - 1 ? len : dst_len - 1;
    memcpy(dst, src, copy);
    dst[copy] = '\0';
  }
  return len;
}

static struct shell_var *find_var(const char *name) {
  int i;
  for (i = 0; i < MAX_ENV; i++) {
    if (shell_vars[i].used && streq(shell_vars[i].name, name)) {
      return &shell_vars[i];
    }
  }
  return NULL;
}

static const char *get_var(const char *name) {
  struct shell_var *var = find_var(name);
  return var ? var->value : "";
}

static void set_var(const char *name, const char *value) {
  struct shell_var *var = find_var(name);
  int i;

  if (var == NULL) {
    for (i = 0; i < MAX_ENV; i++) {
      if (!shell_vars[i].used) {
        var = &shell_vars[i];
        var->used = 1;
        strlcpy_local(var->name, name, sizeof(var->name));
        break;
      }
    }
  }

  if (var != NULL) {
    strlcpy_local(var->value, value, sizeof(var->value));
  }
}

static void unset_var(const char *name) {
  struct shell_var *var = find_var(name);
  if (var != NULL) {
    var->used = 0;
    var->name[0] = '\0';
    var->value[0] = '\0';
  }
}

static void init_shell_vars(void) {
  char cwd[VALUE_LEN];

  set_var("PS1", "$ ");
  set_var("HOME", "/");
  if (getcwd(cwd, sizeof(cwd)) != NULL) {
    set_var("PWD", cwd);
  } else {
    set_var("PWD", "/");
  }
}

static int parse_status(const char *s) {
  int sign = 1;
  int value = 0;

  if (*s == '-') {
    sign = -1;
    s++;
  }
  while (*s >= '0' && *s <= '9') {
    value = value * 10 + (*s - '0');
    s++;
  }
  return sign * value;
}

static ssize_t read_line(int fd, char *buf, size_t len) {
  size_t used = 0;

  if (len == 0) {
    return 0;
  }

  while (used + 1 < len) {
    char c = '\0';
    ssize_t n = read(fd, &c, 1);
    if (n <= 0) {
      break;
    }
    if (c == '\n') {
      break;
    }
    if (c == '\r') {
      continue;
    }
    buf[used++] = c;
  }

  buf[used] = '\0';
  return (ssize_t)used;
}

static void expand_token(char *token, size_t token_len) {
  if (token[0] != '$') {
    return;
  }

  if (streq(token, "$?")) {
    char expanded[16];
    int value = last_status;
    int i = 0;
    int j;

    if (value == 0) {
      strlcpy_local(token, "0", token_len);
      return;
    }
    if (value < 0) {
      expanded[i++] = '-';
      value = -value;
    }
    {
      char digits[12];
      int d = 0;
      while (value > 0 && d < (int)sizeof(digits)) {
        digits[d++] = (char)('0' + (value % 10));
        value /= 10;
      }
      for (j = d - 1; j >= 0; j--) {
        expanded[i++] = digits[j];
      }
    }
    expanded[i] = '\0';
    strlcpy_local(token, expanded, token_len);
    return;
  }

  strlcpy_local(token, get_var(token + 1), token_len);
}

static int tokenize(char *line, char *argv[MAX_ARGS]) {
  int argc = 0;
  char *src = line;

  while (*src != '\0' && argc < MAX_ARGS - 1) {
    char *dst;
    int quote = 0;

    while (*src == ' ' || *src == '\t') {
      src++;
    }
    if (*src == '\0') {
      break;
    }

    argv[argc++] = src;
    dst = src;

    while (*src != '\0') {
      if (quote == 0 && (*src == ' ' || *src == '\t')) {
        break;
      }
      if (*src == '\'' || *src == '"') {
        if (quote == 0) {
          quote = *src++;
          continue;
        }
        if (quote == *src) {
          quote = 0;
          src++;
          continue;
        }
      }
      if (*src == '\\' && src[1] != '\0') {
        src++;
      }
      *dst++ = *src++;
    }

    *dst = '\0';
    expand_token(argv[argc - 1], VALUE_LEN);

    if (*src != '\0') {
      *src++ = '\0';
    }
  }

  argv[argc] = NULL;
  return argc;
}

static int builtin_colon(char *argv[MAX_ARGS], int argc) {
  (void)argv;
  (void)argc;
  return 0;
}

static int builtin_true(char *argv[MAX_ARGS], int argc) {
  (void)argv;
  (void)argc;
  return 0;
}

static int builtin_false(char *argv[MAX_ARGS], int argc) {
  (void)argv;
  (void)argc;
  return 1;
}

static int builtin_echo(char *argv[MAX_ARGS], int argc) {
  int i;
  for (i = 1; i < argc; i++) {
    if (i > 1) {
      write(STDOUT_FILENO, " ", 1);
    }
    write(STDOUT_FILENO, argv[i], strlen(argv[i]));
  }
  write(STDOUT_FILENO, "\n", 1);
  return 0;
}

static int builtin_pwd(char *argv[MAX_ARGS], int argc) {
  char cwd[VALUE_LEN];
  (void)argv;
  (void)argc;
  if (getcwd(cwd, sizeof(cwd)) == NULL) {
    printf("pwd: failed\n");
    return 1;
  }
  printf("%s\n", cwd);
  set_var("PWD", cwd);
  return 0;
}

static int builtin_cd(char *argv[MAX_ARGS], int argc) {
  char cwd[VALUE_LEN];
  const char *target = argc > 1 ? argv[1] : get_var("HOME");

  if (target[0] == '\0') {
    target = "/";
  }
  if (chdir(target) != 0) {
    printf("cd: %s: not found\n", target);
    return 1;
  }
  if (getcwd(cwd, sizeof(cwd)) != NULL) {
    set_var("PWD", cwd);
  }
  return 0;
}

static int builtin_export(char *argv[MAX_ARGS], int argc) {
  int i;

  if (argc == 1) {
    for (i = 0; i < MAX_ENV; i++) {
      if (shell_vars[i].used) {
        printf("export %s=%s\n", shell_vars[i].name, shell_vars[i].value);
      }
    }
    return 0;
  }

  for (i = 1; i < argc; i++) {
    char *eq = strchr(argv[i], '=');
    if (eq == NULL) {
      if (find_var(argv[i]) == NULL) {
        set_var(argv[i], "");
      }
      continue;
    }
    *eq = '\0';
    set_var(argv[i], eq + 1);
    *eq = '=';
  }
  return 0;
}

static int builtin_unset(char *argv[MAX_ARGS], int argc) {
  int i;
  for (i = 1; i < argc; i++) {
    unset_var(argv[i]);
  }
  return 0;
}

static int builtin_set(char *argv[MAX_ARGS], int argc) {
  int i;
  (void)argv;
  (void)argc;
  for (i = 0; i < MAX_ENV; i++) {
    if (shell_vars[i].used) {
      printf("%s=%s\n", shell_vars[i].name, shell_vars[i].value);
    }
  }
  return 0;
}

static int builtin_exit(char *argv[MAX_ARGS], int argc) {
  int code = argc > 1 ? parse_status(argv[1]) : last_status;
  exit(code);
}

static int builtin_help(char *argv[MAX_ARGS], int argc) {
  (void)argv;
  (void)argc;
  printf("Builtins: : true false echo pwd cd export unset set exit help\n");
  return 0;
}

static int run_builtin(char *argv[MAX_ARGS], int argc) {
  if (argc == 0) {
    return 0;
  }
  if (streq(argv[0], ":")) {
    return builtin_colon(argv, argc);
  }
  if (streq(argv[0], "true")) {
    return builtin_true(argv, argc);
  }
  if (streq(argv[0], "false")) {
    return builtin_false(argv, argc);
  }
  if (streq(argv[0], "echo")) {
    return builtin_echo(argv, argc);
  }
  if (streq(argv[0], "pwd")) {
    return builtin_pwd(argv, argc);
  }
  if (streq(argv[0], "cd")) {
    return builtin_cd(argv, argc);
  }
  if (streq(argv[0], "export")) {
    return builtin_export(argv, argc);
  }
  if (streq(argv[0], "unset")) {
    return builtin_unset(argv, argc);
  }
  if (streq(argv[0], "set")) {
    return builtin_set(argv, argc);
  }
  if (streq(argv[0], "exit")) {
    return builtin_exit(argv, argc);
  }
  if (streq(argv[0], "help")) {
    return builtin_help(argv, argc);
  }
  if (streq(argv[0], "pci")) {
    char buf[2048];
    long ret = sys_pci_list(buf, sizeof(buf) - 1);
    if (ret > 0) {
      buf[ret] = '\0';
      write(STDOUT_FILENO, buf, (size_t)ret);
    }
    return 0;
  }

  if (streq(argv[0], "pci_test")) {
    long failures = sys_pci_test();
    if (failures == 0) {
      printf("PCI self-test: PASS\n");
    } else {
      printf("PCI self-test: %ld failure(s)\n", failures);
    }
    return (int)failures;
  }

  printf("%s: not found\n", argv[0]);
  return 127;
}

int main(void) {
  char line[LINE_BUF];
  char *argv[MAX_ARGS];

  init_shell_vars();
  printf("CoreOS sh\n");

  for (;;) {
    const char *ps1 = get_var("PS1");
    write(STDOUT_FILENO, ps1, strlen(ps1));
    if (read_line(STDIN_FILENO, line, sizeof(line)) <= 0) {
      continue;
    }
    last_status = run_builtin(argv, tokenize(line, argv));
  }
}
