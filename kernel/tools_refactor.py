import os
import re

for root, _, files in os.walk('src'):
    for file in files:
        if not file.endswith('.rs'): continue
        curr_path = os.path.join(root, file)
        
        with open(curr_path, 'r') as f:
            content = f.read()

        orig = content

        content = content.replace('PROCESSES[', 'PROCESSES.lock()[')
        content = content.replace('OPEN_FILES[', 'OPEN_FILES.lock()[')
        content = content.replace('PIPES[', 'PIPES.lock()[')

        content = content.replace('PROCESSES.iter', 'PROCESSES.lock().iter')
        content = content.replace('OPEN_FILES.iter', 'OPEN_FILES.lock().iter')
        content = content.replace('PIPES.iter', 'PIPES.lock().iter')

        if content != orig:
            with open(curr_path, 'w') as f:
                f.write(content)
            print(f"Updated {curr_path}")
