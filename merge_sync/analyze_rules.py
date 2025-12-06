import re
import os

CONF_PATH = '/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑.conf'

def parse_conf():
    with open(CONF_PATH, 'r') as f:
        lines = f.readlines()
    
    rulesets = []
    current_section = None
    
    for i, line in enumerate(lines):
        line = line.strip()
        if line.startswith('[') and line.endswith(']'):
            current_section = line[1:-1]
            continue
            
        if current_section == 'Rule' and line.startswith('RULE-SET'):
            # Parse RULE-SET line
            # Format: RULE-SET,URL,Policy,options...
            parts = line.split(',')
            if len(parts) >= 3:
                url = parts[1]
                policy = parts[2]
                rulesets.append({
                    'line': i + 1,
                    'url': url,
                    'policy': policy,
                    'raw': line
                })
    
    return rulesets

def main():
    rs = parse_conf()
    print(f"Found {len(rs)} rulesets.")
    for r in rs:
        print(f"Line {r['line']}: {r['url'].split('/')[-1]} -> {r['policy']}")

if __name__ == '__main__':
    main()
