#!/usr/bin/env python3
"""Check and fix Surge config for invalid lines"""
import os
import sys

def check_surge_config():
    """Check Surge config for invalid lines"""
    surge_path = os.path.expanduser('~/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf')
    
    if not os.path.exists(surge_path):
        print(f"❌ Surge config not found: {surge_path}")
        return False
    
    print(f"📋 Checking Surge config: {surge_path}")
    print("")
    
    with open(surge_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    issues = []
    in_rule_section = False
    
    for i, line in enumerate(lines, 1):
        stripped = line.strip()
        
        # Track [Rule] section
        if stripped == '[Rule]':
            in_rule_section = True
            continue
        elif stripped.startswith('[') and stripped.endswith(']'):
            in_rule_section = False
            continue
        
        # Check for invalid lines in [Rule] section
        if in_rule_section and stripped and not stripped.startswith('#'):
            # Check for reddit keyword rule
            if 'reddit' in stripped.lower() and 'DOMAIN-KEYWORD' in stripped:
                parts = stripped.split(',')
                if len(parts) >= 3:
                    policy = parts[2]
                    # Check if policy exists (should be "🌐 社交媒体 📱" not "Reddit")
                    if policy == 'Reddit':
                        issues.append({
                            'line': i,
                            'content': stripped,
                            'issue': f'Invalid policy "Reddit" (should be "🌐 社交媒体 📱")',
                            'fix': stripped.replace(',Reddit', ',🌐 社交媒体 📱')
                        })
    
    if issues:
        print(f"❌ Found {len(issues)} invalid line(s):")
        print("")
        for issue in issues:
            print(f"Line {issue['line']}: {issue['issue']}")
            print(f"  Current: {issue['content']}")
            print(f"  Fix to:  {issue['fix']}")
            print("")
        return False
    else:
        print("✅ No invalid lines found!")
        return True

def fix_surge_config():
    """Fix invalid lines in Surge config"""
    surge_path = os.path.expanduser('~/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf')
    
    if not os.path.exists(surge_path):
        print(f"❌ Surge config not found: {surge_path}")
        return False
    
    print(f"🔧 Fixing Surge config: {surge_path}")
    print("")
    
    with open(surge_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    fixed_count = 0
    in_rule_section = False
    
    for i, line in enumerate(lines):
        stripped = line.strip()
        
        # Track [Rule] section
        if stripped == '[Rule]':
            in_rule_section = True
            continue
        elif stripped.startswith('[') and stripped.endswith(']'):
            in_rule_section = False
            continue
        
        # Fix invalid lines in [Rule] section
        if in_rule_section and stripped and not stripped.startswith('#'):
            if 'reddit' in stripped.lower() and 'DOMAIN-KEYWORD' in stripped and ',Reddit' in stripped:
                lines[i] = line.replace(',Reddit', ',🌐 社交媒体 📱')
                fixed_count += 1
                print(f"✅ Fixed line {i+1}: Reddit → 🌐 社交媒体 📱")
    
    if fixed_count > 0:
        # Backup original
        backup_path = surge_path + '.backup'
        with open(backup_path, 'w', encoding='utf-8') as f:
            with open(surge_path, 'r', encoding='utf-8') as orig:
                f.write(orig.read())
        print(f"📦 Backup saved: {backup_path}")
        
        # Write fixed config
        with open(surge_path, 'w', encoding='utf-8') as f:
            f.writelines(lines)
        
        print(f"✅ Fixed {fixed_count} line(s)")
        return True
    else:
        print("ℹ️  No lines need fixing")
        return True

if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--fix':
        success = fix_surge_config()
    else:
        success = check_surge_config()
        if not success:
            print("")
            print("💡 Run with --fix to automatically fix issues:")
            print(f"   python3 {sys.argv[0]} --fix")
    
    sys.exit(0 if success else 1)
