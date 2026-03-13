; syscall.asm — raw x86_64 syscall wrappers
; Calling convention: System V AMD64 (rdi, rsi, rdx, r10, r8, r9)
; Linux syscall convention:  rax=num, rdi=arg1, rsi=arg2, rdx=arg3
; Note: in syscall convention arg4 uses r10 (not rcx, which is clobbered by syscall)

bits 64
section .text

; ssize_t _sys_read(int fd, void *buf, size_t count)
global _sys_read
_sys_read:
    mov rax, 0
    syscall
    ret

; ssize_t _sys_write(int fd, const void *buf, size_t count)
global _sys_write
_sys_write:
    mov rax, 1
    syscall
    ret

; void *_sys_brk(void *addr)
global _sys_brk
_sys_brk:
    mov rax, 12
    syscall
    ret

; int _sys_yield(void)
global _sys_yield
_sys_yield:
    mov rax, 24
    syscall
    ret

; void _sys_exit(int code) __attribute__((noreturn))
global _sys_exit
_sys_exit:
    mov rax, 60
    syscall
    ; should not return, but just in case:
.hang:
    hlt
    jmp .hang
