import re

with open("scripts/tools/generate_surge_host_dns.py", "r", encoding="utf-8") as f:
    content = f.read()

new_block = """# Traditional IPs for Mainland Speed (Single Primary IP for Surge server: syntax)
TRADITIONAL_CN_ALI = "223.5.5.5"
TRADITIONAL_CN_PUB = "119.29.29.29"
TRADITIONAL_CN_360 = "112.65.69.15"
TRADITIONAL_CN_VOLCANO = "180.184.1.1"

# For generic mainland usage (fast Tencent DNS)
TRADITIONAL_CN_GENERIC = "119.29.29.29"

# Traditional IPs for International Global Speed (Single IP for Surge server: syntax)
TRADITIONAL_GLOBAL = "1.1.1.1"
"""

content = re.sub(r'# Traditional IPs for Mainland Speed \(Dual Stack\).*?TRADITIONAL_GLOBAL = "[^"]+"\n', new_block, content, flags=re.DOTALL)

with open("scripts/tools/generate_surge_host_dns.py", "w", encoding="utf-8") as f:
    f.write(content)
