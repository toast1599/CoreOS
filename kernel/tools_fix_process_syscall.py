import re

with open('kernel/src/syscall/process.rs', 'r') as f:
    content = f.read()

# Fix simple accessors that just map a field
content = re.sub(
    r'proc::current_process\(\)\.map\(\|p\| p\.(\w+) as u64\)\.unwrap_or\(0\)',
    r'proc::with_current_process(|p| p.\1 as u64).unwrap_or(0)',
    content
)

# Fix setuid/setgid etc
content = re.sub(
    r'let process = result::option\(proc::current_process_mut\(\), SysError::Invalid\)\?;\n    process\.(\w+) = (.*?);\n    process\.(\w+) = (.*?);\n    result::ok\(0u64\)',
    r'''result::option(
        proc::with_current_process_mut(|process| {
            process.\1 = \2;
            process.\3 = \4;
        }),
        SysError::Invalid,
    )?;
    result::ok(0u64)''',
    content
)

content = re.sub(
    r'let process = result::option\(proc::current_process_mut\(\), SysError::Invalid\)\?;\n    process\.(\w+) = (.*?);\n    result::ok\((.*?)\)',
    r'''let old = proc::with_current_process(|p| p.\1).unwrap_or(0);
    result::option(
        proc::with_current_process_mut(|process| {
            process.\1 = \2;
        }),
        SysError::Invalid,
    )?;
    result::ok(\3)''',
    content
)

# Fix getpgid / getsid
content = re.sub(
    r'''let slot = result::option\(proc::find_slot_by_pid\(pid as usize\), SysError::NoEntry\)\?;\n\s*let process = result::option\(proc::PROCESSES\.lock\(\)\[slot\]\.as_ref\(\), SysError::NoEntry\)\?;\n\s*result::ok\(process\.(\w+) as u64\)''',
    r'''let val = {
        let lock = proc::PROCESSES.lock();
        let process = result::option(lock[slot].as_ref(), SysError::NoEntry)?;
        process.\1 as u64
    };
    result::ok(val)''',
    content
)

# Fix getresuid etc
content = re.sub(
    r'let process = result::option\(proc::current_process\(\), SysError::Invalid\)\?;',
    r'let process_uid = proc::with_current_process(|p| p.uid).unwrap_or(0);\n    let process_euid = proc::with_current_process(|p| p.euid).unwrap_or(0);\n    let process_gid = proc::with_current_process(|p| p.gid).unwrap_or(0);\n    let process_egid = proc::with_current_process(|p| p.egid).unwrap_or(0);',
    content
)
content = content.replace('&process.uid', '&process_uid')
content = content.replace('&process.euid', '&process_euid')
content = content.replace('&process.gid', '&process_gid')
content = content.replace('&process.egid', '&process_egid')


# Fix setpgid_impl
content = content.replace(
'''    let process = result::option(proc::current_process_mut(), SysError::Invalid)?;
    let new_pgid = if pgid == 0 { target_pid } else { pgid as usize };
    result::ensure(new_pgid != 0, SysError::Invalid)?;
    result::ensure(
        new_pgid == target_pid || new_pgid == process.pgid,
        SysError::Unsupported,
    )?;
    process.pgid = new_pgid;
    result::ok(0u64)''',

'''    let old_pgid = proc::with_current_process(|p| p.pgid).unwrap_or(0);
    let new_pgid = if pgid == 0 { target_pid } else { pgid as usize };
    result::ensure(new_pgid != 0, SysError::Invalid)?;
    result::ensure(
        new_pgid == target_pid || new_pgid == old_pgid,
        SysError::Unsupported,
    )?;
    result::option(
        proc::with_current_process_mut(|p| p.pgid = new_pgid),
        SysError::Invalid,
    )?;
    result::ok(0u64)'''
)

# Fix kill
content = content.replace(
'''    if let Some(process) = proc::current_process_mut() {
        if process.pid == pid as usize {
            process.state = proc::ProcessState::Zombie;
            process.exit_code = 128 + sig as i64;
            task::kill_task(slot);
            return result::ok(0u64);
        }
    }''',
'''    let found = proc::with_current_process_mut(|process| {
        if process.pid == pid as usize {
            process.state = proc::ProcessState::Zombie;
            process.exit_code = 128 + sig as i64;
            true
        } else {
            false
        }
    }).unwrap_or(false);
    if found {
        task::kill_task(slot);
        return result::ok(0u64);
    }'''
)


with open('kernel/src/syscall/process.rs', 'w') as f:
    f.write(content)
print("done")
