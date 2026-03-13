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

OBJS    = start.o shell.o libcshim/libcshim.o

.PHONY: all clean

all: shell.elf

start.o: start.asm
	$(ASM) -f elf64 start.asm -o start.o

shell.o: shell.c libcshim/libcshim.h
	$(CC) $(CFLAGS) -c shell.c -o shell.o

libcshim/libcshim.o: libcshim/libcshim.c libcshim/libcshim.h
	$(CC) $(CFLAGS) -c libcshim/libcshim.c -o libcshim/libcshim.o

shell.elf: $(OBJS) shell.ld
	$(LD) $(LDFLAGS) -o shell.elf $(OBJS)

clean:
	rm -f *.o *.elf
