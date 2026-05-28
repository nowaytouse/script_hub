import sys
import os
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(SCRIPT_DIR)
sys.path.insert(0, os.path.join(PROJECT_ROOT, "scripts"))

from lib.common import write_file

target_file = os.path.join(PROJECT_ROOT, "ruleset/Surge(Shadowkroket)/AI.list")

with open(target_file, 'r', encoding='utf-8') as f:
    old_content = f.read()

lines = old_content.splitlines()
new_lines = []
for line in lines:
    if line.startswith("# Updated:"):
        new_lines.append("# Updated: 2026-05-29 12:34:56")
    else:
        new_lines.append(line)

new_content = "\n".join(new_lines) + "\n"

mtime_before = os.path.getmtime(target_file)
time.sleep(1.1)

print("Calling write_file...")
write_file(target_file, new_content)

mtime_after = os.path.getmtime(target_file)

print("mtime before:", mtime_before)
print("mtime after:", mtime_after)
print("Modified?", mtime_before != mtime_after)
