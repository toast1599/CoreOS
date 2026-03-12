bits 64

section .text
global _start

_start:
	mov rax, 0xDEAD
	ud2

section .rodata
msg:        db  "[USERSPACE] hello from ring 3!", 0x0A
msg_len equ $ - msg
