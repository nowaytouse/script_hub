import os

filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts/pipeline/adblock_manager.py"
with open(filepath, 'r') as f:
    content = f.read()

# Replace FIREWALL_MODULE definition to avoid errors if it's used elsewhere, but actually we will just use TARGET_MODULE and TARGET_MODULE_LITE in firewall_sync.py.
# We will inject the firewall block into generate_module.
injection = """        rule_lines: List[str] = [
            "# --- SYNCED PORT RULES START ---",
            "# Automated update from ports source",
            "# --- SYNCED PORT RULES END ---",
            "",
            "# 自建防火墙规则 (已注销，拦截功能交由上游列表处理)",
            "# DEST-PORT,8076,REJECT-DROP",
            "# IN-PORT,127.0.0.1:12654,REJECT-DROP",
            "# IN-PORT,8076,REJECT-DROP",
            "# DEST-PORT,31337,REJECT-DROP",
            "# DEST-PORT,31338,REJECT-DROP",
            "# DEST-PORT,12345,REJECT-DROP",
            "# DEST-PORT,12346,REJECT-DROP",
            "",
            "# High-Priority White-list (Prevent DNS failure)",
"""

content = content.replace('        rule_lines: List[str] = [\n            "# High-Priority White-list (Prevent DNS failure)",\n', injection)

with open(filepath, 'w') as f:
    f.write(content)
