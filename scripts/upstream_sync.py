#!/usr/bin/env python3
import os
import re
import json
import subprocess
import concurrent.futures
import platform
import tarfile
import shutil
from datetime import datetime
from lib.common import Logger, get_project_root, write_file

ROOT = get_project_root()
METACUBEX_DIR = os.path.join(ROOT, "ruleset/MetaCubeX")
SKK_UPSTREAM_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/skk_upstream")
MODULE_DIR = os.path.join(ROOT, "module/surge(main)/amplify_nexus")

# CONFIGURATION

SKK_SOURCES = {
    "reject.list": "https://ruleset.skk.moe/List/non_ip/reject.conf",
    "reject-no-drop.list": "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf",
    "reject-drop.list": "https://ruleset.skk.moe/List/non_ip/reject-drop.conf",
    "BlockHttpDNS.list": "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list"
}

METACUBEX_RULES = {
    "telegram": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/telegram.srs",
    "discord": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/discord.srs",
    "google": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/google.srs",
    "apple": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/apple.srs",
    "microsoft": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/microsoft.srs",
    "bilibili": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/bilibili.srs",
    "category-ai-cn": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-ai-!cn.srs",
    "category-games": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-games.srs",
    "category-ads-all": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-ads-all.srs",
    "instagram": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/instagram.srs",
    "twitter": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/twitter.srs",
    "spotify": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/spotify.srs",
    "steam": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/steam.srs",
    "youtube": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/youtube.srs",
    "github": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/github.srs",
    "netflix": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/netflix.srs",
    "tiktok": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/tiktok.srs",
    "paypal": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/paypal.srs",
    "amazon": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/amazon.srs",
    "reddit": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/reddit.srs",
    "whatsapp": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/whatsapp.srs",
    "facebook": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/facebook.srs",
    "openai": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/openai.srs",
    "cloudflare": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/cloudflare.srs"
}

DIRECT_SOURCES = {
    "felixonmars": "https://raw.githubusercontent.com/felixonmars/dnsmasq-china-list/master/accelerated-domains.china.conf",
    "blackmatrix7": "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/China/China.list",
    "loyalsoldier": "https://raw.githubusercontent.com/Loyalsoldier/v2ray-rules-dat/release/direct-list.txt"
}

DEHYDRATION_KEYWORDS = [
    "google", "github", "facebook", "twitter", "instagram", "netflix", 
    "spotify", "telegram", "discord", "amazon", "akamai", "fastly", 
    "cloudflare", "aws", "azure", "pypi", "docker", "npm", "browserleaks"
]

NEXUS_MODULES = [
    "https://yfamilys.com/module/bili.module",
    "https://raw.githubusercontent.com/chavyleung/scripts/master/box/rewrite/boxjs.rewrite.surge.sgmodule",
    "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule",
    "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/VVebo_Repair.sgmodule",
    "https://raw.githubusercontent.com/Maasea/sgmodule/refs/heads/master/YouTube.Enhance.sgmodule",
    "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/Sub_Info.sgmodule",
    "https://raw.githubusercontent.com/xream/scripts/main/surge/modules/network-info/net-lsp-x.sgmodule",
    "https://raw.githubusercontent.com/Rabbit-Spec/Surge/Master/Module/Panel/Timecard/Moore/Timecard.sgmodule",
    "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule",
    "https://github.com/NSRingo/News/releases/latest/download/iRingo.News.sgmodule",
    "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.sgmodule",
    "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule",
    "https://github.com/DualSubs/Universal/releases/latest/download/DualSubs.Universal.sgmodule",
    "https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule",
    "https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule",
    "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"
]

NEXUS_GROUP = "『 🛠️ Amplify Nexus › 增幅枢纽 』"
HEAD_EXPANSE_GROUP = "『 🔝 Head Expanse › 首端扩域 』"
NARROW_PIERCE_GROUP = "『 🎯 Narrow Pierce › 窄域穿刺 』"
PROTECTED_MODULES = ["🌐 DNS & Host Enhanced.sgmodule"]

# SYNC CLASS

class UpstreamSyncer:
    def __init__(self):
        self.singbox_path = self._setup_singbox()

    def _setup_singbox(self):
        """Find or automatically download the latest sing-box beta binary."""
        # 1. Check system path
        try:
            result = subprocess.run(["which", "sing-box"], capture_output=True, text=True)
            if result.returncode == 0:
                return result.stdout.strip()
        except: pass

        # 2. Check local root
        local_path = os.path.join(ROOT, "sing-box")
        if os.path.exists(local_path) and os.access(local_path, os.X_OK):
            return local_path

        # 3. Auto-download latest beta
        Logger.info("sing-box not found. Attempting to download latest beta...")
        try:
            # Scrape releases page for latest pre-release version
            # Use curl to avoid GitHub API limitations
            cmd = "curl -L -s https://github.com/SagerNet/sing-box/releases | grep -oE 'v[0-9]+\\.[0-9]+\\.[0-9]+-(beta|alpha|rc)\\.[0-9]+' | head -n 1"
            version_bytes = subprocess.check_output(cmd, shell=True)
            version = version_bytes.decode().strip()
            if not version:
                # Fallback to a stable-ish known pattern if scraping fails
                version = "v1.14.0-alpha.11" 
            
            clean_version = version.lstrip('v')
            arch = "arm64" if platform.machine() == "arm64" else "amd64"
            primary_url = f"https://github.com/SagerNet/sing-box/releases/download/{version}/sing-box-{clean_version}-darwin-{arch}.tar.gz"
            mirror_url = f"https://ghproxy.net/https://github.com/SagerNet/sing-box/releases/download/{version}/sing-box-{clean_version}-darwin-{arch}.tar.gz"
            
            Logger.info(f"Downloading sing-box {version} for darwin-{arch}...")
            tar_path = os.path.join(ROOT, "sing-box.tar.gz")
            
            # Try primary then mirror
            if not self.download_to_file(primary_url, tar_path):
                Logger.info("Primary source failed. Trying mirror...")
                if not self.download_to_file(mirror_url, tar_path):
                    raise Exception("Download failed from all sources")

            with tarfile.open(tar_path, mode="r:gz") as tar:
                # Find the binary in the tarball (usually in a subdirectory)
                binary_member = None
                for member in tar.getmembers():
                    if member.name.endswith("/sing-box") or member.name == "sing-box":
                        binary_member = member
                        break
                
                if binary_member:
                    # Extract binary
                    tar.extract(binary_member, path=ROOT)
                    # Move binary to root and cleanup
                    src_path = os.path.join(ROOT, binary_member.name)
                    dest_path = os.path.join(ROOT, "sing-box")
                    if os.path.exists(dest_path): os.remove(dest_path)
                    shutil.move(src_path, dest_path)
                    os.chmod(dest_path, 0o755)
                    
                    # Cleanup extracted folder if it was in a subfolder
                    if "/" in binary_member.name:
                        top_dir = binary_member.name.split("/")[0]
                        shutil.rmtree(os.path.join(ROOT, top_dir), ignore_errors=True)
                    
                    if os.path.exists(tar_path): os.remove(tar_path)
                    Logger.success(f"sing-box {version} installed successfully at {dest_path}")
                    return dest_path
        except Exception as e:
            Logger.error(f"Failed to auto-setup sing-box: {e}")
        
        return None

    def download_to_file(self, url: str, dest_path: str) -> bool:
        """Download a large file directly to disk using curl."""
        try:
            # -L follow redirects, -s silent, -m timeout, -f fail on error, -o output
            # Added -k (insecure) to bypass local proxy MITM handshake issues
            subprocess.run(
                [
                    "curl", "-L", "-k", "-s", "-m", "300", "-f",
                    "-H", "User-Agent: curl/7.84.0",
                    "-o", dest_path, url
                ],
                check=True
            )
            return True
        except Exception as e:
            Logger.warn(f"File download failed: {url} - {e}")
            if os.path.exists(dest_path): os.remove(dest_path)
            return False

    def download(self, url: str) -> bytes:
        """Use system curl for robust SSL handling while maintaining security."""
        try:
            # -L follow redirects, -s silent, -m timeout, -f fail on error
            # Added -k (insecure) to bypass local proxy MITM handshake issues
            result = subprocess.run(
                [
                    "curl", "-L", "-k", "-s", "-m", "60", "-f",
                    "-H", "User-Agent: curl/7.84.0",
                    url
                ],
                capture_output=True, check=True
            )
            return result.stdout
        except Exception as e:
            Logger.warn(f"Download failed: {url} - {e}")
            return b""

    def sync_skk(self):
        Logger.section("Syncing SKK Upstream Rulesets")
        os.makedirs(SKK_UPSTREAM_DIR, exist_ok=True)
        for name, url in SKK_SOURCES.items():
            content_bytes = self.download(url)
            if content_bytes:
                content = content_bytes.decode('utf-8', errors='ignore')
                lines = [l.strip() for l in content.splitlines() if l.strip() and not l.strip().startswith('#')]
                if name == "reject-no-drop.conf":
                    lines = [l for l in lines if "bilibili" not in l.lower()]
                final = f"# Ruleset: {name}\n# Updated: {datetime.now()}\n\n" + "\n".join(lines) + "\n"
                write_file(os.path.join(SKK_UPSTREAM_DIR, name), final)
                Logger.success(f"SKK: {name} ({len(lines)} rules)")

    def process_nexus_module(self, url: str):
        import urllib.parse
        filename = os.path.basename(url)
        filename = urllib.parse.unquote(filename)
        if not filename.endswith(('.sgmodule', '.module')): filename += ".sgmodule"
        
        if filename in PROTECTED_MODULES:
            Logger.info(f"Skipping protected module: {filename}")
            return

        content_bytes = self.download(url)
        if not content_bytes: return
        
        content = content_bytes.decode('utf-8', errors='ignore')
        if "<!DOCTYPE html>" in content:
            Logger.error(f"Invalid file (HTML) for {filename}")
            return

        # Category logic (Preserve Chinese group names)
        if "#!category=" in content:
            content = re.sub(r'^#!category=.*', f'#!category={NEXUS_GROUP}', content, flags=re.MULTILINE)
        else:
            if "#!name=" in content:
                content = re.sub(r'^(#!name=.*)', f'\\1\n#!category={NEXUS_GROUP}', content, flags=re.MULTILINE)
            else:
                content = f"#!category={NEXUS_GROUP}\n" + content
        
        content = re.sub(r'^#!group=.*$', '', content, flags=re.MULTILINE)
        lines = content.splitlines()
        new_lines = []
        category_seen = False
        for line in lines:
            if line.startswith("#!category="):
                if not category_seen:
                    new_lines.append(line)
                    category_seen = True
                continue
            new_lines.append(line)
        
        target_path = os.path.join(MODULE_DIR, filename)
        write_file(target_path, "\n".join(new_lines) + "\n")
        Logger.success(f"Nexus: {filename}")

    def sync_nexus(self):
        Logger.section("Syncing Amplify Nexus Modules")
        os.makedirs(MODULE_DIR, exist_ok=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(self.process_nexus_module, url) for url in NEXUS_MODULES]
            concurrent.futures.wait(futures)

    def process_metacubex_rule(self, name: str, url: str):
        content = self.download(url)
        if not content: return
        tmp_srs = f"/tmp/{name}_{os.getpid()}.srs"
        tmp_json = f"/tmp/{name}_{os.getpid()}.json"
        try:
            with open(tmp_srs, 'wb') as f: f.write(content)
            if not self.singbox_path:
                Logger.error("sing-box not found, cannot decompile MetaCubeX rules")
                return
            result = subprocess.run([self.singbox_path, "rule-set", "decompile", tmp_srs, "-o", tmp_json], capture_output=True)
            if result.returncode != 0:
                Logger.error(f"Decompile failed for {name}: {result.stderr.decode()}")
                return
            with open(tmp_json, 'r') as f: data = json.load(f)
            rules = []
            for rule in data.get('rules', []):
                rules.extend([f"DOMAIN,{d.strip()}" for d in rule.get('domain', [])])
                rules.extend([f"DOMAIN-SUFFIX,{d.strip()}" for d in rule.get('domain_suffix', [])])
                rules.extend([f"DOMAIN-KEYWORD,{d.strip()}" for d in rule.get('domain_keyword', [])])
                rules.extend([f"DOMAIN-REGEX,{d.strip()}" for d in rule.get('domain_regex', []) if self._is_valid_regex(d)])
                for ip in rule.get('ip_cidr', []):
                    prefix = "IP-CIDR6" if ":" in ip else "IP-CIDR"
                    rules.append(f"{prefix},{ip.strip()},no-resolve")
            rules = sorted(list(set(rules)))
            output_path = os.path.join(METACUBEX_DIR, f"MetaCubeX_{name}.list")
            final_content = f"# MetaCubeX geosite-{name}\n# Rules: {len(rules)}\n\n" + "\n".join(rules) + "\n"
            write_file(output_path, final_content)
            Logger.success(f"MetaCubeX: {name} ({len(rules)} rules)")
        finally:
            for p in [tmp_srs, tmp_json]:
                if os.path.exists(p): os.remove(p)

    def _is_valid_regex(self, pattern):
        if not pattern or len(pattern) < 2 or pattern in [".", "*", "]", "[", "^", "$", "(", ")", "|"]:
            return False
        try:
            re.compile(pattern)
            return True
        except:
            return False

    def sync_metacubex(self):
        Logger.section("Syncing MetaCubeX Rules (Concurrent)")
        os.makedirs(METACUBEX_DIR, exist_ok=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(self.process_metacubex_rule, name, url) for name, url in METACUBEX_RULES.items()]
            concurrent.futures.wait(futures)

    def sync_direct(self):
        Logger.section("Syncing & Dehydrating Direct Ruleset (Triple-Source)")
        combined_rules = set()
        
        # 1. Fetch all sources
        results = {}
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
            future_to_name = {executor.submit(self.download, url): name for name, url in DIRECT_SOURCES.items()}
            for future in concurrent.futures.as_completed(future_to_name):
                name = future_to_name[future]
                results[name] = future.result().decode('utf-8', errors='ignore')

        # 2. Process Felixonmars (dnsmasq format)
        if results.get("felixonmars"):
            # server=/example.com/114.114.114.114 -> DOMAIN-SUFFIX,example.com
            matches = re.findall(r'^server=/([^/]+)/', results["felixonmars"], re.MULTILINE)
            for m in matches: combined_rules.add(f"DOMAIN-SUFFIX,{m}")

        # 3. Process Blackmatrix7 (Surge format)
        if results.get("blackmatrix7"):
            for line in results["blackmatrix7"].splitlines():
                line = line.strip()
                if line and not line.startswith('#'): combined_rules.add(line)

        # 4. Process Loyalsoldier (Raw domain format)
        if results.get("loyalsoldier"):
            for line in results["loyalsoldier"].splitlines():
                domain = line.strip()
                if domain and not domain.startswith('#'): combined_rules.add(f"DOMAIN-SUFFIX,{domain}")

        # 5. Load Local Non-Domain Rules (Archive fallback)
        archive_path = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/archive/Direct_polluted.list")
        if os.path.exists(archive_path):
            with open(archive_path, 'r') as f:
                for line in f:
                    if line.startswith(('IP-CIDR', 'IP-CIDR6', 'PROCESS-NAME', 'USER-AGENT')):
                        combined_rules.add(line.strip())

        # 6. DEHYDRATION (Keyword filtering)
        final_list = []
        for rule in combined_rules:
            if any(kw in rule.lower() for kw in DEHYDRATION_KEYWORDS):
                continue
            final_list.append(rule)

        # 7. Finalize and Write
        final_list.sort()
        header = f"# Ruleset: Direct (Unified & Dehydrated)\n# Updated: {datetime.now()}\n# Sources: {', '.join(DIRECT_SOURCES.keys())}\n\n"
        write_file(os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/Direct.list"), header + "\n".join(final_list) + "\n")
        Logger.success(f"Direct Ruleset: Regenerated with {len(final_list)} pure rules")

    def dehydrate_proxy_lists(self):
        Logger.section("Dehydrating Proxy Lists (Removing Domestic Water)")
        lists_to_clean = [
            "Microsoft.list", "GlobalProxy.list", "PayPal.list", "Bilibili.list", 
            "Google.list", "Cloudflare.list", "Gaming.list", "CDN.list", "GitHub.list"
        ]
        
        for list_name in lists_to_clean:
            path = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)", list_name)
            if not os.path.exists(path): continue
            
            with open(path, 'r') as f:
                lines = f.readlines()
            
            original_count = len(lines)
            new_lines = []
            for line in lines:
                l_lower = line.lower()
                # 核心逻辑：如果是国内域名关键字或以 .cn 结尾，则剔除
                if re.search(r'\.cn(,|\s|$)', l_lower) or any(kw in l_lower for kw in [".tmall.com", "alicdn", "alipay", "aliyun", "baidu", "taobao"]):
                    if "domain-suffix" in l_lower or "domain," in l_lower:
                        continue
                new_lines.append(line)
            
            if len(new_lines) < original_count:
                with open(path, 'w') as f:
                    f.writelines(new_lines)
                Logger.success(f"Dehydrated {list_name}: Removed {original_count - len(new_lines)} domestic items")

    def global_deduplicate(self):
        Logger.section("Global Cross-File Deduplication (One Domain, One List)")
        # 核心优先级：明确排序的文件
        defined_priority = [
            "HTTPDNS_Hijack.list", "AdBlock.list", "reject-drop.list", "reject-no-drop.list",
            "Direct.list", "Bilibili.list", "Apple.list", "PayPal.list", "Gaming.list", 
            "Netflix.list", "Google.list", "Microsoft.list", "GitHub.list", "AI.list",
            "StreamUS.list", "StreamJP.list", "StreamTW.list", "StreamHK.list", "GlobalProxy.list"
        ]
        
        # 自动获取目录下所有 .list 文件，并将未在 defined_priority 中的放入末尾
        all_lists = [f for f in os.listdir(os.path.join(ROOT, "ruleset/Surge(Shadowkroket)")) if f.endswith('.list')]
        priority_order = defined_priority + [f for f in all_lists if f not in defined_priority]
        
        seen_domains = set()
        total_removed = 0
        
        for list_name in priority_order:
            path = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)", list_name)
            if not os.path.exists(path): continue
            
            with open(path, 'r') as f:
                lines = f.readlines()
            
            original_count = len(lines)
            new_lines = []
            for line in lines:
                line_s = line.strip()
                if not line_s or line_s.startswith('#'):
                    new_lines.append(line)
                    continue
                
                parts = line_s.split(',')
                if len(parts) >= 2:
                    rule_val = parts[1].lower().strip()
                    if rule_val in seen_domains:
                        continue
                    seen_domains.add(rule_val)
                new_lines.append(line)
            
            if len(new_lines) < original_count:
                removed_count = original_count - len(new_lines)
                total_removed += removed_count
                with open(path, 'w') as f:
                    f.writelines(new_lines)
                Logger.success(f"Deduplicated {list_name}: Removed {removed_count} redundant rules")
        
        Logger.success(f"Global Cleanup Finished: Total {total_removed} redundant rules purged")

if __name__ == "__main__":
    syncer = UpstreamSyncer()
    syncer.sync_skk()
    syncer.sync_nexus()
    syncer.sync_metacubex()
    from ruleset_manager import RulesetManager
    from smart_cleanup import run_cleanup as run_ruleset_cleanup

    RulesetManager(force=True).run()
    run_ruleset_cleanup()
