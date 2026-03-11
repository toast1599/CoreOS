ASM    = nasm
LD     = ld

ASMFLAGS = -f elf64
LDFLAGS  = -static \
           -m elf_x86_64 \
           --entry _start \
           -Ttext 0x400000 \
           --oformat elf64-x86-64

OUT = test.elf

all: $(OUT)

$(OUT): test.asm
	$(ASM) $(ASMFLAGS) test.asm -o test.o
	$(LD)  $(LDFLAGS)  test.o  -o $(OUT)
	rm test.o

clean:
	rm -f $(OUT) test.o
