bits 64

global _start
extern shell_main

section .text
_start:
    ; Align stack to 16 bytes (ABI requirement before any call)
    and rsp, ~0xF

    call shell_main

    ; shell_main should never return (it calls sys_exit),
    ; but if it does, exit cleanly.
    mov rdi, 0
    mov rax, 60
    syscall
