import glob

domain = "raw.githubusercontent.com"
ips = ["185.199.108.133", "185.199.109.133", "185.199.110.133", "185.199.111.133"]

for file in glob.glob("rulesets/AdBlock/*.list"):
    with open(file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(",")
            rule_type = parts[0].strip().upper()
            rule_val = parts[1].strip() if len(parts) > 1 else ""
            
            if rule_type == "DOMAIN":
                if rule_val.lower() == domain:
                    print(f"MATCH DOMAIN in {file}: {line}")
            elif rule_type == "DOMAIN-SUFFIX":
                if domain.endswith(rule_val.lower()):
                    print(f"MATCH DOMAIN-SUFFIX in {file}: {line}")
            elif rule_type == "DOMAIN-KEYWORD":
                if rule_val.lower() in domain:
                    print(f"MATCH DOMAIN-KEYWORD in {file}: {line}")
            elif rule_type == "IP-CIDR":
                # simplified check
                for ip in ips:
                    if ip.startswith(rule_val.split("/")[0][:7]): # very basic
                        pass
