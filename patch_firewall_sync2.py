filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts/pipeline/firewall_sync.py"
with open(filepath, 'r') as f:
    content = f.read()

# Remove the old FIREWALL_MODULE check
old_check = """
    if not os.path.exists(FIREWALL_MODULE):
        Logger.error(f"Firewall module not found: {FIREWALL_MODULE}")
        return
"""

content = content.replace(old_check, "")

with open(filepath, 'w') as f:
    f.write(content)
