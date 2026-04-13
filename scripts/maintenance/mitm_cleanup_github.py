import os
import re
import argparse

# Configuration for exclusions
GITHUB_DOMAINS = [
    'github.com', 'api.github.com', 'raw.githubusercontent.com', 
    'gist.githubusercontent.com', 'objects.githubusercontent.com'
]
DEFAULT_EXCLUSIONS = ['-github.com', '-api.github.com', '-*.githubusercontent.com']

def process_file(filepath, exclusions, dry_run=False):
    """
    Processes a module file to remove positive GitHub mappings and add exclusions.
    """
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"[!] Error reading {filepath}: {e}")
        return False

    if '[MITM]' not in content:
        return False

    lines = content.splitlines()
    new_lines = []
    modified = False

    for line in lines:
        stripped = line.strip()
        # Only target hostname lines that are not disabled
        if stripped.startswith('hostname') and not stripped.startswith('hostname-disabled'):
            if '=' not in line:
                new_lines.append(line)
                continue
                
            parts = line.split('=', 1)
            prefix = parts[0]
            val_part = parts[1].strip()
            
            # Handle Surge/Shadowrocket tags
            tag = ""
            if val_part.startswith('%APPEND%'):
                tag = "%APPEND% "
                val_part = val_part[len('%APPEND%'):].strip()
            elif val_part.startswith('%INSERT%'):
                tag = "%INSERT% "
                val_part = val_part[len('%INSERT%'):].strip()
            elif val_part.startswith('%SET%'):
                tag = "%SET% "
                val_part = val_part[len('%SET%'):].strip()

            # Split domains by comma and clean up
            domains = [d.strip() for d in val_part.split(',') if d.strip()]
            
            # 1. Remove positive mappings for restricted domains
            restricted = [g.lower() for g in GITHUB_DOMAINS]
            new_domains = [d for d in domains if d.lower() not in restricted]
            
            # 2. Add exclusions if not present
            for ex in exclusions:
                if ex not in new_domains:
                    new_domains.append(ex)
            
            # Check if actual changes were made
            if sorted(new_domains) != sorted(domains):
                modified = True
            
            # Construct new line while preserving original indentation if possible
            new_line = f"{prefix}= {tag}{', '.join(new_domains)}"
            new_lines.append(new_line)
        else:
            new_lines.append(line)

    if modified:
        if dry_run:
            print(f"[DRY RUN] Would modify: {filepath}")
        else:
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write('\n'.join(new_lines) + '\n')
        return True
    return False

def run_cleanup(directory="module", exclusions=DEFAULT_EXCLUSIONS, dry_run=False):
    """
    Main entry point for programmatic cleanup.
    """
    modified_count = 0
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith(('.sgmodule', '.module')):
                path = os.path.join(root, file)
                if process_file(path, exclusions, dry_run):
                    if not dry_run:
                        print(f"[✓] MITM Cleaned: {path}")
                    modified_count += 1
    return modified_count

def main():
    parser = argparse.ArgumentParser(description="Mass-cleanup of MITM hostname exclusions.")
    parser.add_argument("--dir", default="module", help="Directory to scan (default: module)")
    parser.add_argument("--dry-run", action="store_true", help="Print changes without saving")
    parser.add_argument("--exclude", nargs="+", default=DEFAULT_EXCLUSIONS, help="List of exclusions to enforce")
    args = parser.parse_args()

    count = run_cleanup(args.dir, args.exclude, args.dry_run)
    status = "Modification candidates found" if args.dry_run else "Modules modified"
    print(f"\nTotal: {count} {status}")

if __name__ == "__main__":
    main()
