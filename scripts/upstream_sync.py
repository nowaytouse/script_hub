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
from lib.common import Logger, get_project_root, write_file, _curl_fetch, _BROWSER_UA

ROOT = get_project_root()
METACUBEX_DIR = os.path.join(ROOT, "ruleset/MetaCubeX")
SKK_UPSTREAM_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/skk_upstream")
MODULE_DIR = os.path.join(ROOT, "module/surge(main)/amplify_nexus")
LOCAL_SOURCES_DIR = os.path.join(ROOT, "module/local_sources")

# CONFIGURATION

SKK_SOURCES = {
    "reject.list": "https://ruleset.skk.moe/List/non_ip/reject.conf",
    "reject-no-drop.list": "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf",
    "reject-drop.list": "https://ruleset.skk.moe/List/non_ip/reject-drop.conf",
    "reject-url-regex.list": "https://ruleset.skk.moe/List/non_ip/reject-url-regex.conf",
    "sogouinput.list": "https://ruleset.skk.moe/List/non_ip/sogouinput.conf",
    "my_reject.list": "https://ruleset.skk.moe/List/non_ip/my_reject.conf",
    "reject-domainset.list": "https://ruleset.skk.moe/List/domainset/reject.conf",
    "reject-extra-domainset.list": "https://ruleset.skk.moe/List/domainset/reject_extra.conf",
    "reject-phishing-domainset.list": "https://ruleset.skk.moe/List/domainset/reject_phishing.conf",
    "reject-ip.list": "https://ruleset.skk.moe/List/ip/reject.conf",
    "ai.list": "https://ruleset.skk.moe/List/non_ip/ai.conf",
    "apple_cn.list": "https://ruleset.skk.moe/List/non_ip/apple_cn.conf",
    "apple_intelligence.list": "https://ruleset.skk.moe/List/non_ip/apple_intelligence.conf",
    "apple_services.list": "https://ruleset.skk.moe/List/non_ip/apple_services.conf",
    "microsoft.list": "https://ruleset.skk.moe/List/non_ip/microsoft.conf",
    "telegram.list": "https://ruleset.skk.moe/List/non_ip/telegram.conf",
    "global.list": "https://ruleset.skk.moe/List/non_ip/global.conf",
    "direct.list": "https://ruleset.skk.moe/List/non_ip/direct.conf",
    "domestic.list": "https://ruleset.skk.moe/List/non_ip/domestic.conf",
    "download.list": "https://ruleset.skk.moe/List/non_ip/download.conf",
    "neteasemusic.list": "https://ruleset.skk.moe/List/non_ip/neteasemusic.conf",
    "game-download.list": "https://ruleset.skk.moe/List/domainset/game-download.conf",
    "speedtest.list": "https://ruleset.skk.moe/List/domainset/speedtest.conf",
    "cdn.list": "https://ruleset.skk.moe/List/non_ip/cdn.conf",
    "cdn-domainset.list": "https://ruleset.skk.moe/List/domainset/cdn.conf",
    "BlockHttpDNS.list": "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list",
    "telegram-ip.list": "https://ruleset.skk.moe/List/ip/telegram.conf",
    "telegram_asn-ip.list": "https://ruleset.skk.moe/List/ip/telegram_asn.conf",
    "apple_services-ip.list": "https://ruleset.skk.moe/List/ip/apple_services.conf",
    "download-ip.list": "https://ruleset.skk.moe/List/ip/download.conf",
    "neteasemusic-ip.list": "https://ruleset.skk.moe/List/ip/neteasemusic.conf",
    "cdn-ip.list": "https://ruleset.skk.moe/List/ip/cdn.conf",
    "domestic-ip.list": "https://ruleset.skk.moe/List/ip/domestic.conf"
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


NEXUS_MODULES = [
    "https://yfamilys.com/module/bili.module",
    "https://raw.githubusercontent.com/chavyleung/scripts/master/box/rewrite/boxjs.rewrite.surge.sgmodule",
    "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule",
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
    "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule",
    "https://ruleset.skk.moe/Modules/sukka_url_redirect.sgmodule",
]

# Modules merged directly into PROMAX via local_sources (include_sections=True).
# These are NOT shown as standalone modules in the helper HTML.
# Format: { output_filename: upstream_url }
LOCAL_SOURCE_MODULES = {
    "XWebAds.sgmodule": "https://raw.githubusercontent.com/fmz200/wool_scripts/refs/heads/main/Surge/module/XWebAds.module",
    "BiliBili.ADBlock.sgmodule": "https://github.com/BiliUniverse/ADBlock/releases/latest/download/BiliBili.ADBlock.sgmodule",
    "[Sukka] Enhance Better ADBlock for Surge.sgmodule": "https://ruleset.skk.moe/Modules/sukka_enhance_adblock.sgmodule",
}

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
        except Exception:
            pass  # which/not-found is expected on some systems

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
        """Download a large file directly to disk (streams via curl -o) with detailed error reporting."""
        cmd = ["curl", "-L", "-s", "-m", "300", "-f",
               "-H", f"User-Agent: {_BROWSER_UA}", "-o", dest_path, url]
        try:
            result = subprocess.run(cmd, capture_output=True)
            if result.returncode == 0:
                return True
            else:
                stderr = result.stderr.decode("utf-8", errors="ignore").strip()
                Logger.warn(f"File download failed: {url} | exit code {result.returncode} | stderr: {stderr}")
        except Exception as e:
            Logger.warn(f"File download failed: {url} - {e}")
        if os.path.exists(dest_path):
            os.remove(dest_path)
        return False

    def download(self, url: str) -> bytes:
        """Download URL content; returns empty bytes on failure."""
        data = _curl_fetch(url, timeout=60, retries=1)
        if data is None:
            Logger.warn(f"Download failed: {url}")
            return b""
        return data

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
                final = f"# Ruleset: {name}\n\n" + "\n".join(lines) + "\n"
                write_file(os.path.join(SKK_UPSTREAM_DIR, name), final)
                Logger.success(f"SKK: {name} ({len(lines)} rules)")

    def process_nexus_module(self, url: str):
        import urllib.parse
        filename = os.path.basename(url)
        filename = urllib.parse.unquote(filename)
        # Always use .sgmodule for the Surge directory to keep it consistent
        if filename.endswith(".module"):
            filename = filename[:-7] + ".sgmodule"
        elif not filename.endswith(".sgmodule"):
            filename += ".sgmodule"
        
        if filename in PROTECTED_MODULES:
            Logger.info(f"Skipping protected module: {filename}")
            return

        content_bytes = self.download(url)
        if not content_bytes: return
        
        content = content_bytes.decode('utf-8', errors='ignore')
        if "<!DOCTYPE html>" in content:
            Logger.error(f"Invalid file (HTML) for {filename}")
            return

        # DEEP CLEANUP: Remove all existing category lines and duplicate metadata
        lines = content.splitlines()
        new_lines = []
        name_line_idx = -1
        
        for line in lines:
            stripped = line.strip()
            # Skip any existing category lines (to prevent DualSubs-style duplication)
            if stripped.startswith(("#!category", "#!group", "# category:")):
                continue
            
            if stripped.startswith("#!name"):
                name_line_idx = len(new_lines)
            
            new_lines.append(line)

        # Inject standard category
        category_line = f"#!category={NEXUS_GROUP}"
        if name_line_idx != -1:
            new_lines.insert(name_line_idx + 1, category_line)
        else:
            new_lines.insert(0, category_line)
        
        # Final formatting cleanup
        final_content = "\n".join(new_lines).strip() + "\n"
        
        target_path = os.path.join(MODULE_DIR, filename)
        write_file(target_path, final_content)
        Logger.success(f"Nexus: {filename}")

    def process_local_source_module(self, filename: str, url: str):
        """Download an upstream module into local_sources/ for PROMAX merging (include_sections=True)."""
        content_bytes = self.download(url)
        if not content_bytes:
            return
        content = content_bytes.decode('utf-8', errors='ignore')
        if "<!DOCTYPE html>" in content:
            Logger.error(f"Invalid file (HTML) for local_source: {filename}")
            return
        target_path = os.path.join(LOCAL_SOURCES_DIR, filename)
        write_file(target_path, content)
        Logger.success(f"LocalSource: {filename} ← {url}")

    def sync_nexus(self):
        Logger.section("Syncing Amplify Nexus Modules")
        os.makedirs(MODULE_DIR, exist_ok=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = {executor.submit(self.process_nexus_module, url): url for url in NEXUS_MODULES}
            for future in concurrent.futures.as_completed(futures):
                url = futures[future]
                try:
                    future.result()
                except Exception as exc:
                    Logger.warn(f"Nexus sync failed [{url}]: {exc}")

    def sync_local_sources(self):
        """Sync upstream modules that are merged into PROMAX (not standalone)."""
        Logger.section("Syncing Local-Source Modules (→ PROMAX)")
        os.makedirs(LOCAL_SOURCES_DIR, exist_ok=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = {
                executor.submit(self.process_local_source_module, fname, url): fname
                for fname, url in LOCAL_SOURCE_MODULES.items()
            }
            for future in concurrent.futures.as_completed(futures):
                fname = futures[future]
                try:
                    future.result()
                except Exception as exc:
                    Logger.warn(f"LocalSource sync failed [{fname}]: {exc}")

    def process_metacubex_rule(self, name: str, url: str):
        content = self.download(url)
        if not content: return
        cache_dir = os.path.join(ROOT, ".cache")
        os.makedirs(cache_dir, exist_ok=True)
        tmp_srs = os.path.join(cache_dir, f"{name}_{os.getpid()}.srs")
        tmp_json = os.path.join(cache_dir, f"{name}_{os.getpid()}.json")
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
        except re.error:
            return False

    def sync_metacubex(self):
        Logger.section("Syncing MetaCubeX Rules (Concurrent)")
        os.makedirs(METACUBEX_DIR, exist_ok=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = {
                executor.submit(self.process_metacubex_rule, name, url): name
                for name, url in METACUBEX_RULES.items()
            }
            for future in concurrent.futures.as_completed(futures):
                name = futures[future]
                try:
                    future.result()
                except Exception as exc:
                    Logger.warn(f"MetaCubeX sync failed [{name}]: {exc}")

if __name__ == "__main__":
    syncer = UpstreamSyncer()
    syncer.sync_skk()
    syncer.sync_nexus()
    syncer.sync_metacubex()
    syncer.sync_local_sources()
