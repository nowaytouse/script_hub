import os
import urllib.request
import re
from urllib.parse import urlparse
import ssl

# Constants
CONF_DIR = '/Users/nyamiiko/Library/Mobile Documents/com~apple~CloudDocs/Application/script_hub/ruleset/Surge(Shadowkroket)'
CONF_FILE = '/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑.conf'
NEW_CONF_FILE = '/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf'

# Generic/Polluted lists that need cleaning
GENERIC_LISTS = ['NSFW.list', 'GlobalMedia.list', 'GlobalProxy.list']
# Built-in or special rulesets to ignore (do not download, do not rewrite)
IGNORED_RULESETS = {'SYSTEM', 'LAN', 'GEOIP', 'wired', 'wifi', 'cellular'}

def parse_conf_rules():
    rules = []
    with open(CONF_FILE, 'r') as f:
        lines = f.readlines()
    
    in_rule = False
    for line in lines:
        line = line.strip()
        if line == '[Rule]':
            in_rule = True
            continue
        if line.startswith('['):
            in_rule = False
        
        if in_rule and line.startswith('RULE-SET'):
            # RULE-SET,URL,Policy,...
            parts = line.split(',')
            if len(parts) < 3: continue
            url = parts[1]
            policy = parts[2]
            
            # Smart naming
            if url in IGNORED_RULESETS:
                name = url
            elif url.startswith('http'):
                name = os.path.basename(urlparse(url).path)
            else:
                name = url # Treat as name if no scheme

            rules.append({'name': name, 'url': url, 'policy': policy})
    return rules

def load_ruleset_content(url, name):
    if name in IGNORED_RULESETS:
        print(f"Skipping built-in/ignored: {name}")
        return set(), []

    local_path = os.path.join(CONF_DIR, name)
    content = ""
    is_external = "nowaytouse/script_hub" not in url and url.startswith('http')
    
    if not is_external and os.path.exists(local_path):
        print(f"Reading local: {name}")
        with open(local_path, 'r') as f:
            content = f.read()
    elif url.startswith('http'):
        print(f"Downloading: {name} from {url}")
        try:
            # Create a context that doesn't verify SSL certificates
            ctx = ssl.create_default_context()
            ctx.check_hostname = False
            ctx.verify_mode = ssl.CERT_NONE
            
            with urllib.request.urlopen(url, context=ctx, timeout=30) as response:
                content = response.read().decode('utf-8')
                
            # Save it locally immediately
            with open(local_path, 'w') as f:
                f.write(content)
        except Exception as e:
            print(f"Error downloading {url}: {e}")
            if os.path.exists(local_path):
                print("Using cached version.")
                with open(local_path, 'r') as f:
                    content = f.read()
    else:
        # Not a URL, not local file? might be a relative path or error
        pass

    # Parse content into a Set of lines (code lines only)
    lines = set()
    raw_lines = content.split('\n')
    for line in raw_lines:
        l = line.strip()
        if l and not l.startswith('#') and not l.startswith('//'):
            if '#' in l:
                l = l.split('#')[0].strip()
            lines.add(l)
    
    return lines, raw_lines

def main():
    if not os.path.exists(CONF_DIR):
        os.makedirs(CONF_DIR)

    rules = parse_conf_rules()
    
    # data[name] = set_of_rules
    data = {}
    
    # 1. Load all rulesets
    print("Loading rulesets...")
    for r in rules:
        name = r['name']
        if name not in data:
            rule_set, _ = load_ruleset_content(r['url'], name)
            if rule_set: # Only add if we got content (built-ins return empty)
                data[name] = rule_set
    
    # 2. Identify Protected Content
    print("Identifying protected content...")
    specific_content = set()
    
    for name, content in data.items():
        if name not in GENERIC_LISTS:
            for line in content:
                specific_content.add(line)
    
    # 3. Clean Generics
    print("Cleaning generics...")
    for gen_name in GENERIC_LISTS:
        if gen_name in data:
            start_len = len(data[gen_name])
            data[gen_name] = data[gen_name] - specific_content
            end_len = len(data[gen_name])
            print(f"Ruleset {gen_name}: {start_len} -> {end_len} lines")

    # 4. Global Deduplication
    print("Performing global deduplication...")
    seen_lines = set()
    
    ordered_names = []
    seen_names = set()
    for r in rules:
        if r['name'] not in seen_names and r['name'] not in IGNORED_RULESETS:
            ordered_names.append(r['name'])
            seen_names.add(r['name'])
            
    for name in ordered_names:
        if name not in data: continue
        
        current_set = data[name]
        overlap = current_set.intersection(seen_lines)
        
        if overlap:
            print(f"Removing {len(overlap)} duplicates from {name}")
            data[name] = current_set - overlap
        
        seen_lines.update(data[name])

    # 5. Save Files
    print("Saving files...")
    for name, content in data.items():
        sorted_lines = sorted(list(content))
        save_path = os.path.join(CONF_DIR, name)
        with open(save_path, 'w') as f:
            for line in sorted_lines:
                f.write(line + "\n")
                
    # 6. Create New Config File
    print("Generating new config file...")
    BASE_URL = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/"
    
    with open(CONF_FILE, 'r') as f:
        conf_lines = f.readlines()
        
    with open(NEW_CONF_FILE, 'w') as f:
        in_rule = False
        for line in conf_lines:
            orig_line = line
            line = line.strip()
            if line == '[Rule]':
                in_rule = True
                f.write(orig_line)
                continue
            if line.startswith('['):
                in_rule = False
            
            if in_rule and line.startswith('RULE-SET'):
                parts = line.split(',')
                url = parts[1]
                policy = parts[2]
                
                if url in IGNORED_RULESETS:
                     f.write(orig_line)
                     continue

                if url.startswith('http'):
                    name = os.path.basename(urlparse(url).path)
                else:
                    name = url # fallback
                
                new_url = BASE_URL + name
                options = parts[3:]
                
                new_line = f"RULE-SET,{new_url},{policy}"
                if options:
                    new_line += "," + ",".join(options)
                f.write(new_line + "\n")
            else:
                f.write(orig_line)
                
    print(f"Created {NEW_CONF_FILE}")

if __name__ == '__main__':
    main()
