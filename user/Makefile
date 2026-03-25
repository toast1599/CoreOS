CC      = clang
ASM     = nasm
LD      = ld

# Target: bare-metal x86_64, no standard library
CFLAGS  = -target x86_64-unknown-none \
          -ffreestanding \
          -fno-stack-protector \
          -fno-builtin \
          -O2 \
          -I libcshim \
          -mno-red-zone

LDFLAGS = -T shell.ld \
          -nostdlib \
          --no-dynamic-linker \
          -static

COMMON_OBJS = start.o libcshim/libcshim.o
ALL_ELFS = shell.elf syscall_test.elf syscall_child.elf posix_newsys_test.elf

.PHONY: all clean

all: $(ALL_ELFS)

start.o: start.asm
	$(ASM) -f elf64 start.asm -o start.o

shell.o: shell.c libcshim/libcshim.h libcshim/sysnr.h
	$(CC) $(CFLAGS) -c shell.c -o shell.o

syscall_test.o: syscall_test.c libcshim/libcshim.h libcshim/sysnr.h
	$(CC) $(CFLAGS) -c syscall_test.c -o syscall_test.o

syscall_child.o: syscall_child.c libcshim/libcshim.h libcshim/sysnr.h
	$(CC) $(CFLAGS) -c syscall_child.c -o syscall_child.o

posix_newsys_test.o: posix_newsys_test.c libcshim/libcshim.h libcshim/sysnr.h
	$(CC) $(CFLAGS) -c posix_newsys_test.c -o posix_newsys_test.o

libcshim/libcshim.o: libcshim/libcshim.c libcshim/libcshim.h libcshim/sysnr.h
	$(CC) $(CFLAGS) -c libcshim/libcshim.c -o libcshim/libcshim.o

shell.elf: $(COMMON_OBJS) shell.o shell.ld
	$(LD) $(LDFLAGS) -o shell.elf $(COMMON_OBJS) shell.o

syscall_test.elf: $(COMMON_OBJS) syscall_test.o shell.ld
	$(LD) $(LDFLAGS) -o syscall_test.elf $(COMMON_OBJS) syscall_test.o

syscall_child.elf: $(COMMON_OBJS) syscall_child.o shell.ld
	$(LD) $(LDFLAGS) -o syscall_child.elf $(COMMON_OBJS) syscall_child.o

posix_newsys_test.elf: $(COMMON_OBJS) posix_newsys_test.o shell.ld
	$(LD) $(LDFLAGS) -o posix_newsys_test.elf $(COMMON_OBJS) posix_newsys_test.o

clean:
	rm -f *.o *.elf
