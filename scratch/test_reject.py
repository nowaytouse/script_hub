import sys
import os

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(SCRIPT_DIR)

file_path = os.path.join(PROJECT_ROOT, "ruleset/Surge(Shadowkroket)/skk_upstream/reject.list")

with open(file_path, 'r', encoding='utf-8') as f:
    old_content = f.read()

# Simulate download and formatting in upstream_sync.py
lines = [l.strip() for l in old_content.splitlines() if l.strip() and not l.strip().startswith('#')]
# Wait, let's check how upstream_sync.py constructs reject.list:
# final = f"# Ruleset: {name}\n# Updated: {datetime.now()}\n\n" + "\n".join(lines) + "\n"
new_content = f"# Ruleset: reject.list\n# Updated: 2026-05-29 12:34:56.789012\n\n" + "\n".join(lines) + "\n"

def strip_dynamic(text: str) -> str:
    lines = text.splitlines()
    filtered = []
    for line in lines:
        stripped = line.strip()
        if (stripped.startswith("#") and any(k in stripped.lower() for k in ("updated", "date"))):
            continue
        if stripped.startswith("#!date") or stripped.startswith("#!version"):
            continue
        if '"generated":' in stripped or '"date":' in stripped:
            continue
        filtered.append(line)
    return "\n".join(filtered)

old_stripped = strip_dynamic(old_content)
new_stripped = strip_dynamic(new_content)

print("Equal?", old_stripped == new_stripped)
if old_stripped != new_stripped:
    import difflib
    print("Diff:")
    print("\n".join(list(difflib.unified_diff(
        old_stripped.splitlines(),
        new_stripped.splitlines(),
        lineterm=''
    ))))
