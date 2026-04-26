import os
import subprocess

files = ['singbox_entrance.js', 'singbox_landing.js', 'singbox_relay.js']
old_commit = 'f8c76438'
new_commit = '48472fd7'

def get_file_content(commit, filename):
    return subprocess.check_output(['git', 'show', f'{commit}:scripts/Substore/{filename}']).decode('utf-8')

for file in files:
    print(f"Restoring legacy header to {file}...")
    old_content = get_file_content(old_commit, file)
    new_content = get_file_content(new_commit, file)
    
    # Extract header from old (up to the end of the comment block)
    header_end = old_content.find('*/') + 2
    old_header = old_content[:header_end]
    
    # Extract body from new (after its header)
    new_header_end = new_content.find('*/') + 2
    new_body = new_content[new_header_end:]
    
    # Combine
    final_content = old_header + new_body
    
    with open(file, f'w', encoding='utf-8') as f:
        f.write(final_content)

print("Done!")
