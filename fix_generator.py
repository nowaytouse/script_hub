import re

filepath = "/Users/nyamiiko/Downloads/GitHub/script_hub/scripts/tools/generate_surge_host_dns.py"
with open(filepath, 'r') as f:
    content = f.read()

# I will replace `lines.extend(...)` logic inside build_host_section() with a smart line-merger.
# But wait, it's easier to write a smart string processing function that takes the FULL string output of `build_host_section()`, finds duplicates, merges them, and removes the later occurrences!

# Let's write the smart function:
def merge_host_lines(lines_str: str) -> str:
    lines = lines_str.splitlines()
    domain_to_ips = {}
    domain_to_line_idx = {}
    
    out_lines = []
    
    for idx, line in enumerate(lines):
        if ' = ' in line and not line.strip().startswith('#'):
            domain = line.split(' = ', 1)[0].strip()
            ips_str = line.split(' = ', 1)[1].strip()
            
            # Split IPs by comma
            ips = [ip.strip() for ip in ips_str.split(',')]
            
            if domain in domain_to_ips:
                # Merge!
                for ip in ips:
                    if ip not in domain_to_ips[domain]:
                        domain_to_ips[domain].append(ip)
                # Update the original line!
                first_idx = domain_to_line_idx[domain]
                out_lines[first_idx] = f"{domain} = {', '.join(domain_to_ips[domain])}"
                # And WE DO NOT APPEND the current line!
                out_lines.append("") # Keep spacing? No, just drop it.
                continue
            else:
                domain_to_ips[domain] = ips
                domain_to_line_idx[domain] = len(out_lines)
                
        out_lines.append(line)
    
    # Filter out empty strings that were dropped duplicates? 
    # Actually, if we just drop them, we might leave empty lines, which is fine, or we can just not append.
    result = []
    for line in out_lines:
        if line == "" and len(result) > 0 and result[-1] == "":
            pass # Skip consecutive empty lines
        else:
            result.append(line)
            
    return "\n".join(result)

# I can inject this at the end of build_host_section() in generate_surge_host_dns.py!
