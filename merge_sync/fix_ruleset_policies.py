#!/usr/bin/env python3
"""
Fix ruleset files by removing policy and options from rules.
Ruleset files should only contain rules, not policies.

Example:
  DOMAIN,xxx,REJECT,extended-matching,pre-matching -> DOMAIN,xxx
  IP-CIDR,xxx,no-resolve,REJECT -> IP-CIDR,xxx,no-resolve
"""

import sys
import re
from pathlib import Path

def fix_ruleset(filepath):
    """Fix a single ruleset file."""
    with open(filepath, 'r') as f:
        lines = f.readlines()
    
    cleaned_lines = []
    fixed_count = 0
    
    # Policies that should be removed
    policies = {'REJECT', 'DIRECT', 'PROXY', 'REJECT-DROP', 'REJECT-TINYGIF', 
                'REJECT-NO-DROP', 'REJECT-IMG'}
    
    # Options that should be removed (except no-resolve for IP rules)
    options = {'extended-matching', 'pre-matching'}
    
    for line in lines:
        original_line = line
        line = line.strip()
        
        # Keep comments and empty lines
        if not line or line.startswith('#'):
            cleaned_lines.append(original_line.rstrip())
            continue
        
        parts = line.split(',')
        if len(parts) < 2:
            cleaned_lines.append(line)
            continue
        
        rule_type = parts[0]
        
        # Filter out parts that are policies or options
        new_parts = [parts[0], parts[1]]  # Keep rule type and value
        
        # For IP rules, keep no-resolve if present
        for i in range(2, len(parts)):
            part = parts[i].strip()
            if part.lower() == 'no-resolve':
                new_parts.append('no-resolve')
            elif part.upper() not in policies and part.lower() not in options:
                # Unknown part, might be part of the value
                pass
        
        new_line = ','.join(new_parts)
        
        if new_line != line:
            fixed_count += 1
        
        cleaned_lines.append(new_line)
    
    # Write back
    with open(filepath, 'w') as f:
        f.write('\n'.join(cleaned_lines) + '\n')
    
    return fixed_count

def main():
    ruleset_dir = Path('ruleset/Surge(Shadowkroket)')
    
    if not ruleset_dir.exists():
        print(f"Directory not found: {ruleset_dir}")
        sys.exit(1)
    
    total_fixed = 0
    
    for list_file in ruleset_dir.glob('*.list'):
        fixed = fix_ruleset(list_file)
        if fixed > 0:
            print(f"  {list_file.name}: fixed {fixed} rules")
            total_fixed += fixed
    
    print(f"\nTotal fixed: {total_fixed} rules")

if __name__ == '__main__':
    main()
