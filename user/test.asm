bits 64

section .text
global _start

_start:
    mov rax, 1
    mov rdi, 1
    lea rsi, [rel msg]
    mov rdx, msg_len
    syscall

    mov rax, 60
    xor rdi, rdi
    syscall

.hang:
    hlt
    jmp .hang

section .rodata
msg:        db  "[USERSPACE] hello from ring 3!", 0x0A
msg_len equ $ - msg
