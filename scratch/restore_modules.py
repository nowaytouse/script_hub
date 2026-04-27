import os
import subprocess

target_dir = "module/surge(main)/narrow_pierce/"
commit_hash = "3517bbb51762e24a689cc6d52e5a0838e43bb57b^"

# Get list of files in that commit for the directory
cmd = ["git", "ls-tree", "-r", "--name-only", commit_hash, target_dir]
result = subprocess.run(cmd, capture_output=True, text=True)
files_in_commit = result.stdout.splitlines()

for file_path in files_in_commit:
    filename = os.path.basename(file_path)
    if not os.path.exists(file_path):
        print(f"Restoring {file_path}...")
        subprocess.run(["git", "checkout", commit_hash, "--", file_path])
    else:
        print(f"Skipping {file_path} (already exists)")
