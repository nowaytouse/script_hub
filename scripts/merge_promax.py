import os
import re

SOURCE_DIR = "/Users/nyamiiko/Downloads/GitHub/script_hub/module/surge(main)/narrow_pierce"
TARGET_FILE = "/Users/nyamiiko/Downloads/GitHub/script_hub/module/surge(main)/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
PROMAX_FILE = "/Users/nyamiiko/Downloads/GitHub/script_hub/module/surge(main)/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"

BLACKLIST_FILES = [
    "WeChat_Enhance.sgmodule",
    "扫描全能王解锁.sgmodule",
    "[Sukka] URL Rewrite.sgmodule"
]

def parse_module(filepath):
    sections = {}
    current_section = None
    headers = []
    
    with open(filepath, 'r', encoding='utf-8') as f:
        for line in f:
            stripped = line.strip()
            if not stripped:
                continue
            
            if current_section is None and stripped.startswith('#!'):
                headers.append(line)
                continue
            
            if re.match(r'^\[.*\]$', stripped):
                current_section = stripped
                if current_section not in sections:
                    sections[current_section] = []
            else:
                if current_section:
                    sections[current_section].append(line)
    
    return headers, sections

def main():
    lite_headers, lite_sections = parse_module(TARGET_FILE)
    
    merged_sections = {}
    for sec, lines in lite_sections.items():
        merged_sections[sec] = lines[:]
            
    files_to_remove = []
    
    for filename in os.listdir(SOURCE_DIR):
        if not filename.endswith('.sgmodule'): continue
        if filename in BLACKLIST_FILES: continue
        
        # Only include files that look like ad-blocking
        if "去广告" not in filename and "Ad" not in filename and "10099" not in filename and "RedNote" not in filename:
            print(f"Skipping {filename} (Doesn't seem like pure AdBlock)")
            continue
            
        filepath = os.path.join(SOURCE_DIR, filename)
        print(f"Merging {filename}...")
        
        _, sections = parse_module(filepath)
        
        mitm_hostnames = set()
        
        for sec, lines in sections.items():
            if sec == '[MITM]':
                for line in lines:
                    if line.strip().startswith('hostname'):
                        parts = line.split('=', 1)
                        if len(parts) == 2:
                            hosts = parts[1].replace('%APPEND%', '').replace('%INSERT%', '').split(',')
                            for h in hosts:
                                h = h.strip()
                                if h and not h.startswith('-github.com') and not h.startswith('-api.github.com') and not h.startswith('-*.githubusercontent.com'):
                                    mitm_hostnames.add(h)
            else:
                if sec not in merged_sections:
                    merged_sections[sec] = []
                if lines:
                    merged_sections[sec].append(f"\n# --- Extracted from {filename} ---")
                    merged_sections[sec].extend(lines)
        
        if mitm_hostnames:
            if '[MITM]' not in merged_sections:
                merged_sections['[MITM]'] = []
            hosts_str = ", ".join(sorted(list(mitm_hostnames)))
            merged_sections['[MITM]'].append(f"hostname = %APPEND% {hosts_str}")
            
        files_to_remove.append(filepath)

    # Update Headers
    new_headers = []
    for h in lite_headers:
        if h.startswith('#!name='):
            new_headers.append("#!name=🚫 Universal Ad-Blocking Rules (PROMAX)\n")
        elif h.startswith('#!desc='):
            new_headers.append("#!desc=Canonical ad-block rebuild from curated rule sources, now fused with application-specific AdBlock modules (PROMAX version).\n")
        else:
            new_headers.append(h)

    with open(PROMAX_FILE, 'w', encoding='utf-8') as f:
        for h in new_headers:
            f.write(h if h.endswith('\n') else h + '\n')
        f.write("\n")
        
        sec_order = ['[General]', '[Rule]', '[URL Rewrite]', '[Body Rewrite]', '[Map Local]', '[Script]', '[MITM]']
        for sec in merged_sections.keys():
            if sec not in sec_order:
                sec_order.append(sec)
                
        for sec in sec_order:
            if sec in merged_sections and merged_sections[sec]:
                f.write(f"{sec}\n")
                
                if sec == '[MITM]':
                    all_hosts = set()
                    other_mitm_lines = []
                    for line in merged_sections[sec]:
                        if line.strip().startswith('hostname'):
                            parts = line.split('=', 1)
                            if len(parts) == 2:
                                hosts = parts[1].replace('%APPEND%', '').replace('%INSERT%', '').split(',')
                                for h in hosts:
                                    h = h.strip()
                                    if h: all_hosts.add(h)
                        else:
                            other_mitm_lines.append(line)
                            
                    for line in other_mitm_lines:
                        f.write(line if line.endswith('\n') else line + '\n')
                        
                    if all_hosts:
                        # Append common exclusions for safety
                        all_hosts.update(['-github.com', '-api.github.com', '-*.githubusercontent.com'])
                        hosts_str = ", ".join(sorted(list(all_hosts)))
                        f.write(f"hostname = %APPEND% {hosts_str}\n")
                else:
                    for line in merged_sections[sec]:
                        f.write(line if line.endswith('\n') else line + '\n')
                f.write("\n")

    for fp in files_to_remove:
        os.remove(fp)
        print(f"Removed {os.path.basename(fp)}")
        
    os.remove(TARGET_FILE)
    print("Merge complete, removed LITE file.")

if __name__ == '__main__':
    main()
