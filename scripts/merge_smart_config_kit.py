import os
import re
from lib.common import Logger, get_project_root, write_file

ROOT = get_project_root()
SCK_DIR = os.path.join(ROOT, "Smart-Config-Kit/Passwall2/shunt-rules")
CUSTOM_DIR = os.path.join(ROOT, "ruleset/Sources/custom/SmartConfigKit")
SOURCES_DIR = os.path.join(ROOT, "ruleset/Sources/Links")

# Mapping SCK files to Script Hub rulesets
MAPPING = {
    "01-ai-service.list": "AI",
    "02-crypto.list": "Binance",
    "03-payments.list": "PayPal",
    "05-im.list": "SocialMedia",
    "06-social.list": "SocialMedia",
    "08-cn-media.list": "Bilibili",
    "09-sea-media.list": "StreamHK", # Or TW/HK split
    "10-us-media.list": "StreamUS",
    "11-hk-media.list": "StreamHK",
    "12-tw-media.list": "StreamTW",
    "13-jp-media.list": "StreamJP",
    "14-eu-media.list": "StreamEU",
    "16-intl-game.list": "Gaming",
    "17-search.list": "Google", # Or Bing split
    "18-dev.list": "GitHub",
    "19-microsoft.list": "Microsoft",
    "20-apple.list": "Apple",
    "22-cloud-cdn.list": "CDN",
    "25-gfw.list": "GlobalProxy",
    "26-intl-site.list": "GlobalProxy"
}

def parse_sck_list(filepath):
    rules = []
    if not os.path.exists(filepath):
        return rules
    
    with open(filepath, 'r') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            
            # Convert Passwall/Xray syntax to Surge/Clash format used in script_hub
            if line.startswith('domain:'):
                rules.append(f"DOMAIN-SUFFIX,{line[7:]}")
            elif line.startswith('full:'):
                rules.append(f"DOMAIN,{line[5:]}")
            elif line.startswith('keyword:'):
                rules.append(f"DOMAIN-KEYWORD,{line[8:]}")
            # geosite:xxx is ignored as script_hub syncs geosite separately
    return rules

def merge():
    Logger.section("Merging Smart-Config-Kit Rules")
    os.makedirs(CUSTOM_DIR, exist_ok=True)
    
    for sck_file, target_ruleset in MAPPING.items():
        sck_path = os.path.join(SCK_DIR, sck_file)
        rules = parse_sck_list(sck_path)
        
        if not rules:
            continue
        
        custom_file = f"SCK_{target_ruleset}.txt"
        custom_path = os.path.join(CUSTOM_DIR, custom_file)
        
        # Write unique SCK rules to a separate file
        header = f"# Rules from Smart-Config-Kit: {sck_file}\n"
        write_file(custom_path, header + "\n".join(rules) + "\n")
        Logger.success(f"Extracted {len(rules)} rules to {custom_file}")
        
        # Link it in the corresponding _sources.txt
        sources_file = os.path.join(SOURCES_DIR, f"{target_ruleset}_sources.txt")
        if os.path.exists(sources_file):
            with open(sources_file, 'r') as f:
                content = f.read()
            
            relative_link = f"../custom/SmartConfigKit/{custom_file}"
            if relative_link not in content:
                with open(sources_file, 'a') as f:
                    f.write(f"\n# Smart-Config-Kit Supplement\n{relative_link}\n")
                Logger.info(f"Linked {custom_file} in {target_ruleset}_sources.txt")

if __name__ == "__main__":
    merge()
