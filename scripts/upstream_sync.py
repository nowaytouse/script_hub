#!/usr/bin/env python3
import os
import re
import json
import subprocess
import concurrent.futures
from datetime import datetime
from lib.common import Logger, get_project_root, write_file

ROOT = get_project_root()
METACUBEX_DIR = os.path.join(ROOT, "ruleset/MetaCubeX")
SKK_UPSTREAM_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/skk_upstream")
MODULE_DIR = os.path.join(ROOT, "module/surge(main)")

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════════

SKK_SOURCES = {
    "reject.conf": "https://ruleset.skk.moe/List/non_ip/reject.conf",
    "reject-no-drop.conf": "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf",
    "reject-drop.conf": "https://ruleset.skk.moe/List/non_ip/reject-drop.conf",
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

NEXUS_MODULES = [
    "https://yfamilys.com/module/bili.module",
    "https://raw.githubusercontent.com/chavyleung/scripts/master/box/rewrite/boxjs.rewrite.surge.sgmodule",
    "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule",
    "https://raw.githubusercontent.com/Semporia/TikTok-Unlock/master/Surge/TiKTok-US.sgmodule",
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
PROTECTED_MODULES = ["🌐 DNS & Host Enhanced.sgmodule"]

# ═══════════════════════════════════════════════════════════════════════════════
# SYNC CLASS
# ═══════════════════════════════════════════════════════════════════════════════

class UpstreamSyncer:
    def __init__(self):
        self.singbox_path = self._find_singbox()

    def _find_singbox(self):
        try:
            result = subprocess.run(["which", "sing-box"], capture_output=True, text=True)
            if result.returncode == 0:
                return result.stdout.strip()
        except: pass
        local_path = os.path.join(ROOT, "scripts/config-manager-auto-update/bin/sing-box")
        if os.path.exists(local_path): return local_path
        return None

    def download(self, url: str) -> bytes:
        """Use system curl for robust SSL handling while maintaining security."""
        last_error = None
        for _ in range(3):
            try:
                # -L follow redirects, -s silent, -m timeout, -f fail on error
                result = subprocess.run(
                    ["curl", "-L", "-s", "-m", "60", "-f", url],
                    capture_output=True, check=True
                )
                if result.stdout:
                    return result.stdout
            except Exception as e:
                last_error = e
        Logger.warn(f"Download failed: {url} - {last_error}")
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
                rules.extend([f"DOMAIN,{d}" for d in rule.get('domain', [])])
                rules.extend([f"DOMAIN-SUFFIX,{d}" for d in rule.get('domain_suffix', [])])
                rules.extend([f"DOMAIN-KEYWORD,{d}" for d in rule.get('domain_keyword', [])])
                rules.extend([f"DOMAIN-REGEX,{d}" for d in rule.get('domain_regex', []) if self._is_valid_regex(d)])
                for ip in rule.get('ip_cidr', []):
                    prefix = "IP-CIDR6" if ":" in ip else "IP-CIDR"
                    rules.append(f"{prefix},{ip},no-resolve")
            rules = sorted(list(set(rules)))
            output_path = os.path.join(METACUBEX_DIR, f"MetaCubeX_{name}.list")
            final_content = f"# MetaCubeX geosite-{name}\n# Rules: {len(rules)}\n\n" + "\n".join(rules) + "\n"
            write_file(output_path, final_content)
            Logger.success(f"MetaCubeX: {name} ({len(rules)} rules)")
        finally:
            for p in [tmp_srs, tmp_json]:
                if os.path.exists(p): os.remove(p)

    def _is_valid_regex(self, pattern):
        try: re.compile(pattern); return True
        except: return False

    def sync_metacubex(self):
        Logger.section("Syncing MetaCubeX Rules (Concurrent)")
        os.makedirs(METACUBEX_DIR, exist_ok=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(self.process_metacubex_rule, name, url) for name, url in METACUBEX_RULES.items()]
            concurrent.futures.wait(futures)

if __name__ == "__main__":
    syncer = UpstreamSyncer()
    syncer.sync_skk()
    syncer.sync_nexus()
    syncer.sync_metacubex()
