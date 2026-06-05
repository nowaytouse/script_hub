import glob
import ipaddress

ips = ["185.199.108.133", "185.199.109.133", "185.199.110.133", "185.199.111.133"]
parsed_ips = [ipaddress.ip_address(ip) for ip in ips]

for file in glob.glob("rulesets/AdBlock/*.list"):
    with open(file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(",")
            rule_type = parts[0].strip().upper()
            rule_val = parts[1].strip() if len(parts) > 1 else ""
            
            if rule_type in ("IP-CIDR", "IP-CIDR6"):
                try:
                    network = ipaddress.ip_network(rule_val, strict=False)
                    for parsed_ip in parsed_ips:
                        if parsed_ip in network:
                            print(f"MATCH IP in {file}: {line}")
                except ValueError:
                    pass
