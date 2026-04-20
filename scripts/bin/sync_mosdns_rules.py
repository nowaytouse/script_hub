import os
import re
import urllib.request

# ────────────────────────────────────────────────────────────────────
# 配置路径 (MosDNS v5 专配)
# ────────────────────────────────────────────────────────────────────
BASE_DIR = "/Users/nyamiiko/Downloads/GitHub/script_hub/mosdns"
MOSDNS_DATA_DIR = os.path.join(BASE_DIR, "rules/mosdns")
ARCHIVE_DIR = BASE_DIR
SURGE_RULES_DIR = "/Users/nyamiiko/Downloads/GitHub/script_hub/ruleset/Surge(Shadowkroket)"

# 中国 IP 列表源
CN_IP_URL = "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"

# 映射逻辑: 源文件 -> (描述名, 组名)
FILE_MAPPING = {
    "steer_Apple.txt": "steer_Apple.txt",
    "steer_Google.txt": "steer_Google.txt",
    "steer_Microsoft.txt": "steer_Microsoft.txt",
    "steer_AI.txt": "steer_AI.txt",
    "steer_Cloudflare.txt": "steer_AI.txt",
    "steer_GitHub.txt": "steer_GitHub.txt",
    "steer_SocialMedia.txt": "steer_SocialMedia.txt",
    "steer_TikTok.txt": "steer_TikTok.txt",
    "steer_StreamTW.txt": "steer_StreamTW.txt",
    "nsfw_domain.txt": "nsfw_domain.txt",
    "steer_NSFW.txt": "nsfw_domain.txt",
    "steer_Airport.txt": "steer_Airport.txt",
    "steer_Bootstrap.txt": "steer_Bootstrap.txt",
    "vendor_ali.txt": "vendor_ali.txt",
    "vendor_tencent.txt": "vendor_tencent.txt",
    "vendor_baidu.txt": "vendor_baidu.txt",
    "vendor_360.txt": "vendor_360.txt",
}

def clean_line_for_mosdns(line):
    """清洗单行内容为 MosDNS v5 纯域名格式"""
    line = line.strip()
    if not line or line.startswith("#"):
        return None
    # 兼容 Surge 格式: DOMAIN-SUFFIX,example.com,DIRECT
    if "," in line:
        parts = [p.strip() for p in line.split(",")]
        if parts[0].upper() in ["DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-SET"]:
            if len(parts) >= 2:
                line = parts[1]
        else:
            return None
    # 移除 MosDNS 前缀
    prefixes = ["domain:", "full:", "keyword:", "*.", "."]
    for p in prefixes:
        if line.startswith(p):
            line = line[len(p):]
    return line if line else None

def main():
    print("🚀 Starting MosDNS V5 Rule Synchronization...")
    if not os.path.exists(MOSDNS_DATA_DIR):
        os.makedirs(MOSDNS_DATA_DIR)
        
    # 1. 下载最新中国 IP 列表
    cn_ip_path = os.path.join(MOSDNS_DATA_DIR, "cn_ip.txt")
    print(f"📥 Downloading CN IP list...")
    try:
        urllib.request.urlretrieve(CN_IP_URL, cn_ip_path)
    except Exception as e:
        print(f"⚠️ Warning: Failed to download IP list: {e}")

    # 2. 聚合域名数据 (12w 规则集精准分拣)
    groups = {} 
    for src, target_file in FILE_MAPPING.items():
        src_path = os.path.join(ARCHIVE_DIR, src)
        if not os.path.exists(src_path):
            continue
            
        if target_file not in groups:
            groups[target_file] = set()
            
        with open(src_path, "r", encoding="utf-8") as f:
            for line in f:
                cleaned = clean_line_for_mosdns(line)
                if cleaned:
                    groups[target_file].add(cleaned)

    # 3. 构建全量国内域名池 (cn_domain.txt)
    cn_pool = set()
    cn_pool_sources = ["vendor_ali.txt", "vendor_tencent.txt", "vendor_baidu.txt", "vendor_360.txt"]
    for src in cn_pool_sources:
        if src in groups:
            cn_pool.update(groups[src])
    
    # 加入 Surge Direct
    direct_path = os.path.join(SURGE_RULES_DIR, "Direct.list")
    if os.path.exists(direct_path):
        with open(direct_path, "r", encoding="utf-8") as f:
            for line in f:
                cleaned = clean_line_for_mosdns(line)
                if cleaned:
                    cn_pool.add(cleaned)
    
    groups["cn_domain.txt"] = cn_pool

    # 4. 写入规则文件 (排序并去重)
    for filename, domains in groups.items():
        output_path = os.path.join(MOSDNS_DATA_DIR, filename)
        with open(output_path, "w", encoding="utf-8") as f:
            f.write("\n".join(sorted(list(domains))) + "\n")
        print(f"✅ Generated: {filename} ({len(domains)} domains)")

    print("\n🎉 MosDNS Rules are updated and ready for V5.3.4 Engine!")

if __name__ == "__main__":
    main()
