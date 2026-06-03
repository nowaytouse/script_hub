import re

filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/modules/surge/head_expanse/🌟 AdBlock Helper .sgmodule"
with open(filepath, 'r') as f:
    content = f.read()

# Replace the name
content = content.replace("#!name=🛡️ PROMAX AdBlock Helper (DNS & Firewall)", "#!name=🌟 AdBlock Helper ")

# Remove the [Rule] section entirely
content = re.sub(r'\[Rule\].*?(?=\[|$)', '', content, flags=re.DOTALL)

with open(filepath, 'w') as f:
    f.write(content)
