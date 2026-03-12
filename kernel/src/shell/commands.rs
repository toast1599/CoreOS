/// Shell command handlers.
///
/// Each command receives a `ShellContext` (mutable access to kernel state)
/// and returns a `ShellOutput` that tells the UI what to render.
extern crate alloc;
use super::{cmd_is, get_arg, get_rest, Shell, BUF_LEN};
use crate::boot::CoreOS_BootInfo;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Context — kernel state the commands need to access
// ---------------------------------------------------------------------------

pub struct ShellContext<'a> {
    pub boot_info: *const CoreOS_BootInfo,
    pub filesystem: &'a mut Option<crate::fs::RamFS>,
    pub global_scale: &'a mut usize,
    pub current_y: &'a mut usize,
    pub screen_h: usize,
}

// ---------------------------------------------------------------------------
// Output — what the UI should do after a command runs
// ---------------------------------------------------------------------------

pub enum ShellOutput {
    /// Print the given string at the current output line.
    Print(String),
    /// Print multiple lines (e.g. `ls` output); each entry is one line.
    PrintLines(Vec<String>),
    /// Clear the output area and reset the cursor.
    Clear,
    /// No visible output.
    None,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn dispatch(shell: &Shell, ctx: &mut ShellContext) -> ShellOutput {
    let buf = &shell.buffer;

    if cmd_is(buf, "clear") {
        cmd_clear()
    } else if cmd_is(buf, "meminfo") {
        cmd_meminfo()
    } else if cmd_is(buf, "ls") {
        cmd_ls(ctx)
    } else if cmd_is(buf, "touch") {
        cmd_touch(buf, ctx)
    } else if cmd_is(buf, "rm") {
        cmd_rm(buf, ctx)
    } else if cmd_is(buf, "cat") {
        cmd_cat(buf, 3, ctx)
    } else if cmd_is(buf, "print") {
        cmd_cat(buf, 5, ctx)
    } else if cmd_is(buf, "write") {
        cmd_write(buf, ctx)
    } else if cmd_is(buf, "push") {
        cmd_push(buf, ctx)
    } else if cmd_is(buf, "echo") {
        cmd_echo(buf)
    } else if cmd_is(buf, "uptime") {
        cmd_uptime()
    } else if cmd_is(buf, "ticks") {
        cmd_ticks()
    } else if cmd_is(buf, "sleep") {
        cmd_sleep(buf)
    } else if cmd_is(buf, "font") {
        cmd_font(buf, ctx)
    } else if cmd_is(buf, "exec") {
        cmd_exec(buf, ctx)
    } else if cmd_is(buf, "reboot") {
        unsafe {
            crate::hw::reboot();
        }
    } else {
        ShellOutput::None
    }
}

// ---------------------------------------------------------------------------
// Individual commands
// ---------------------------------------------------------------------------

fn cmd_clear() -> ShellOutput {
    ShellOutput::Clear
}

fn cmd_meminfo() -> ShellOutput {
    let free_mb = crate::pmm::free_bytes() / (1024 * 1024);
    ShellOutput::Print(format!("Free physical RAM: {} MB", free_mb))
}

fn cmd_ls(ctx: &ShellContext) -> ShellOutput {
    let Some(fs) = ctx.filesystem.as_ref() else {
        return ShellOutput::None;
    };
    let mut lines = alloc::vec!["Files in RAM:".to_string()];
    for f in fs.files.iter() {
        let name: String = f.name.iter().collect();
        lines.push(format!("  {} ({} bytes)", name, f.data.len()));
    }
    ShellOutput::PrintLines(lines)
}

fn cmd_touch(buf: &[char; BUF_LEN], ctx: &mut ShellContext) -> ShellOutput {
    let filename = get_arg(buf, 5);
    let Some(fs) = ctx.filesystem.as_mut() else {
        return ShellOutput::None;
    };
    if fs.create(filename) {
        ShellOutput::Print("File created.".into())
    } else {
        ShellOutput::Print("Error: file exists or invalid name.".into())
    }
}

fn cmd_rm(buf: &[char; BUF_LEN], ctx: &mut ShellContext) -> ShellOutput {
    let filename = get_arg(buf, 2);
    let Some(fs) = ctx.filesystem.as_mut() else {
        return ShellOutput::None;
    };
    if fs.remove(filename) {
        ShellOutput::Print("File removed.".into())
    } else {
        ShellOutput::Print("Error: file not found.".into())
    }
}

fn cmd_cat(buf: &[char; BUF_LEN], cmd_len: usize, ctx: &ShellContext) -> ShellOutput {
    let filename = get_arg(buf, cmd_len);
    let Some(fs) = ctx.filesystem.as_ref() else {
        return ShellOutput::None;
    };
    match fs.find(filename) {
        Some(file) => {
            let s: alloc::string::String = file.data.iter().map(|&b| b as char).collect();
            ShellOutput::Print(s)
        }
        None => ShellOutput::Print("Error: file not found.".into()),
    }
}

fn cmd_write(buf: &[char; BUF_LEN], ctx: &mut ShellContext) -> ShellOutput {
    // "write <filename> <content>"
    let filename = get_arg(buf, 5); // first word after "write "
    let content = get_rest(buf, 6 + filename.len()); // everything after "write <filename> "
    let Some(fs) = ctx.filesystem.as_mut() else {
        return ShellOutput::None;
    };
    let Some(file) = fs.find_mut(filename) else {
        return ShellOutput::Print("Error: file not found.".into());
    };
    file.data.clear();
    for &c in content {
        file.data.push(c as u8);
    }
    ShellOutput::Print("Overwritten.".into())
}

fn cmd_push(buf: &[char; BUF_LEN], ctx: &mut ShellContext) -> ShellOutput {
    // "push <filename> <content>"
    let filename = get_arg(buf, 4); // first word after "push "
    let content = get_rest(buf, 5 + filename.len()); // everything after "push <filename> "
    let Some(fs) = ctx.filesystem.as_mut() else {
        return ShellOutput::None;
    };
    let Some(file) = fs.find_mut(filename) else {
        return ShellOutput::Print("Error: file not found.".into());
    };
    for &c in content {
        file.data.push(c as u8);
    }
    ShellOutput::Print("Appended.".into())
}

fn cmd_echo(buf: &[char; BUF_LEN]) -> ShellOutput {
    let s: alloc::string::String = buf[5..].iter().take_while(|&&c| c != '\0').collect();
    ShellOutput::Print(s)
}

fn cmd_uptime() -> ShellOutput {
    ShellOutput::Print(format!(
        "Uptime: {} seconds",
        crate::hw::pit::uptime_seconds()
    ))
}

fn cmd_ticks() -> ShellOutput {
    ShellOutput::Print(format!("Kernel ticks: {}", crate::hw::pit::ticks()))
}

fn cmd_sleep(buf: &[char; BUF_LEN]) -> ShellOutput {
    let arg = get_arg(buf, 5);
    let mut n: u64 = 0;
    for &c in arg {
        if c >= '0' && c <= '9' {
            n = n * 10 + (c as u64 - '0' as u64);
        }
    }
    crate::hw::pit::sleep(n * 100);
    ShellOutput::None
}

fn cmd_font(buf: &[char; BUF_LEN], ctx: &mut ShellContext) -> ShellOutput {
    match buf[5] {
        '+' => {
            if *ctx.global_scale < 4 {
                *ctx.global_scale += 1;
            }
        }
        '-' => {
            if *ctx.global_scale > 1 {
                *ctx.global_scale -= 1;
            }
        }
        _ => {}
    }
    ShellOutput::None
}

fn cmd_exec(buf: &[char; BUF_LEN], ctx: &ShellContext) -> ShellOutput {
    let filename = get_arg(buf, 4);
    let Some(fs) = ctx.filesystem.as_ref() else {
        return ShellOutput::Print("Error: no filesystem.".into());
    };
    let elf_bytes: alloc::vec::Vec<u8> = match fs.find(filename) {
        Some(file) => file.data.clone(),
        None => return ShellOutput::Print("Error: file not found.".into()),
    };

    let pid = unsafe { crate::exec::exec_as_task(elf_bytes.as_slice()) };
    if pid == 0 {
        ShellOutput::Print("exec failed.".into())
    } else {
        ShellOutput::Print(alloc::format!("spawned pid={}", pid))
    }
}

