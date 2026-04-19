import os
import re
import urllib.request

# 配置路径
ARCHIVE_DIR = "/Users/nyamiiko/Downloads/GitHub/script_hub/archive/mosdns_archived"
OUTPUT_DIR = "/Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/rules"
SURGE_RULES_DIR = "/Users/nyamiiko/Downloads/GitHub/script_hub/ruleset/Surge(Shadowkroket)"

# 中国 IP 列表源
CN_IP_URL = "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"

FILE_MAPPING = {
    "steer_Apple.txt": "apple.txt",
    "steer_Google.txt": "google.txt",
    "steer_Microsoft.txt": "microsoft.txt",
    "steer_AI.txt": "ai_cf.txt",
    "steer_Cloudflare.txt": "ai_cf.txt",
    "steer_GitHub.txt": "github.txt",
    "steer_SocialMedia.txt": "social.txt",
    "steer_TikTok.txt": "tiktok.txt",
    "steer_StreamTW.txt": "tw.txt",
    "steer_StreamJP.txt": "jp.txt",
    "steer_NSFW.txt": "nsfw.txt",
    "nsfw_domain.txt": "nsfw.txt",
    "steer_Airport.txt": "airport.txt",
    "steer_Bootstrap.txt": "bootstrap.txt",
    "vendor_ali.txt": "cn.txt",
    "vendor_tencent.txt": "cn.txt",
    "vendor_baidu.txt": "cn.txt",
    "vendor_360.txt": "cn.txt",
}

def download_file(url, dest):
    print(f"Downloading: {url} -> {dest}")
    try:
        urllib.request.urlretrieve(url, dest)
        return True
    except Exception as e:
        print(f"Error downloading {url}: {e}")
        return False

def extract_from_surge(file_path):
    domains = set()
    if not os.path.exists(file_path):
        return domains
    with open(file_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            match = re.match(r"^(DOMAIN|DOMAIN-SUFFIX),([^,]+)", line, re.IGNORECASE)
            if match:
                domains.add(match.group(2).strip())
            elif "," not in line:
                domains.add(line)
    return domains

def extract_from_mosdns(file_path):
    domains = set()
    if not os.path.exists(file_path):
        return domains
    with open(file_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("domain:"):
                domains.add(line[7:])
            elif line.startswith("full:"):
                domains.add(line[5:])
            elif not line.startswith("keyword:"):
                domains.add(line)
    return domains

def main():
    if not os.path.exists(OUTPUT_DIR):
        os.makedirs(OUTPUT_DIR)
        
    # 1. 下载中国 IP 列表
    cn_ip_path = os.path.join(OUTPUT_DIR, "cn_ip.txt")
    download_file(CN_IP_URL, cn_ip_path)

    # 2. 聚合域名
    aggregated = {}
    for src, dest in FILE_MAPPING.items():
        src_path = os.path.join(ARCHIVE_DIR, src)
        if dest not in aggregated:
            aggregated[dest] = set()
        aggregated[dest].update(extract_from_mosdns(src_path))

    # 额外整合 Surge 的 Direct.list 到 cn.txt
    if "cn.txt" not in aggregated:
        aggregated["cn.txt"] = set()
    
    direct_path = os.path.join(SURGE_RULES_DIR, "Direct.list")
    aggregated["cn.txt"].update(extract_from_surge(direct_path))

    for filename, domains in aggregated.items():
        output_path = os.path.join(OUTPUT_DIR, filename)
        with open(output_path, "w", encoding="utf-8") as f:
            for domain in sorted(list(domains)):
                f.write(f"{domain}\n")
        print(f"Sync: {output_path} ({len(domains)} domains)")

if __name__ == "__main__":
    main()
