These files are intentionally quarantined from the active syscall tree.

They target an older process/VFS design built around `process::current_task()`,
lock-based task state, and node-backed file descriptors. The current kernel no
longer uses that model.

Do not re-enable these files by wiring them into `syscall/mod.rs` directly.
If functionality from them is needed, port it into the active syscall modules:

- `kernel/src/syscall/process.rs`
- `kernel/src/syscall/io.rs`
- `kernel/src/syscall/vfs.rs`
- `kernel/src/syscall/sys.rs`

Quarantined on purpose during the syscall/proc refactor to keep the live path
coherent.
