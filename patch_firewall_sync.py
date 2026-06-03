import re

filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts/pipeline/firewall_sync.py"
with open(filepath, 'r') as f:
    content = f.read()

# Replace FIREWALL_MODULE definition
content = content.replace(
    'FIREWALL_MODULE = os.path.join(ROOT, "modules/surge/head_expanse/🛡️ PROMAX AdBlock Helper (DNS & Firewall).sgmodule")',
    'FIREWALL_MODULES = [\n    os.path.join(ROOT, "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"),\n    os.path.join(ROOT, "modules/surge/head_expanse/📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule")\n]'
)

# Replace sync logic to iterate over FIREWALL_MODULES
sync_logic = """
    for module_path in FIREWALL_MODULES:
        if not os.path.exists(module_path):
            Logger.warn(f"Firewall module not found: {module_path}")
            continue

        module_lines = read_file(module_path)
        new_module_content = []
        START_MARKER = "# --- SYNCED PORT RULES START ---"
        END_MARKER = "# --- SYNCED PORT RULES END ---"
        managed_keys = set()
        for rule in port_rules:
            parts = [p.strip().upper() for p in rule.split(',')]
            if len(parts) >= 2:
                managed_keys.add(f"{parts[0]},{parts[1]}")
        
        skip = False
        in_rule_section = False
        for line in module_lines:
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                in_rule_section = (stripped.lower() == "[rule]")
            if START_MARKER in line:
                new_module_content.append(line)
                new_module_content.append(f"# Automated update from ports source\\n")
                for rule in sorted(port_rules): new_module_content.append(f"{rule}\\n")
                skip = True
                continue
            if END_MARKER in line:
                skip = False
                new_module_content.append(line)
                continue
            if not skip:
                if in_rule_section and stripped and not stripped.startswith('#'):
                    parts = [p.strip().upper() for p in stripped.split(',')]
                    if len(parts) >= 2:
                        key = f"{parts[0]},{parts[1]}"
                        if key in managed_keys:
                            continue
                new_module_content.append(line)

        final_content = "".join(new_module_content)

        if execute:
            final_content = re.sub(r'#!version=.*', f'#!version={datetime.now().strftime("%Y.%m.%d")}', final_content)
            write_file(module_path, final_content)
            Logger.success(f"Firewall module updated successfully: {os.path.basename(module_path)}")
        else:
            Logger.info(f"Dry Run: Use --execute to apply changes to {os.path.basename(module_path)}.")
"""

# Find where `# 2. Process module` starts and replace it all until the `if __name__ == "__main__":` block.
start_idx = content.find("    # 2. Process module")
end_idx = content.find("if __name__ == \"__main__\":")

content = content[:start_idx] + sync_logic + "\n" + content[end_idx:]

with open(filepath, 'w') as f:
    f.write(content)

