import re

filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts/tools/generate_surge_host_dns.py"
with open(filepath, 'r') as f:
    content = f.read()

merge_function = """
def merge_host_lines(lines_str: str) -> str:
    lines = lines_str.splitlines()
    domain_to_ips = {}
    domain_to_line_idx = {}
    
    out_lines = []
    
    for idx, line in enumerate(lines):
        if ' = ' in line and not line.strip().startswith('#'):
            domain = line.split(' = ', 1)[0].strip()
            ips_str = line.split(' = ', 1)[1].strip()
            ips = [ip.strip() for ip in ips_str.split(',')]
            
            if domain in domain_to_ips:
                for ip in ips:
                    if ip not in domain_to_ips[domain]:
                        domain_to_ips[domain].append(ip)
                first_idx = domain_to_line_idx[domain]
                out_lines[first_idx] = f"{domain} = {', '.join(domain_to_ips[domain])}"
                out_lines.append("")
                continue
            else:
                domain_to_ips[domain] = ips
                domain_to_line_idx[domain] = len(out_lines)
                
        out_lines.append(line)
    
    result = []
    for line in out_lines:
        if line == "" and len(result) > 0 and result[-1] == "":
            pass
        else:
            result.append(line)
            
    return "\\n".join(result)

def build_host_section() -> str:
"""

content = content.replace("def build_host_section() -> str:\n", merge_function)

build_host_tail = """    lines.extend(tail_block())
    return merge_host_lines("\\n".join(lines).rstrip() + "\\n")
"""

content = content.replace(
    '    lines.extend(tail_block())\n    return "\\n".join(lines).rstrip() + "\\n"\n',
    build_host_tail
)

with open(filepath, 'w') as f:
    f.write(content)
