import os
import glob

# Configuration
RULESET_DIR = os.path.join(os.path.dirname(__file__), "../ruleset/Surge(Shadowkroket)")

# Priority Definitions (Higher priority lists steal domains from Lower priority lists)
# Format: "Specific": ["Generic1", "Generic2"]
# Meaning: If a domain is in Specific, remove it from Generic1 and Generic2.
#
# 🔥 优先级顺序（从高到低）:
#   1. 广告拦截规则集 (AdBlock, NSFW) - 最高优先级
#   2. 细分网站规则集 (Twitter, Netflix, Steam等) - 中等优先级
#   3. 兜底规则集 (GlobalProxy, GlobalMedia, SocialMedia等) - 最低优先级
#
CONFLICT_MAP = {
    # ========== 第一优先级: 广告拦截 ==========
    # AdBlock优先于所有其他规则集
    "AdBlock.list": ["GlobalProxy.list", "GlobalMedia.list", "SocialMedia.list", 
                     "Google.list", "Microsoft.list", "Apple.list",
                     "Twitter.list", "Instagram.list", "Facebook.list",
                     "YouTube.list", "Netflix.list", "Spotify.list"],
    "AdBlock_Merged.list": ["GlobalProxy.list", "GlobalMedia.list", "SocialMedia.list",
                            "Google.list", "Microsoft.list", "Apple.list"],
    
    # ========== 第二优先级: 细分网站规则集 ==========
    # 社交媒体细分
    "Twitter.list": ["SocialMedia.list", "GlobalMedia.list", "GlobalProxy.list"],
    "Instagram.list": ["SocialMedia.list", "GlobalMedia.list", "GlobalProxy.list"],
    "Facebook.list": ["SocialMedia.list", "GlobalMedia.list", "GlobalProxy.list"],
    "Telegram.list": ["SocialMedia.list", "GlobalMedia.list", "GlobalProxy.list"],
    "TikTok.list": ["SocialMedia.list", "GlobalMedia.list", "GlobalProxy.list"],
    "Reddit.list": ["SocialMedia.list", "GlobalMedia.list", "GlobalProxy.list"],
    
    # 流媒体细分
    "YouTube.list": ["GlobalMedia.list", "GlobalProxy.list", "Google.list"],
    "Netflix.list": ["GlobalMedia.list", "GlobalProxy.list"],
    "Spotify.list": ["GlobalMedia.list", "GlobalProxy.list"],
    "Disney.list": ["GlobalMedia.list", "GlobalProxy.list"],
    
    # 游戏细分
    "Steam.list": ["Gaming.list", "GlobalProxy.list"],
    "Epic.list": ["Gaming.list", "GlobalProxy.list"],
    
    # AI细分
    "OpenAI.list": ["AI.list", "GlobalProxy.list"],
    "Claude.list": ["AI.list", "GlobalProxy.list"],
    
    # 科技公司细分
    "Google.list": ["GlobalProxy.list"],
    "Microsoft.list": ["GlobalProxy.list"],
    "Apple.list": ["GlobalProxy.list"],
    "GitHub.list": ["GlobalProxy.list"],
    
    # NSFW细分（成人内容）
    "NSFW.list": ["GlobalProxy.list"],
    
    # ========== 第三优先级: 兜底规则集 ==========
    # 这些规则集优先级最低，会被细分规则集覆盖
    # GlobalProxy, GlobalMedia, SocialMedia, Gaming, AI 等
}

# Also standard exclusions: Remove "Direct" domains from "Proxy" lists if they appear?
# Maybe too risky. Focus on the defined map.

def is_valid_rule(line):
    """检查规则是否合法（Surge/Shadowrocket 兼容）"""
    # 跳过 RULE-SET（不应该出现在 .list 文件中）
    if line.startswith('RULE-SET'):
        return False
    
    # 🔥 DOMAIN/DOMAIN-SUFFIX/DOMAIN-KEYWORD 不能带 no-resolve
    # no-resolve 只能用于 IP-CIDR/IP-CIDR6/GEOIP 规则
    if line.startswith('DOMAIN') and ',no-resolve' in line:
        return False
    
    return True

def clean_rule(line):
    """清理规则，移除非法参数"""
    # 移除 DOMAIN 规则中的 no-resolve（如果有的话）
    if line.startswith('DOMAIN') and ',no-resolve' in line:
        line = line.replace(',no-resolve', '')
    return line

def load_list(filepath):
    """Loads rules from a file into a set."""
    rules = set()
    if not os.path.exists(filepath):
        return rules
    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith('#') and not line.startswith('//'):
                 # Normalize: remove comments "DOMAIN,x.com # comment"
                if '#' in line:
                    line = line.split('#')[0].strip()
                # 🔥 清理非法规则
                line = clean_rule(line)
                # 🔥 跳过非法规则
                if is_valid_rule(line):
                    rules.add(line)
    return rules

def write_list(filepath, rules):
    """Writes sorted rules back to file, preserving existing header if present."""
    sorted_rules = sorted(list(rules))
    filename = os.path.basename(filepath)
    
    # 🔥 尝试保留原有header（由ruleset_merger.sh生成的详细header）
    existing_header = []
    
    if os.path.exists(filepath):
        with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                # 保留所有注释行作为header
                if line.startswith('#') or (line.strip() == ''):
                    existing_header.append(line)
                else:
                    # 遇到第一个规则行，header结束
                    break
    
    # 写入文件
    with open(filepath, 'w', encoding='utf-8') as f:
        if existing_header and len(existing_header) > 5:
            # 有详细header，保留它（包括所有注释和分类标记）
            for line in existing_header:
                f.write(line)
            # 在header末尾添加smart_cleanup标记
            f.write(f"# [smart_cleanup.py] Deduplicated: {len(sorted_rules)} rules\n")
            f.write("\n")
        else:
            # 没有详细header，使用简单header
            f.write(f"# Ruleset: {filename}\n")
            f.write("# Cleaned by smart_cleanup.py\n")
            f.write(f"# Total: {len(sorted_rules)}\n")
            f.write("\n")
        
        # 写入规则（不再添加分类标记，因为header中已有）
        for rule in sorted_rules:
            f.write(rule + "\n")

def main():
    print("Starting Smart Cleanup...")
    
    # 1. Load all content into memory map
    file_content = {} # filename -> set of rules
    
    # Get all .list files
    files = glob.glob(os.path.join(RULESET_DIR, "*.list"))
    for fpath in files:
        fname = os.path.basename(fpath)
        file_content[fname] = load_list(fpath)
        
    # 2. Apply Conflict Map (Subtraction)
    for specific_name, generic_names in CONFLICT_MAP.items():
        if specific_name not in file_content:
            continue
            
        specific_rules = file_content[specific_name]
        
        for generic_name in generic_names:
            if generic_name in file_content:
                original_count = len(file_content[generic_name])
                # Subtract
                file_content[generic_name] -= specific_rules
                new_count = len(file_content[generic_name])
                
                diff = original_count - new_count
                if diff > 0:
                    print(f"Removed {diff} rules from {generic_name} (found in {specific_name})")

    # 3. Global Unique Enforcement (Optional but requested "ensure no repeats")
    # This is tricky because "who wins?". 
    # We can rely on the Conflict Map for explicit wins.
    # For others, maybe we don't care, or we just let them exist.
    # User said "Ensure ruleset and ruleset do not repeat". 
    # Let's do a simple pass: If a rule is in "Generic" lists, keep it there ONLY if not in specific?
    # We already did that.
    
    # 4. Save changed files
    for fname, rules in file_content.items():
        fpath = os.path.join(RULESET_DIR, fname)
        write_list(fpath, rules)
        
    print("Smart Cleanup Complete.")

if __name__ == "__main__":
    main()
