import re

def fix_fd_rs():
    with open('kernel/src/proc/fd.rs', 'r') as f:
        content = f.read()
    
    # Fix PROCESSES.lock()[slot].as_mut()
    content = re.sub(
        r'let p = match PROCESSES\.lock\(\)\[slot\]\.as_mut\(\) \{\n\s*Some\((.*?)\) => \1,\n\s*None => (.*?),\n\s*\};',
        r'let mut _lock = PROCESSES.lock();\n    let p = match _lock[slot].as_mut() {\n        Some(\1) => \1,\n        None => \2,\n    };',
        content
    )
    
    # Fix PROCESSES.lock()[slot].as_ref()
    content = re.sub(
        r'let p = PROCESSES\.lock\(\)\[slot\]\.as_ref\(\)\?;',
        r'let _lock = PROCESSES.lock();\n    let p = _lock[slot].as_ref()?;',
        content
    )

    with open('kernel/src/proc/fd.rs', 'w') as f:
        f.write(content)

def fix_process_rs():
    with open('kernel/src/proc/process.rs', 'r') as f:
        content = f.read()
    
    # Fix PROCESSES[slot].as_mut() -> returns reference to temporary
    # We will change current_process and current_process_mut to not exist or change their semantics later,
    # but for now let's just let it compile if possible, or wait, we can't.
    pass

fix_fd_rs()
