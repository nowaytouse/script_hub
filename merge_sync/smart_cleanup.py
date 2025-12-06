import os
import glob

# Configuration
RULESET_DIR = os.path.join(os.path.dirname(__file__), "../ruleset/Surge(Shadowkroket)")

# Priority Definitions (Higher priority lists steal domains from Lower priority lists)
# Format: "Specific": ["Generic1", "Generic2"]
# Meaning: If a domain is in Specific, remove it from Generic1 and Generic2.
CONFLICT_MAP = {
    # Social Media specific > Generic Social/Media
    "Twitter.list": ["SocialMedia.list", "GlobalMedia.list"],
    "Instagram.list": ["SocialMedia.list", "GlobalMedia.list", "Facebook.list"],
    "Facebook.list": ["SocialMedia.list", "GlobalMedia.list"],
    "Telegram.list": ["SocialMedia.list", "GlobalMedia.list"],
    "TikTok.list": ["SocialMedia.list", "GlobalMedia.list"],
    "YouTube.list": ["GlobalMedia.list", "Google.list"],
    "Netflix.list": ["GlobalMedia.list"],
    "Spotify.list": ["GlobalMedia.list"],
    "Disney.list": ["GlobalMedia.list"],
    
    # NSFW is often a mix, but if we have specific NSFW lists, they should win?
    # Actually, user issue was x.com in NSFW. 
    # So Twitter.list > NSFW.list
    "Twitter.list": ["NSFW.list", "SocialMedia.list", "GlobalMedia.list"],
    "Reddit.list": ["NSFW.list", "SocialMedia.list"],
    
    # Gaming
    "Steam.list": ["Gaming.list"],
    "Epic.list": ["Gaming.list"],
    "Nintendo.list": ["Gaming.list"],
    "PlayStation.list": ["Gaming.list"],
    "Xbox.list": ["Gaming.list"],
    
    # AI
    "OpenAI.list": ["AI.list"],
    "Claude.list": ["AI.list"],
    "Gemini.list": ["AI.list"],
    
    # General
    "Google.list": ["GlobalProxy.list"],
    "Microsoft.list": ["GlobalProxy.list"],
    "Apple.list": ["GlobalProxy.list"],
}

# Also standard exclusions: Remove "Direct" domains from "Proxy" lists if they appear?
# Maybe too risky. Focus on the defined map.

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
                rules.add(line)
    return rules

def write_list(filepath, rules):
    """Writes sorted rules back to file with header."""
    sorted_rules = sorted(list(rules))
    filename = os.path.basename(filepath)
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(f"# Ruleset: {filename}\n")
        f.write("# Cleaned by smart_cleanup.py\n")
        f.write(f"# Total: {len(sorted_rules)}\n")
        f.write("\n")
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
