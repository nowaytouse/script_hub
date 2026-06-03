from typing import List, Set, Dict

def dedupe_section_lines(section: str, lines: List[str]) -> List[str]:
    seen: Set[str] = set()
    result: List[str] = []

    if section == "Host":
        host_map: Dict[str, List[str]] = {}
        for line in lines:
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            if "=" in stripped:
                domain, ips = stripped.split("=", 1)
                domain = domain.strip().lower()
                ip_list = [ip.strip() for ip in ips.split(",") if ip.strip()]
                if domain not in host_map:
                    host_map[domain] = []
                for ip in ip_list:
                    if ip not in host_map[domain]:
                        host_map[domain].append(ip)

        for line in lines:
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            if "=" in stripped:
                domain = stripped.split("=", 1)[0].strip().lower()
                if domain in seen:
                    continue
                seen.add(domain)
                merged_ips = ", ".join(host_map[domain])
                result.append(f"{domain} = {merged_ips}")
            else:
                result.append(stripped)
        return result

    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("#"):
            continue
        # Mock _dedupe_key
        key = f"{section}:{stripped}"
        if not key or key in seen:
            continue
        seen.add(key)
        result.append(stripped)
    return result

test_lines = [
    "freedns.controld.com = 76.76.10.2, 2606:1a40:1::2",
    "freedns.controld.com = 76.76.2.0, 76.76.10.0",
    "dns.google = 8.8.8.8",
    "dns.google = 8.8.4.4, 8.8.8.8"
]

print(dedupe_section_lines("Host", test_lines))
