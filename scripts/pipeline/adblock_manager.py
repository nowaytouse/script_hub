#!/usr/bin/env python3
import argparse
import ipaddress
import json
import os
import re
import subprocess
import sys
import urllib.parse
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, List, Optional, Set, Tuple

from hub.common import Logger, get_file_hash, get_project_root, read_file, write_file, _BROWSER_UA, safe_download, safe_remove
from hub.module_sanitizer import dedupe_section_lines, format_header, format_module, merge_mitm_hosts
from hub.promax_line_classifier import (
    classify_promax_line,
    module_ingest_mode,
    should_keep_promax_line,
)
from hub.promax_module_split import format_split_module, split_module_sections
from hub.surge_compliance import (
    format_url_regex_for_surge,
    is_invalid_domain_regex_payload,
    strip_trailing_policy,
)

# ============================================================================
# CONFIGURATION
# ============================================================================

from hub.paths import (
    ADBLOCK_DIR,
    ADBLOCK_LIST,
    HTTPDNS_HIJACK_LIST,
    LIST_DIR,
    LOCAL_DIR,
    SKK_UPSTREAM_DIR,
)

ROOT = get_project_root()
SURGE_MODULE_DIR = os.path.join(ROOT, "modules/surge")
HEAD_EXPANSE_DIR = os.path.join(ROOT, "modules/surge/head_expanse")
CACHE_DIR = os.path.join(ROOT, ".cache")
HASH_FILE = os.path.join(CACHE_DIR, "adblock_hashes.txt")
WHITELIST_FILE = os.path.join(ROOT, "rulesets/Sources/adblock_whitelist.txt")
ADBLOCK_SOURCES_FILE = os.path.join(ROOT, "rulesets/Sources/Links/AdBlock_sources.txt")
ADBLOCK_FUNCTIONAL_SOURCES_FILE = os.path.join(
    ROOT, "rulesets/Sources/Links/AdBlock_functional_sources.txt"
)
LOCAL_SOURCES_DIR = LOCAL_DIR
LOCAL_MODULES_DIR = os.path.join(ROOT, "rulesets/Sources/LocalModules")
AMPLIFY_NEXUS_DIR = os.path.join(ROOT, "modules/surge/amplify_nexus")
# Rule-ingest hint for remote/list sources only (not used to pull amplify 增强 modules).
NEXUS_AD_RULE_KEYWORDS = (
    "reject",
    "广告",
    "拦截",
    "anti-ad",
    "adblock",
    "去广告",
)

PROMAX_DISPLAY_NAME = "🚫 Universal Ad-Blocking Rules (PROMAX)"

# Disqualify unlock / 功能增强 / 翻译类模块（永不并入 PROMAX 功能段）
FUNCTIONAL_BLOCK_NAME_TOKENS = (
    "解锁",
    "unlock",
    "unblock",
    "破解",
    "crack",
    "会员",
    "vip",
    "premium",
    "增强",
    "enhance",
    "enhanced",
    ".global",
    "global.sgmodule",
    "redirect",
    "翻译",
    "translation",
    "nsfw",
    "iringo",
    "weatherkit",
    "dualsubs",
    "sub_info",
    "boxjs",
    "timecard",
    "net-lsp",
    "surge-beta",
    "apple服务",
    "wechat_enhance",
    "reddit",
    "扫描全能",
    "camscanner",
    "nsringo",
    "geoservices",
    "boxjs",
    "maasea/sgmodule",
    "youtube.enhance",
    "增强合集",
    "bilibili.enhanced",
    "bilibili.global",
    "bilibili.redirect",
)

# 专项去广告模块文件名特征（规则 + 脚本并入 PROMAX，不提供单独安装）
ADBLOCK_MODULE_NAME_TOKENS = (
    "去广告",
    "adblock",
    "anti-ad",
    "xwebads",
    "allinone",
    "all-in-one",
    "adultraplus",
    "微博去广告",
    "rednote",
    "redpaper",
)

# 允许含 “enhance” 字样的经典去广告包
ADBLOCK_FILENAME_ALLOWLIST = frozenset(
    {
        "[Sukka] Enhance Better ADBlock for Surge.sgmodule",
        "sukka_url_redirect.sgmodule",
        "AllInOne_Mock.sgmodule",
        "All-in-One-2.x.sgmodule",
        "XWebAds.sgmodule",
        "BiliBili.ADBlock.sgmodule",
        "YouTube.ADBlock.sgmodule",
        "WeChat.ADBlock.sgmodule",
    }
)

FUNCTIONAL_BLOCK_LOCAL_SOURCES = frozenset({"Reddit.ADBlock.sgmodule"})
TARGET_MODULE = os.path.join(
    HEAD_EXPANSE_DIR,
    "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
)
TARGET_MODULE_LITE = os.path.join(
    HEAD_EXPANSE_DIR,
    "📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule",
)

# Categories included in the Lite (mobile-friendly) tier.
# Excludes heavy ThreatIntel shards (~1.2M rules) that are desktop-only.
LITE_CATEGORIES = {"Local", "Advertising", "Privacy", "Security", "AntiAD", "Other"}
NARROW_PIERCE_DIR = os.path.join(ROOT, "modules/surge/narrow_pierce")
PROMAX_SPLITS_DIR = os.path.join(ROOT, "modules/source/build/promax_splits")
SKK_REJECT = os.path.join(SKK_UPSTREAM_DIR, "reject.list")
SKK_HTTPDNS = os.path.join(SKK_UPSTREAM_DIR, "BlockHttpDNS.list")
FIREWALL_MODULE = os.path.join(HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.sgmodule")
ADBLOCK_CATALOG_JSON = os.path.join(ADBLOCK_DIR, "catalog.json")
ADBLOCK_README = os.path.join(ADBLOCK_DIR, "README.md")
ADBLOCK_CALLCHAIN_DOC = os.path.join(ROOT, "docs", "AdBlock_callchain.md")

CDN_BASE_URL = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
RULES_PER_SHARD = 100_000

# Purpose taxonomy: internal key → display label + dedup priority (earlier = higher)
CATEGORY_META: Dict[str, Dict[str, str]] = {
    "Local": {
        "label_zh": "应用去广告",
        "desc": "各 App 去广告域名规则（脚本/重写已并入 PROMAX，勿单独安装专项模块）",
    },
    "Advertising": {
        "label_zh": "通用广告",
        "desc": "blackmatrix7 / ACL4SSR / geekdada / 广告联盟等通用域名",
    },
    "Privacy": {
        "label_zh": "隐私追踪",
        "desc": "Privacy / tracking-protection / 防追踪列表",
    },
    "Security": {
        "label_zh": "安全威胁",
        "desc": "劫持 / 钓鱼 / BanProgram / HTTPDNS / skk reject",
    },
    "AntiAD": {
        "label_zh": "Anti-AD",
        "desc": "privacy-protection-tools anti-AD 主列表与 discretion",
    },
    "ThreatIntel_Ultimate": {
        "label_zh": "威胁情报·终极",
        "desc": "HaGeZi DNS-Blocklists ultimate",
    },
    "ThreatIntel_TIF": {
        "label_zh": "威胁情报·TIF",
        "desc": "HaGeZi TIF medium 威胁情报",
    },
    "ThreatIntel": {
        "label_zh": "威胁情报·补充",
        "desc": "HaGeZi / skk 其它威胁与钓鱼域名集",
    },
    "Other": {
        "label_zh": "其他补充",
        "desc": "社区 sgmodule / 未归类远程源",
    },
}

GROUP_HEAD_EXPANSE = "『 🔝 Head Expanse › 首端扩域 』"
MODULE_SUFFIXES = (".sgmodule", ".module")
REMOTE_PREFIXES = ("http://", "https://")
# PROMAX ships RULE-SET refs plus merged functional sections from AdBlock_functional_sources.txt.
SECTION_NAMES = ("URL Rewrite", "Map Local", "Script", "Body Rewrite", "Header Rewrite")
MODULE_RULE_SECTIONS = frozenset({"Rule", "Adblock Plus"})
PROTECTED_MODULES: tuple[str, ...] = ()
SUPPORTED_POLICIES = {
    "REJECT",
    "REJECT-DROP",
    "REJECT-NO-DROP",
    "REJECT-TINYGIF",
    "REJECT-IMG",
    "DIRECT",
    "PROXY",
}
ALLOWED_RULE_PREFIXES = (
    "DOMAIN,",
    "DOMAIN-SUFFIX,",
    "DOMAIN-KEYWORD,",
    "DOMAIN-REGEX,",
    "DOMAIN-SET,",
    "IP-CIDR,",
    "IP-CIDR6,",
    "URL-REGEX,",
    "USER-AGENT,",
    "PROCESS-NAME,",
    "DEST-PORT,",
)
BODY_REWRITE_PREFIXES = ("http-response", "http-request", "http-response-jq", "http-request-jq")
UNSAFE_IP_RULE_PREFIXES = (
    "IP-CIDR,0.0.0.",
    "IP-CIDR,127.",
    "IP-CIDR,10.0.0.0/8",
    "IP-CIDR,192.168.0.0/16",
    "IP-CIDR,172.16.0.0/12",
)
MODULE_TARGET_ALIASES = {
}


@dataclass(frozen=True)
class SourceEntry:
    manifest_path: str
    source: str
    policy: str = "REJECT"
    kind: str = "auto"

    @property
    def is_remote(self) -> bool:
        return self.source.startswith(REMOTE_PREFIXES)

    @property
    def resolved_path(self) -> str:
        if self.is_remote:
            return self.source
        
        # Decode possible URL encoding in local path (e.g. %E5%B9%BF%E5%91%8A)
        decoded_source = urllib.parse.unquote(self.source)
        
        # If it's an absolute path within the project (starts with /), join with ROOT
        if decoded_source.startswith("/"):
            return os.path.normpath(os.path.join(ROOT, decoded_source.lstrip("/")))
            
        # Otherwise it's relative to the manifest file
        return os.path.normpath(os.path.join(os.path.dirname(self.manifest_path), decoded_source))

    @property
    def display_name(self) -> str:
        if self.is_remote:
            return self.source.rsplit("/", 1)[-1]
        # For local files, show relative path from ROOT for clarity
        try:
            return os.path.relpath(self.resolved_path, ROOT)
        except ValueError:
            return os.path.basename(self.resolved_path)


class AdBlockManager:
    def __init__(self):
        self.whitelist: Set[str] = set()
        # rules[policy][category] = set(rules); order = dedup priority (first wins)
        self.category_names = list(CATEGORY_META.keys())
        self.rules: Dict[str, Dict[str, Set[str]]] = {
            policy: {cat: set() for cat in self.category_names}
            for policy in ["REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT"]
        }
        # seen_rules[policy] = Set(rendered_rule)
        self.seen_rules: Dict[str, Set[str]] = {
            policy: set() for policy in ["REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT"]
        }
        self.sections: Dict[str, List[str]] = {name: [] for name in SECTION_NAMES}
        self._section_seen: Dict[str, Set[str]] = {name: set() for name in SECTION_NAMES}
        self.functional_stats: Dict[str, int] = {}
        self._functional_manifest_paths: Set[str] = set()
        self.mitm_hosts: Set[str] = set()
        self.hashes: Dict[str, str] = {}

    def cleanup_local_lists(self):
        """Clean all configured local lists from whitelisted items."""
        targets = [ADBLOCK_LIST, SKK_REJECT, SKK_HTTPDNS]
        # Also find any local lists in sources
        source_entries = self.load_source_entries()
        for entry in source_entries:
            if not entry.is_remote and entry.resolved_path.endswith(".list"):
                targets.append(entry.resolved_path)
            # Find HTTPDNS_Hijack.list - it's hardcoded in header but let's find its path
            hijack_path = HTTPDNS_HIJACK_LIST
            if os.path.exists(hijack_path):
                targets.append(hijack_path)

        for path in set(targets):
            if not os.path.exists(path):
                continue
            lines = read_file(path)
            new_lines = []
            changed = False
            for line in lines:
                stripped = line.strip()
                if not stripped or stripped.startswith("#"):
                    new_lines.append(line)
                    continue
                # Simple domain check in rule line
                if any(white_item in stripped for white_item in self.whitelist):
                    changed = True
                    continue
                new_lines.append(line)
            if changed:
                Logger.info(f"Scrubbed whitelist items from {os.path.basename(path)}")
                write_file(path, "".join(new_lines))

    def load_whitelist(self):
        if not os.path.exists(WHITELIST_FILE):
            return
        for line in read_file(WHITELIST_FILE):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            cleaned = re.sub(r"^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN),", "", line)
            self.whitelist.add(cleaned)
        Logger.info(f"Loaded {len(self.whitelist)} whitelist patterns")

    def load_hashes(self):
        if not os.path.exists(HASH_FILE):
            return
        for line in read_file(HASH_FILE):
            if "|" not in line:
                continue
            name, digest = line.strip().split("|", 1)
            self.hashes[name] = digest

    def save_hashes(self, hashes: Dict[str, str]):
        content = "\n".join(f"{k}|{v}" for k, v in sorted(hashes.items())) + "\n"
        write_file(HASH_FILE, content)

    def _download(self, url: str) -> Optional[str]:
        url = urllib.parse.unquote(url)
        return safe_download(url, retries=2, timeout=60)

    def discover_local_modules(self) -> List[str]:
        discovered_paths = []
        seen_canonical_names = set()
        excluded = {os.path.basename(TARGET_MODULE), os.path.basename(FIREWALL_MODULE)}
        
        # Scan HEAD_EXPANSE_DIR
        for entry in sorted(os.listdir(HEAD_EXPANSE_DIR)):
            if not entry.endswith(MODULE_SUFFIXES):
                continue
            if entry in excluded:
                continue
            
            canonical_name = urllib.parse.unquote(entry)
            if canonical_name in seen_canonical_names:
                continue
            
            seen_canonical_names.add(canonical_name)
            discovered_paths.append(os.path.join(HEAD_EXPANSE_DIR, entry))
            
        return discovered_paths

    def load_source_entries(self) -> List[SourceEntry]:
        entries: List[SourceEntry] = []
        seen = set()
        if not os.path.exists(ADBLOCK_SOURCES_FILE):
            Logger.warn(f"Sources file not found: {ADBLOCK_SOURCES_FILE}")
            return entries

        for raw_line in read_file(ADBLOCK_SOURCES_FILE):
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue

            parts = [part.strip() for part in line.split("|")]
            source = parts[0]
            policy = parts[1].upper() if len(parts) > 1 and parts[1] else "REJECT"
            kind = parts[2].lower() if len(parts) > 2 and parts[2] else "auto"

            if policy not in SUPPORTED_POLICIES:
                Logger.warn(f"Skipping invalid policy in AdBlock manifest: {line}")
                continue
            if kind not in {"auto", "domainset"}:
                Logger.warn(f"Skipping invalid source kind in AdBlock manifest: {line}")
                continue

            entry = SourceEntry(
                manifest_path=ADBLOCK_SOURCES_FILE,
                source=source,
                policy=policy,
                kind=kind,
            )
            key = (entry.source, entry.policy, entry.kind)
            if key in seen:
                continue
            seen.add(key)
            entries.append(entry)

        Logger.info(f"Loaded {len(entries)} canonical AdBlock sources")
        return entries

    def resolve_module_ingest_mode(self, path: str, *, from_manifest: bool = False) -> str:
        """skip | full | split — see lib.promax_line_classifier.module_ingest_mode."""
        if not os.path.isfile(path):
            return "skip"
        name = os.path.basename(path)
        if name in FUNCTIONAL_BLOCK_LOCAL_SOURCES:
            return "skip"
        if name in ADBLOCK_FILENAME_ALLOWLIST:
            return "full"
        low = name.lower()
        # amplify_nexus 默认仅增幅枢纽独立安装；manifest 显式列出的混合包（如 B 站合集）才并入
        if (
            not from_manifest
            and os.path.dirname(os.path.normpath(path))
            == os.path.normpath(AMPLIFY_NEXUS_DIR)
        ):
            return "skip"
        if not from_manifest and any(tok in low for tok in FUNCTIONAL_BLOCK_NAME_TOKENS):
            return "skip"
        text = "".join(read_file(path))
        mode = module_ingest_mode(path, text)
        if mode != "skip":
            return mode
        if os.path.dirname(os.path.normpath(path)) == os.path.normpath(LOCAL_SOURCES_DIR):
            return "full" if name.endswith(MODULE_SUFFIXES) else "skip"
        if any(tok in low for tok in ADBLOCK_MODULE_NAME_TOKENS):
            return "full"
        return "skip"

    def write_promax_split_artifact(self, source_path: str) -> None:
        """Write ad-only extract for mixed modules (review / audit)."""
        text = "".join(read_file(source_path))
        ad_sections, stats = split_module_sections(text, source_path=source_path)
        if stats.get("kept_lines", 0) == 0:
            return
        os.makedirs(PROMAX_SPLITS_DIR, exist_ok=True)
        stem = os.path.splitext(os.path.basename(source_path))[0]
        safe_stem = re.sub(r"[^\w\u4e00-\u9fff.-]+", "_", stem)[:120]
        out_path = os.path.join(PROMAX_SPLITS_DIR, f"{safe_stem}.ad-extract.sgmodule")
        note = (
            f"从混合模块拆出的去广告行 {stats['kept_lines']}/{stats['total_lines']}；"
            f"解锁/增强仍留在原模块"
        )
        write_file(
            out_path,
            format_split_module(source_path, ad_sections, note=note),
        )
        Logger.info(
            f"  ↳ split artifact: {os.path.relpath(out_path, ROOT)} "
            f"({stats['kept_lines']}/{stats['total_lines']} lines)"
        )

    def load_functional_source_paths(self) -> Tuple[List[Tuple[str, str]], List[str]]:
        """Resolve module paths (full|split) + remote URLs for PROMAX functional merge."""
        paths: List[Tuple[str, str]] = []
        remote_urls: List[str] = []
        seen: Set[str] = set()
        self._functional_manifest_paths = set()

        def add(path: str, *, from_manifest: bool = False) -> None:
            norm = os.path.normpath(path)
            if norm in seen or not os.path.isfile(norm):
                return
            mode = self.resolve_module_ingest_mode(norm, from_manifest=from_manifest)
            if mode == "skip":
                Logger.warn(
                    f"  ⊘ Skipped (no ad / unlock-only): {os.path.relpath(norm, ROOT)}"
                )
                return
            if from_manifest:
                self._functional_manifest_paths.add(norm)
            seen.add(norm)
            paths.append((norm, mode))

        if os.path.isfile(ADBLOCK_FUNCTIONAL_SOURCES_FILE):
            for raw_line in read_file(ADBLOCK_FUNCTIONAL_SOURCES_FILE):
                line = raw_line.strip()
                if not line or line.startswith("#"):
                    continue
                if line.startswith(REMOTE_PREFIXES):
                    remote_urls.append(line)
                    continue
                resolved = os.path.normpath(
                    os.path.join(os.path.dirname(ADBLOCK_FUNCTIONAL_SOURCES_FILE), line)
                )
                add(resolved, from_manifest=True)

        if os.path.isdir(LOCAL_SOURCES_DIR):
            for name in sorted(os.listdir(LOCAL_SOURCES_DIR)):
                if name.endswith(MODULE_SUFFIXES):
                    add(os.path.join(LOCAL_SOURCES_DIR, name))

        for scan_dir in (LOCAL_MODULES_DIR,):
            if not os.path.isdir(scan_dir):
                continue
            for name in sorted(os.listdir(scan_dir)):
                if not name.endswith(MODULE_SUFFIXES) or name in PROTECTED_MODULES:
                    continue
                add(os.path.join(scan_dir, name))

        split_n = sum(1 for _, m in paths if m == "split")
        Logger.info(
            f"Resolved {len(paths)} functional module(s) "
            f"({split_n} mixed→line-split), {len(remote_urls)} remote URL(s)"
        )
        return paths, remote_urls

    def integrate_functional_sections(self) -> Dict[str, int]:
        """Merge Script / Rewrite / MITM from local + nexus + manifest into self.sections."""
        Logger.section("Integrating Functional Sections (Script / Rewrite / MITM)")
        counts_before = {name: len(self.sections[name]) for name in SECTION_NAMES}
        mitm_before = len(self.mitm_hosts)

        local_paths, remote_urls = self.load_functional_source_paths()
        for url in remote_urls:
            content = self._download(url)
            if not content:
                Logger.warn(f"  ⚠ Functional remote fetch failed: {url}")
                continue
            Logger.info(f"  + Functional remote: {url.rsplit('/', 1)[-1]}")
            self.extract_from_text(
                content,
                default_policy="REJECT",
                category="Local",
                include_sections=True,
                functional_mode=True,
                sections_only=True,
            )

        for path, ingest_mode in local_paths:
            tag = "split" if ingest_mode == "split" else "full"
            Logger.info(f"  + Functional ({tag}): {os.path.relpath(path, ROOT)}")
            self.extract_from_file(
                path,
                default_policy="REJECT",
                category="Local",
                include_sections=True,
                functional_mode=True,
                sections_only=True,
                promax_line_split=(ingest_mode == "split"),
            )
            if ingest_mode == "split":
                self.write_promax_split_artifact(path)

        added = {
            name: len(self.sections[name]) - counts_before[name] for name in SECTION_NAMES
        }
        added["MITM"] = len(self.mitm_hosts) - mitm_before
        summary = ", ".join(
            f"{name}={count}" for name, count in added.items() if count > 0
        )
        Logger.success(f"Functional sections merged: {summary or 'no new lines'}")
        return added

    def build_input_hashes(self, module_paths: List[str], source_entries: List[SourceEntry]) -> Dict[str, str]:
        hashes: Dict[str, str] = {}
        tracked_paths = {
            ADBLOCK_SOURCES_FILE,
            ADBLOCK_FUNCTIONAL_SOURCES_FILE,
            SKK_REJECT,
            SKK_HTTPDNS,
            WHITELIST_FILE,
        }
        tracked_paths.update(module_paths)
        for entry in source_entries:
            if not entry.is_remote:
                tracked_paths.add(entry.resolved_path)

        for path in sorted(tracked_paths):
            if os.path.exists(path):
                hashes[os.path.relpath(path, ROOT)] = get_file_hash(path)
        return hashes

    def is_module_source(self, entry: SourceEntry) -> bool:
        target = entry.source if entry.is_remote else entry.resolved_path
        return target.endswith(MODULE_SUFFIXES)

    def expand_complex_rule(self, line: str) -> List[str]:
        extracted = []
        if line.startswith("AND,((") or line.startswith("OR,(("):
            for match in re.findall(r"DOMAIN-SUFFIX,([^,\)\]]+)", line):
                suffix = match.strip()
                if suffix and "." in suffix:
                    extracted.append(f"DOMAIN-SUFFIX,{suffix}")
            for match in re.findall(r"DOMAIN-KEYWORD,-?([^,\)\]]+)", line):
                suffix = match.strip()
                if re.search(
                    r"\.(com|net|org|io|cn|jp|kr|tw|hk|sg|uk|de|fr|ru|br|in|au|co|me|tv|cc|xyz|top|app|dev)$",
                    suffix,
                    re.I,
                ):
                    extracted.append(f"DOMAIN-SUFFIX,{suffix}")
            return extracted

        if line.count("(") != line.count(")") or line.endswith(("DOMAIN-KEYWORD", "DOMAIN-SUFFIX", "DOMAIN")):
            return []
        return [line]

    def normalize_policy(self, line: str, default_policy: str) -> Optional[str]:
        match = re.search(
            r",(REJECT-NO-DROP|REJECT-DROP|REJECT-TINYGIF|REJECT-IMG|REJECT|DIRECT|PROXY)(?:,|$)",
            line,
        )
        policy = match.group(1) if match else default_policy
        if policy in {"REJECT-TINYGIF", "REJECT-IMG"}:
            return "REJECT"
        if policy == "PROXY":
            return None
        return policy

    def normalize_rule(self, raw_rule: str) -> Optional[str]:
        rule = raw_rule.strip()
        if not rule or rule.startswith("#") or rule.startswith("RULE-SET,"):
            return None

        rule = re.sub(r"\s+", " ", rule).strip()

        if not rule:
            return None

        if "," in rule:
            rule = strip_trailing_policy(rule)
        else:
            normalized_bare_rule = self.normalize_bare_rule(rule)
            if not normalized_bare_rule:
                return None
            rule = normalized_bare_rule

        if rule.startswith("IP-CIDR,") and "::" in rule:
            rule = rule.replace("IP-CIDR,", "IP-CIDR6,", 1)

        if rule.startswith(UNSAFE_IP_RULE_PREFIXES):
            return None

        if not rule.startswith(ALLOWED_RULE_PREFIXES):
            return None

        if rule.startswith("DOMAIN-REGEX,"):
            payload = rule.split(",", 1)[1].strip().strip('"')
            if " " in payload or is_invalid_domain_regex_payload(payload):
                return None
            return f'DOMAIN-REGEX,"{payload}"'

        if rule.startswith("URL-REGEX,"):
            payload = rule.split(",", 1)[1].strip().strip('"')
            return format_url_regex_for_surge(payload)

        return rule

    def normalize_bare_rule(self, rule: str) -> Optional[str]:
        stripped = rule.strip()
        if not stripped:
            return None

        # Support Adblock-style rules (e.g., ||domain.com^)
        if stripped.startswith("||") and stripped.endswith("^"):
            candidate = stripped[2:-1]
            if not candidate or "." not in candidate:
                return None
            return f"DOMAIN-SUFFIX,{candidate}"

        # Reject rewrite-like freeform lines instead of misclassifying them as DOMAIN rules.
        if self.infer_non_rule_section(stripped):
            return None

        # Accept hosts-file style entries such as "0.0.0.0 example.com".
        tokens = stripped.split()
        if len(tokens) >= 2:
            first_token, second_token = tokens[0], tokens[1]
            if self.try_render_ip_rule(first_token) and self.try_render_domain_rule(second_token):
                return self.try_render_domain_rule(second_token)
            return None

        return self.try_render_ip_rule(stripped) or self.try_render_domain_rule(stripped)

    def try_render_ip_rule(self, value: str) -> Optional[str]:
        candidate = value.strip().strip("[]")
        if not candidate:
            return None

        try:
            network = ipaddress.ip_network(candidate, strict=False)
        except ValueError:
            try:
                address = ipaddress.ip_address(candidate)
            except ValueError:
                return None
            prefix = 128 if address.version == 6 else 32
            network = ipaddress.ip_network(f"{address}/{prefix}", strict=False)

        if network.is_private or network.is_loopback or network.is_link_local:
            return None

        prefix = "IP-CIDR6" if network.version == 6 else "IP-CIDR"
        return f"{prefix},{network.with_prefixlen}"

    def try_render_domain_rule(self, value: str) -> Optional[str]:
        candidate = value.strip()
        if not candidate:
            return None

        is_suffix = False
        if candidate.startswith("*."):
            candidate = candidate[2:]
            is_suffix = True
        elif candidate.startswith("."):
            candidate = candidate[1:]
            is_suffix = True

        candidate = candidate.rstrip(".")
        if not candidate or "." not in candidate:
            return None

        if not re.fullmatch(r"[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+", candidate):
            return None

        prefix = "DOMAIN-SUFFIX" if is_suffix else "DOMAIN"
        return f"{prefix},{candidate}"

    @staticmethod
    def module_has_rule_section(text: str) -> bool:
        for line in text.splitlines():
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]") and not stripped.startswith("#"):
                name = stripped[1:-1]
                if name in MODULE_RULE_SECTIONS:
                    return True
        return False

    @staticmethod
    def is_script_or_rewrite_line(line: str) -> bool:
        """Surge script / rewrite / map-local lines must never become DOMAIN rules."""
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            return False
        lowered = stripped.lower()
        if any(
            token in lowered
            for token in (
                "script-path=",
                "requires-body=",
                "type=http-request",
                "type=http-response",
                "binary-body-mode=",
                "max-size=",
                "data-type=",
                "status-code=",
            )
        ):
            return True
        if stripped.startswith("^") and (" reject" in lowered or " 302" in lowered):
            return True
        if " _ reject" in lowered or " - reject" in lowered:
            return True
        return False

    def infer_non_rule_section(self, line: str) -> Optional[str]:
        if not line or line.startswith("#!"):
            return None

        stripped = line.strip()
        lowered = stripped.lower()

        if stripped.startswith("hostname"):
            return "MITM"
        if "data-type=" in lowered or "status-code=" in lowered:
            return "Map Local"
        if "script-path=" in lowered or "requires-body=" in lowered or "type=http-" in lowered:
            return "Script"
        if lowered.startswith(BODY_REWRITE_PREFIXES):
            return "Body Rewrite"
        if stripped.startswith("^") or " - reject" in lowered or " _ reject" in lowered or lowered.endswith(" 302"):
            return "URL Rewrite"
        return None

    def determine_category(self, source: str) -> str:
        lowered = source.lower()
        if "/source/local" in lowered or "localmodules" in lowered:
            return "Local"
        if "ultimate.txt" in lowered or "/ultimate" in lowered:
            return "ThreatIntel_Ultimate"
        if "tif.medium" in lowered or "/tif." in lowered:
            return "ThreatIntel_TIF"
        if "hagezi" in lowered or "reject_phishing" in lowered:
            return "ThreatIntel"
        if "anti-ad" in lowered:
            return "AntiAD"
        if any(kw in lowered for kw in ["privacy", "tracking", "tracking-protection"]):
            return "Privacy"
        if any(
            kw in lowered
            for kw in [
                "hijacking",
                "phishing",
                "threat",
                "banprogram",
                "reject.conf",
                "reject-url",
                "reject_extra",
                "blockhttpdns",
                "httpdns",
            ]
        ):
            return "Security"
        if any(kw in lowered for kw in ["advertising", "ads", "banad", "adblock", "adg.list"]):
            return "Advertising"
        return "Other"

    def _category_from_filename(self, filename: str) -> str:
        stem = filename.replace(".list", "")
        if not stem.startswith("AdBlock_"):
            return "Other"
        body = stem[len("AdBlock_") :]
        for cat in sorted(self.category_names, key=len, reverse=True):
            if body == cat or body.startswith(f"{cat}_"):
                return cat
        return "Other"

    @staticmethod
    def _shard_sort_key(path: str) -> Tuple[int, str]:
        name = os.path.basename(path)
        match = re.search(r"_(\d+)\.list$", name)
        return (int(match.group(1)) if match else 0, name)

    def add_rule_line(self, line: str, default_policy: str, category: str = "Other"):
        if not line or line.startswith("#"):
            return
        if self.is_script_or_rewrite_line(line):
            return
        for candidate in self.expand_complex_rule(line):
            policy = self.normalize_policy(candidate, default_policy)
            if not policy:
                continue
            normalized = self.normalize_rule(candidate)
            if not normalized:
                continue

            # CRITICAL: Apply deep whitelist filtering at ingestion
            rule_payload = normalized.split(",", 1)[-1] if "," in normalized else normalized
            if any(white_item in rule_payload for white_item in self.whitelist):
                continue

            rendered = self.render_policy_rule(normalized, policy, candidate)
            if not rendered:
                continue
            
            # Ensure the policy bucket exists
            if policy not in self.rules:
                self.rules[policy] = {cat: set() for cat in self.category_names}
                self.seen_rules[policy] = set()

            # CROSS-CATEGORY DEDUPLICATION
            # If we've already seen this rule in a previous (higher priority) category, skip it.
            if rendered in self.seen_rules[policy]:
                return
                
            bucket = self.rules[policy].get(category, self.rules[policy]["Other"])
            bucket.add(rendered)
            self.seen_rules[policy].add(rendered)

    def render_policy_rule(self, normalized_rule: str, policy: str, raw_rule: str) -> str:
        """Return the BARE normalized rule (TYPE,VALUE only).

        Policy is already tracked by the dict key in self.rules[policy],
        and options (extended-matching, pre-matching, no-resolve) belong
        only on the RULE-SET line inside the .sgmodule, NOT on individual
        rules stored in .list files.  Storing them here was the root cause
        of every 'Invalid line' parse error in Surge external rulesets.
        """
        return normalized_rule

    def add_section_line(self, section_name: str, line: str) -> None:
        """Append a non-rule section line with cross-source deduplication."""
        if section_name not in self.sections:
            return
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            return
        from hub.module_sanitizer import _dedupe_key

        key = _dedupe_key(section_name, stripped)
        if not key or key in self._section_seen[section_name]:
            return
        self._section_seen[section_name].add(key)
        self.sections[section_name].append(stripped)

    def is_enhancement_line(
        self,
        line: str,
        section: Optional[str] = None,
        *,
        source_path: Optional[str] = None,
    ) -> bool:
        """True if line is unlock/功能增强 (excluded from PROMAX)."""
        return (
            classify_promax_line(line, section, source_path=source_path) == "enhance"
        )

    def extract_from_text(
        self,
        text: str,
        default_policy: str = "REJECT",
        category: str = "Other",
        include_sections: bool = False,
        *,
        rules_only: bool = False,
        functional_mode: bool = False,
        sections_only: bool = False,
        promax_line_split: bool = True,
        source_path: Optional[str] = None,
    ):
        current_section = None
        in_rule_section = False
        seen_real_header = False

        for line in text.splitlines():
            stripped = line.strip()
            if not stripped:
                continue

            if stripped.startswith("[") and stripped.endswith("]") and not stripped.startswith("#"):
                current_section = stripped[1:-1]
                in_rule_section = current_section in MODULE_RULE_SECTIONS
                seen_real_header = True
                continue

            # Plain .list files may lack [Rule]; sgmodule sources must not guess rules from headers.
            if not seen_real_header:
                if rules_only or sections_only:
                    continue
                if self.is_script_or_rewrite_line(stripped):
                    continue
                if self.normalize_rule(stripped):
                    self.add_rule_line(stripped, default_policy, category=category)
                    continue

                if include_sections:
                    inferred_section = self.infer_non_rule_section(stripped)
                    if inferred_section in self.sections:
                        self.add_section_line(inferred_section, stripped)
                    elif inferred_section == "MITM" and stripped.startswith("hostname"):
                        hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", stripped)
                        self.mitm_hosts.update(host.strip() for host in hosts.split(",") if host.strip())
                continue

            if in_rule_section:
                if sections_only:
                    continue
                if self.is_script_or_rewrite_line(stripped):
                    continue
                if (
                    promax_line_split
                    and classify_promax_line(
                        stripped, "Rule", source_path=source_path
                    )
                    != "ad"
                ):
                    continue
                self.add_rule_line(stripped, default_policy, category=category)
                continue

            if rules_only or not include_sections or stripped.startswith("#"):
                continue

            if current_section in self.sections:
                # Rule section is already handled above, skip here to prevent duplication
                if current_section == "Rule":
                    continue
                if promax_line_split and not should_keep_promax_line(
                    stripped, current_section, source_path=source_path
                ):
                    continue
                self.add_section_line(current_section, stripped)
            elif current_section == "MITM" and stripped.startswith("hostname"):
                hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", stripped)
                self.mitm_hosts.update(host.strip() for host in hosts.split(",") if host.strip())

    def extract_from_file(
        self,
        path: str,
        default_policy: str = "REJECT",
        category: str = "Other",
        include_sections: bool = False,
        *,
        rules_only: Optional[bool] = None,
        functional_mode: bool = False,
        sections_only: bool = False,
        promax_line_split: bool = True,
    ):
        if not os.path.exists(path):
            return
        if rules_only is None:
            rules_only = path.endswith(MODULE_SUFFIXES) and not include_sections
        self.extract_from_text(
            "".join(read_file(path)),
            default_policy=default_policy,
            category=category,
            include_sections=include_sections,
            rules_only=rules_only,
            functional_mode=functional_mode,
            sections_only=sections_only,
            promax_line_split=promax_line_split,
            source_path=os.path.normpath(path),
        )

    def parse_module_structure(self, text: str) -> Tuple[List[str], List[Tuple[str, List[str]]]]:
        header_lines: List[str] = []
        sections: List[Tuple[str, List[str]]] = []
        current_name: Optional[str] = None
        current_lines: List[str] = []
        seen_section = False

        for raw_line in text.splitlines():
            stripped = raw_line.strip()
            if stripped.startswith("[") and stripped.endswith("]") and not stripped.startswith("#"):
                if current_name is not None:
                    sections.append((current_name, current_lines))
                current_name = stripped[1:-1]
                current_lines = []
                seen_section = True
                continue

            if not seen_section:
                header_lines.append(raw_line.rstrip())
            elif current_name is not None:
                current_lines.append(raw_line.rstrip())

        if current_name is not None:
            sections.append((current_name, current_lines))

        return header_lines, sections

    def sanitize_module_header(self, header_lines: List[str]) -> List[str]:
        cleaned = []
        name_line_idx = -1

        for line in header_lines:
            stripped = line.strip()
            if stripped.startswith("#!category="):
                continue  # We will inject our own
            if stripped.startswith("#!name="):
                name_line_idx = len(cleaned)
            
            if line.startswith("#!") or not stripped:
                cleaned.append(line)
        
        # Inject standard category
        category_line = f"#!category={GROUP_HEAD_EXPANSE}"
        if name_line_idx != -1:
            cleaned.insert(name_line_idx + 1, category_line)
        else:
            cleaned.insert(0, category_line)

        while cleaned and not cleaned[-1].strip():
            cleaned.pop()
        return cleaned

    def trim_section_lines(self, lines: List[str]) -> List[str]:
        trimmed = list(lines)
        while trimmed and not trimmed[0].strip():
            trimmed.pop(0)
        while trimmed and not trimmed[-1].strip():
            trimmed.pop()
        return trimmed

    def count_rules_in_section(self, lines: List[str], default_policy: str) -> int:
        count = 0
        for raw_line in lines:
            stripped = raw_line.strip()
            if not stripped or stripped.startswith("#") or stripped.startswith("RULE-SET,"):
                continue
            for candidate in self.expand_complex_rule(stripped):
                policy = self.normalize_policy(candidate, default_policy)
                normalized = self.normalize_rule(candidate)
                if policy and normalized:
                    count += 1
        return count

    def build_cleaned_module_content(self, text: str, default_policy: str) -> str:
        header_lines, sections = self.parse_module_structure(text)
        if not sections:
            return text.rstrip() + "\n"

        rule_count = 0
        non_rule_sections: List[Tuple[str, List[str]]] = []
        for section_name, section_lines in sections:
            if section_name == "Rule":
                rule_count += self.count_rules_in_section(section_lines, default_policy)
            else:
                non_rule_sections.append((section_name, section_lines))

        output = self.sanitize_module_header(header_lines)

        if not sections or rule_count == 0:
            if output:
                # Reconstruct content with sanitized header
                body = []
                for section_name, section_lines in sections:
                    body.append(f"[{section_name}]")
                    body.extend(section_lines)
                    body.append("")
                return "\n".join(output + [""] + body).rstrip() + "\n"
            return text.rstrip() + "\n"
        if output:
            output.append("")

        output.append(f"# NOTE: All {rule_count} rules from this module have been extracted to AdBlock.list")
        if non_rule_sections:
            output.append("# This cleaned version only contains non-Rule sections:")
            seen_sections = set()
            for section_name, _ in non_rule_sections:
                if section_name in seen_sections:
                    continue
                output.append(f"#   - [{section_name}]")
                seen_sections.add(section_name)
            output.append("# Use AdBlock.list for all blocking rules")
        else:
            output.append("# This module is kept synced, but all blocking rules now live in AdBlock.list")

        if non_rule_sections:
            output.append("")
            for index, (section_name, section_lines) in enumerate(non_rule_sections):
                output.append(f"[{section_name}]")
                output.extend(self.trim_section_lines(section_lines))
                if index != len(non_rule_sections) - 1:
                    output.append("")

        return "\n".join(output).rstrip() + "\n"

    def find_sync_targets(self, entry: SourceEntry) -> List[str]:
        if not self.is_module_source(entry):
            return []

        base_name = urllib.parse.unquote(os.path.basename(entry.source))
        
        # Build list of potential names by stripping known module suffixes and trying all
        stem = base_name
        for suffix in MODULE_SUFFIXES:
            if base_name.endswith(suffix):
                stem = base_name[:-len(suffix)]
                break
                
        candidates = {stem + suffix for suffix in MODULE_SUFFIXES}
        candidates.add(base_name)
        
        matches = set()
        for cand in candidates:
            matches.update(MODULE_TARGET_ALIASES.get(cand, []))

        for root_dir, _, filenames in os.walk(SURGE_MODULE_DIR):
            for cand in candidates:
                if cand in filenames:
                    matches.add(os.path.join(root_dir, cand))

        return sorted(matches)

    def sync_module_sources(self, source_entries: List[SourceEntry]) -> Tuple[Dict[str, str], List[str]]:
        cached_contents: Dict[str, str] = {}
        failures: List[str] = []

        for entry in source_entries:
            if not self.is_module_source(entry):
                continue
            # Remote modules with no local sync target: rules extracted later only.
            if entry.is_remote and not self.find_sync_targets(entry):
                continue

            Logger.info(f"Syncing upstream module: {entry.display_name}")
            if entry.is_remote:
                content = self._download(entry.source)
            else:
                if not os.path.exists(entry.resolved_path):
                    content = None
                else:
                    content = "".join(read_file(entry.resolved_path))

            if not content:
                Logger.warn(f"  ⚠ Failed module sync: {entry.display_name}")
                failures.append(entry.display_name)
                continue

            if entry.is_remote:
                cached_contents[entry.source] = content

            cleaned_content = self.build_cleaned_module_content(content, entry.policy)
            targets = self.find_sync_targets(entry)
            
            # If it's a local source, exclude itself from targets to avoid infinite cycle or overwrite
            if not entry.is_remote:
                targets = [t for t in targets if os.path.normpath(t) != os.path.normpath(entry.resolved_path)]

            if not targets:
                # If no targets found and it's remote, it might be a new module we should track
                # but for local it's fine
                if entry.is_remote:
                    Logger.warn(f"  ⚠ No local module target found for: {entry.display_name}")
                continue

            for target_path in targets:
                write_file(target_path, cleaned_content)
            Logger.success(f"  ✓ Synced {len(targets)} local module target(s)")

        return cached_contents, failures

    def process_source_entries(
        self,
        source_entries: List[SourceEntry],
        cached_contents: Optional[Dict[str, str]] = None,
    ) -> List[str]:
        cached_contents = cached_contents or {}
        failures: List[str] = []
        for entry in source_entries:
            category = self.determine_category(entry.source)
            if entry.kind == "domainset":
                if entry.is_remote:
                    self.add_rule_line(f"DOMAIN-SET,{entry.source}", entry.policy, category=category)
                    Logger.success(f"  ✓ domainset: {entry.display_name}")
                else:
                    Logger.warn(f"  ⚠ domainset sources must be remote: {entry.display_name}")
                    failures.append(entry.display_name)
                continue

            if self.is_module_source(entry):
                if entry.source in cached_contents:
                    mod_text = cached_contents[entry.source]
                elif entry.is_remote:
                    mod_text = self._download(entry.source)
                else:
                    mod_text = (
                        "".join(read_file(entry.resolved_path))
                        if os.path.exists(entry.resolved_path)
                        else ""
                    )
                if not mod_text:
                    Logger.warn(f"  ⚠ Failed: {entry.display_name}")
                    failures.append(entry.display_name)
                    continue
                if not self.module_has_rule_section(mod_text):
                    Logger.info(
                        f"  ⊘ Skip (no [Rule] section, script/rewrite-only): {entry.display_name}"
                    )
                    continue
                Logger.info(f"Fetching source: {entry.display_name} (Category: {category})")
                self.extract_from_text(
                    mod_text,
                    default_policy=entry.policy,
                    category=category,
                    include_sections=False,
                    rules_only=True,
                )
                Logger.success(f"  ✓ {entry.display_name}")
                continue

            Logger.info(f"Fetching source: {entry.display_name} (Category: {category})")
            if entry.source in cached_contents:
                content = cached_contents[entry.source]
            elif entry.is_remote:
                content = self._download(entry.source)
            else:
                if not os.path.exists(entry.resolved_path):
                    Logger.warn(f"  ⚠ Missing local source: {entry.display_name}")
                    failures.append(entry.display_name)
                    continue
                content = "".join(read_file(entry.resolved_path))

            if not content:
                Logger.warn(f"  ⚠ Failed: {entry.display_name}")
                failures.append(entry.display_name)
                continue

            self.extract_from_text(
                content,
                default_policy=entry.policy,
                category=category,
                include_sections=False,
                rules_only=False,
            )
            Logger.success(f"  ✓ {entry.display_name}")
        return failures

    def filter_rules(self, rules: Set[str]) -> List[str]:
        filtered = []
        for rule in sorted(rules):
            if any(whitelist_item in rule for whitelist_item in self.whitelist):
                continue
            filtered.append(rule)
        return filtered

    def generate_ruleset(self) -> List[str]:
        """Generate purpose-split rulesets; shard large categories (~100k rules/file)."""
        generated_files = []
        if not os.path.exists(ADBLOCK_DIR):
            os.makedirs(ADBLOCK_DIR)

        Logger.info(
            f"Generating split rulesets in {os.path.relpath(ADBLOCK_DIR, ROOT)} "
            f"(≤{RULES_PER_SHARD:,} rules/shard)"
        )

        for category in self.category_names:
            rules = self.filter_rules(self.rules["REJECT"][category])
            if not rules:
                Logger.info(
                    f"  ⊘ Skip shard for {category}: no unique rules "
                    f"(likely deduped by an earlier source, e.g. Ultimate before TIF)"
                )
                continue

            meta = CATEGORY_META[category]
            chunks = [
                rules[i : i + RULES_PER_SHARD]
                for i in range(0, len(rules), RULES_PER_SHARD)
            ]

            for idx, chunk in enumerate(chunks):
                if len(chunks) == 1:
                    filename = f"AdBlock_{category}.list"
                    shard_label = "1/1"
                else:
                    filename = f"AdBlock_{category}_{idx + 1:02d}.list"
                    shard_label = f"{idx + 1}/{len(chunks)}"

                filepath = os.path.join(ADBLOCK_DIR, filename)
                content = [
                    f"# Ruleset: AdBlock · {meta['label_zh']}\n",
                    f"# Purpose: {meta['desc']}\n",
                    f"# Category-Key: {category}\n",
                    f"# Shard: {shard_label}\n",
                    f"# Total Rules: {len(chunk)}\n",
                ]
                # Rules are already bare (TYPE,VALUE) — write directly
                content.extend(f"{rule}\n" for rule in chunk)
                write_file(filepath, "".join(content))

                rel_path = os.path.relpath(filepath, ROOT)
                generated_files.append(rel_path)
                Logger.success(f"  ✓ {filename} ({len(chunk):,} rules, {meta['label_zh']})")

        # Also update the legacy AdBlock.list with a notice or a subset
        legacy_content = [
            "# Ruleset: AdBlock (LEGACY / REDIRECT)\n",
            "# This file is now split into multiple files in rulesets/AdBlock/\n",
            "# Please update your module to use the new split rulesets.\n",
        ]
        # Include top 1000 rules just in case
        all_reject_rules = []
        for cat in self.category_names:
            all_reject_rules.extend(self.filter_rules(self.rules["REJECT"][cat]))
        
        legacy_content.extend(f"{rule}\n" for rule in all_reject_rules[:1000])
        write_file(ADBLOCK_LIST, "".join(legacy_content))
        self.prune_stale_rulesets(generated_files)

        return generated_files

    def prune_stale_rulesets(self, generated_files: List[str]) -> None:
        """Remove split-list files no longer referenced by the current rebuild."""
        if not os.path.isdir(ADBLOCK_DIR):
            return
        keep = {os.path.basename(path) for path in generated_files}
        removed = []
        for filename in sorted(os.listdir(ADBLOCK_DIR)):
            if not filename.endswith(".list") or filename in keep:
                continue
            file_path = os.path.join(ADBLOCK_DIR, filename)
            if safe_remove(file_path):
                removed.append(filename)
                srs_path = os.path.join(ROOT, "rulesets/SingBox", f"{os.path.splitext(filename)[0]}_Singbox.srs")
                if os.path.exists(srs_path):
                    safe_remove(srs_path)
        if removed:
            Logger.info(f"Pruned {len(removed)} stale AdBlock ruleset(s): {', '.join(removed)}")

    def write_catalog(self, generated_rulesets: List[str]) -> None:
        """Write machine-readable catalog + README for ruleset shards and module links."""
        now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        surge_module = os.path.relpath(TARGET_MODULE, ROOT)
        sr_module = surge_module.replace("surge", "shadowrocket").replace(".sgmodule", ".module")

        shards = []
        for rs_path in generated_rulesets:
            filename = os.path.basename(rs_path)
            category = self._category_from_filename(filename)
            meta = CATEGORY_META.get(category, CATEGORY_META["Other"])
            match = re.search(r"_(\d+)\.list$", filename)
            shard_index = int(match.group(1)) if match else 1
            rule_count = 0
            full_path = os.path.join(ROOT, rs_path)
            if os.path.isfile(full_path):
                with open(full_path, "r", encoding="utf-8", errors="ignore") as handle:
                    rule_count = sum(
                        1
                        for line in handle
                        if line.strip() and not line.strip().startswith("#")
                    )
            encoded = urllib.parse.quote(rs_path)
            shards.append(
                {
                    "file": rs_path,
                    "category": category,
                    "label_zh": meta["label_zh"],
                    "purpose": meta["desc"],
                    "shard_index": shard_index,
                    "rule_count": rule_count,
                    "cdn_url": f"{CDN_BASE_URL}{encoded}",
                }
            )

        surge_module_lite = os.path.relpath(TARGET_MODULE_LITE, ROOT)
        sr_module_lite = surge_module_lite.replace("surge", "shadowrocket").replace(".sgmodule", ".module")

        catalog = {
            "updated": now,
            "rules_per_shard_max": RULES_PER_SHARD,
            "categories": CATEGORY_META,
            "lite_categories": sorted(LITE_CATEGORIES),
            "modules": {
                "surge_promax": {
                    "path": surge_module,
                    "install_url": f"{CDN_BASE_URL}{urllib.parse.quote(surge_module)}",
                },
                "shadowrocket_promax": {
                    "path": sr_module,
                    "install_url": f"https://raw.githubusercontent.com/nowaytouse/script_hub/master/{urllib.parse.quote(sr_module)}",
                },
                "surge_promax_lite": {
                    "path": surge_module_lite,
                    "install_url": f"{CDN_BASE_URL}{urllib.parse.quote(surge_module_lite)}",
                    "note": "手机轻量版 — 不含 ThreatIntel 重型规则",
                },
                "shadowrocket_promax_lite": {
                    "path": sr_module_lite,
                    "install_url": f"https://raw.githubusercontent.com/nowaytouse/script_hub/master/{urllib.parse.quote(sr_module_lite)}",
                    "note": "手机轻量版 — 不含 ThreatIntel 重型规则",
                },
            },
            "manifest": os.path.relpath(ADBLOCK_SOURCES_FILE, ROOT),
            "functional_manifest": os.path.relpath(ADBLOCK_FUNCTIONAL_SOURCES_FILE, ROOT),
            "functional_sections": dict(self.functional_stats),
            "shards": shards,
        }
        write_file(ADBLOCK_CATALOG_JSON, json.dumps(catalog, ensure_ascii=False, indent=2) + "\n")

        lines = [
            "# AdBlock 分片规则集\n",
            "\n",
            f"> 自动生成于 `{now}` · 每片 ≤ {RULES_PER_SHARD:,} 条 · 详见 [`catalog.json`](catalog.json)\n",
            "\n",
            "## 用途分类\n",
            "\n",
            "| 分类 | 说明 |\n",
            "|------|------|\n",
        ]
        for key, meta in CATEGORY_META.items():
            lines.append(f"| **{meta['label_zh']}** (`{key}`) | {meta['desc']} |\n")
        lines.extend(
            [
                "\n",
                "## 当前分片\n",
                "\n",
                "| 用途 | 文件 | 规则数 | CDN |\n",
                "|------|------|--------|-----|\n",
            ]
        )
        for item in shards:
            lines.append(
                f"| {item['label_zh']} | `{item['file']}` | {item['rule_count']:,} | [link]({item['cdn_url']}) |\n"
            )
        lines.extend(
            [
                "\n",
                "## 应用内脚本层（PROMAX 内嵌）\n",
                "\n",
            ]
        )
        if self.functional_stats:
            lines.append("| 段落 | 行数 |\n|------|------|\n")
            for sec_name in SECTION_NAMES:
                count = self.functional_stats.get(sec_name, 0)
                if count:
                    lines.append(f"| `{sec_name}` | {count:,} |\n")
            mitm_n = self.functional_stats.get("MITM", 0)
            if mitm_n:
                lines.append(f"| `MITM` (hostname) | {mitm_n:,} |\n")
        lines.extend(
            [
                "\n",
                "## 安装模块（RULE-SET + 去广告 Script/Rewrite；解锁/增强请用 Amplify Nexus 独立模块）\n",
                "\n",
                "`rulesets/Sources/LocalModules/*` 各 App 去广告逐文件并入 PROMAX，"
                "**勿单独安装**专项模块或已废弃的 narrow_pierce 副本。\n",
                "\n",
                "### 🖥️ 桌面完整版（Full）\n",
                f"- **Surge PROMAX**: [{catalog['modules']['surge_promax']['install_url']}]({catalog['modules']['surge_promax']['install_url']})\n",
                f"- **Shadowrocket PROMAX**: [{catalog['modules']['shadowrocket_promax']['install_url']}]({catalog['modules']['shadowrocket_promax']['install_url']})\n",
                "\n",
                "### 📱 手机轻量版（Lite）\n",
                f"- **Surge PROMAX Lite**: [{catalog['modules']['surge_promax_lite']['install_url']}]({catalog['modules']['surge_promax_lite']['install_url']})\n",
                f"- **Shadowrocket PROMAX Lite**: [{catalog['modules']['shadowrocket_promax_lite']['install_url']}]({catalog['modules']['shadowrocket_promax_lite']['install_url']})\n",
            ]
        )
        write_file(ADBLOCK_README, "".join(lines))
        Logger.success(f"Catalog written: {os.path.relpath(ADBLOCK_CATALOG_JSON, ROOT)}")

    @staticmethod
    def _attach_policy(rule: str, policy: str) -> str:
        """Attach policy to a bare rule for inline module output.

        Strips any stale policy / options that might have leaked through,
        then appends the correct policy.
        """
        strip_values = {
            "REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT", "PROXY",
            "REJECT-TINYGIF", "REJECT-IMG",
            "EXTENDED-MATCHING", "PRE-MATCHING", "NO-RESOLVE",
        }
        parts = [p.strip() for p in rule.split(",") if p.strip()]
        while parts and parts[-1].upper() in strip_values:
            parts.pop()
        parts.append(policy)
        return ",".join(parts)

    def generate_module(self, generated_rulesets: List[str], lite_only: bool = False):
        current_date = datetime.now().strftime("%Y-%m-%d")
        complex_prefixes = ("URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,")

        if lite_only:
            # The user explicitly wants to KEEP privacy, threat intel, etc., in Lite.
            active_rulesets = generated_rulesets
            target_path = TARGET_MODULE_LITE
        else:
            active_rulesets = generated_rulesets
            target_path = TARGET_MODULE

        shard_count = len(active_rulesets)

        rule_lines: List[str] = [
            "# --- SYNCED PORT RULES START ---",
            "# Automated update from ports source",
            "# --- SYNCED PORT RULES END ---",
            "",
            "# 自建防火墙规则 (已注销，拦截功能交由上游列表处理)",
            "# DEST-PORT,102,REJECT-DROP    # ISO-TSAP / Siemens S7 / IEC 61850 MMS（Stuxnet 目标协议）",
            "# DEST-PORT,1025,REJECT-DROP   # 常被 Windows RAT 占用",
            "# DEST-PORT,10250,REJECT-DROP  # Kubelet API（常被横向移动利用）",
            "# DEST-PORT,10255,REJECT-DROP  # Kubelet 只读端口（信息泄露）",
            "# DEST-PORT,1026,REJECT-DROP   # Windows messenger spam / RAT",
            "# DEST-PORT,1027,REJECT-DROP   # Windows messenger spam",
            "# DEST-PORT,111,REJECT-DROP    # RPC Portmapper / rpcbind（NFS 依赖）",
            "# DEST-PORT,11211,REJECT-DROP  # Memcached（DDoS 放大攻击源）",
            "# DEST-PORT,1234,REJECT-DROP   # LLDB 远程调试默认端口",
            "# DEST-PORT,12345,REJECT-DROP",
            "# DEST-PORT,12346,REJECT-DROP",
            "# DEST-PORT,1243,REJECT-DROP   # SubSeven variant",
            "# DEST-PORT,13,REJECT-DROP     # Daytime（信息泄露 + 反射源）",
            "# DEST-PORT,1337,REJECT-DROP   # 黑客文化端口，常被 RAT/WebShell 用",
            "# DEST-PORT,135,REJECT-DROP     # RPC Endpoint Mapper / DCOM",
            "# DEST-PORT,137,REJECT-DROP     # NetBIOS Name Service",
            "# DEST-PORT,138,REJECT-DROP     # NetBIOS Datagram",
            "# DEST-PORT,139,REJECT-DROP     # NetBIOS Session / SMBv1",
            "# DEST-PORT,1433,REJECT-DROP   # Microsoft SQL Server",
            "# DEST-PORT,1434,REJECT-DROP   # MS SQL Server Browser（UDP 扫描源）",
            "# DEST-PORT,14444,REJECT-DROP  # XMR 矿池常用",
            "# DEST-PORT,1521,REJECT-DROP   # Oracle Database",
            "# DEST-PORT,15672,REJECT-DROP  # RabbitMQ 管理后台",
            "# DEST-PORT,161,REJECT-DROP    # SNMP UDP 查询",
            "# DEST-PORT,162,REJECT-DROP    # SNMP Trap",
            "# DEST-PORT,16660,REJECT-DROP",
            "# DEST-PORT,16686,REJECT-DROP  # Jaeger UI（分布式追踪）",
            "# DEST-PORT,17,REJECT-DROP     # QOTD - Quote of the Day（放大 ~140x）",
            "# DEST-PORT,18080,REJECT-DROP  # Spark History Server",
            "# DEST-PORT,1883,REJECT-DROP   # MQTT 明文（无认证暴露极危险）",
            "# DEST-PORT,19,REJECT-DROP     # CharGen（放大 ~360x，经典 DRDoS 源）",
            "# DEST-PORT,1900,REJECT-DROP   # SSDP Discovery（UPnP）",
            "# DEST-PORT,1911,REJECT-DROP   # Niagara Fox（楼控系统，Shodan 大量暴露）",
            "# DEST-PORT,194,REJECT-DROP    # IRC 官方分配端口",
            "# DEST-PORT,1999,REJECT-DROP   # TransScout",
            "# DEST-PORT,20000,REJECT-DROP  # DNP3 over TCP（电力系统 SCADA）",
            "# DEST-PORT,2049,REJECT-DROP   # NFS",
            "# DEST-PORT,20535,REJECT-DROP  # 矿池备用",
            "# DEST-PORT,2181,REJECT-DROP   # ZooKeeper（Kafka 依赖）",
            "# DEST-PORT,2375,REJECT-DROP   # Docker API（无 TLS，高危）",
            "# DEST-PORT,2376,REJECT-DROP   # Docker API（TLS，但仍应内网）",
            "# DEST-PORT,2377,REJECT-DROP   # Docker Swarm 管理",
            "# DEST-PORT,2379,REJECT-DROP   # etcd client（K8s 核心存储）",
            "# DEST-PORT,2380,REJECT-DROP   # etcd peer",
            "# DEST-PORT,2404,REJECT-DROP   # IEC 60870-5-104（变电站远动协议）",
            "# DEST-PORT,27017,REJECT-DROP  # MongoDB",
            "# DEST-PORT,27018,REJECT-DROP  # MongoDB shard",
            "# DEST-PORT,27019,REJECT-DROP  # MongoDB config server",
            "# DEST-PORT,27374,REJECT-DROP",
            "# DEST-PORT,27444,REJECT-DROP",
            "# DEST-PORT,27665,REJECT-DROP",
            "# DEST-PORT,3000,REJECT-DROP   # Grafana / Node Server (⚠️ DevOps users comment this)",
            "# DEST-PORT,3032,REJECT-DROP   # NiceHash Stratum",
            "# DEST-PORT,3050,REJECT-DROP   # Firebird Database",
            "# DEST-PORT,3092,REJECT-DROP   # MiningPoolHub Stratum",
            "# DEST-PORT,31335,REJECT-DROP",
            "# DEST-PORT,31337,REJECT-DROP",
            "# DEST-PORT,31338,REJECT-DROP",
            "# DEST-PORT,3150,REJECT-DROP   # Deep Throat variant",
            "# DEST-PORT,3283,REJECT-DROP   # Apple Remote Desktop / ARMS",
            "# DEST-PORT,3306,REJECT-DROP   # MySQL / MariaDB",
            "# DEST-PORT,33270,REJECT-DROP",
            "# DEST-PORT,3333,REJECT-DROP   # Stratum 最常用端口（ETH/ETC/XMR）",
            "# DEST-PORT,34980,REJECT-DROP  # EtherNet/IP implicit messaging",
            "# DEST-PORT,37,REJECT-DROP     # Time Protocol（明文，可反射）",
            "# DEST-PORT,3702,REJECT-DROP   # WS-Discovery（Windows 设备发现，放大 ~500x）",
            "# DEST-PORT,389,REJECT-DROP    # LDAP 明文（同时防凭证嗅探）",
            "# DEST-PORT,4001,REJECT-DROP   # etcd 旧版 API",
            "# DEST-PORT,4040,REJECT-DROP   # Spark UI / ngrok 本地端口",
            "# DEST-PORT,40421,REJECT-DROP  # Masters Paradise",
            "# DEST-PORT,4045,REJECT-DROP    # Windows Lockd（NFS lock manager）",
            "# DEST-PORT,4444,REJECT-DROP   # Metasploit meterpreter 默认监听",
            "# DEST-PORT,4445,REJECT-DROP   # Metasploit 变体",
            "# DEST-PORT,445,REJECT-DROP     # SMB（WannaCry / NotPetya 主战场）",
            "# DEST-PORT,44818,REJECT-DROP  # EtherNet/IP（Rockwell/Allen-Bradley PLC）",
            "# DEST-PORT,45700,REJECT-DROP  # Stratum 高端口",
            "# DEST-PORT,4659,REJECT-DROP   # Xsan / StorNext CVFS",
            "# DEST-PORT,47808,REJECT-DROP  # BACnet（楼宇自动化，UDP/TCP）",
            "# DEST-PORT,4789,REJECT-DROP   # Docker overlay network VXLAN",
            "# DEST-PORT,4840,REJECT-DROP   # OPC-UA（工业自动化标准协议）",
            "# DEST-PORT,4843,REJECT-DROP   # OPC-UA over TLS",
            "# DEST-PORT,4905,REJECT-DROP   # Silver Sparrow v2 C2 观测端口",
            "# DEST-PORT,4911,REJECT-DROP   # Niagara Fox over TLS",
            "# DEST-PORT,49152,REJECT-DROP  # UPnP SOAP（Broadband Forum TR-064 漏洞）",
            "# DEST-PORT,49153,REJECT-DROP  # UPnP 动态端口",
            "# DEST-PORT,49154,REJECT-DROP  # UPnP 动态端口",
            "# DEST-PORT,49475,REJECT-DROP  # macOS 后门高端口（ESET/SentinelOne 威胁情报）",
            "# DEST-PORT,50050,REJECT-DROP  # Cobalt Strike teamserver",
            "# DEST-PORT,50051,REJECT-DROP  # Cobalt Strike 变体",
            "# DEST-PORT,50070,REJECT-DROP  # Hadoop HDFS NameNode WebUI",
            "# DEST-PORT,50075,REJECT-DROP  # Hadoop HDFS DataNode",
            "# DEST-PORT,502,REJECT-DROP    # Modbus TCP（全球 Shodan 暴露数量最多的 SCADA 协议）",
            "# DEST-PORT,503,REJECT-DROP    # Modbus TCP 备用",
            "# DEST-PORT,5223,REJECT-DROP   # APNs 端口（被伪装成 Apple Push 的 macOS RAT 使用）",
            "# DEST-PORT,5353,REJECT-DROP   # mDNS（可被用于反射放大）",
            "# DEST-PORT,5355,REJECT-DROP",
            "# DEST-PORT,5432,REJECT-DROP   # PostgreSQL",
            "# DEST-PORT,54320,REJECT-DROP  # Back Orifice 2000",
            "# DEST-PORT,54321,REJECT-DROP  # Back Orifice 2000",
            "# DEST-PORT,548,REJECT-DROP    # AFP over TCP",
            "# DEST-PORT,554,REJECT-DROP    # RTSP（IP 摄像头流媒体，常被未授权访问）",
            "# DEST-PORT,5554,REJECT-DROP   # Sasser 蠕虫",
            "# DEST-PORT,5555,REJECT-DROP   # Stratum 备用",
            "# DEST-PORT,5569,REJECT-DROP   # Robo-Hack",
            "# DEST-PORT,5601,REJECT-DROP   # Kibana（Elasticsearch 可视化，常无认证）",
            "# DEST-PORT,5672,REJECT-DROP   # RabbitMQ AMQP",
            "# DEST-PORT,5683,REJECT-DROP   # CoAP UDP",
            "# DEST-PORT,593,REJECT-DROP     # RPC over HTTP",
            "# DEST-PORT,5984,REJECT-DROP   # CouchDB",
            "# DEST-PORT,60000,REJECT-DROP  # Sliver C2 默认",
            "# DEST-PORT,61466,REJECT-DROP  # Telecommando",
            "# DEST-PORT,61616,REJECT-DROP  # ActiveMQ Broker（常被用于 RCE）",
            "# DEST-PORT,61617,REJECT-DROP  # ActiveMQ SSL Broker",
            "# DEST-PORT,6379,REJECT-DROP   # Redis（未授权访问高危）",
            "# DEST-PORT,6380,REJECT-DROP   # Redis TLS / 变体",
            "# DEST-PORT,6443,REJECT-DROP   # Kubernetes API Server",
            "# DEST-PORT,65000,REJECT-DROP",
            "# DEST-PORT,6667,REJECT-DROP   # IRC 标准端口，僵尸网络 C2 经典信道",
            "# DEST-PORT,6668,REJECT-DROP   # IRC 变体",
            "# DEST-PORT,6669,REJECT-DROP   # IRC 变体",
            "# DEST-PORT,6670,REJECT-DROP",
            "# DEST-PORT,6711,REJECT-DROP",
            "# DEST-PORT,6712,REJECT-DROP",
            "# DEST-PORT,6713,REJECT-DROP",
            "# DEST-PORT,6771,REJECT-DROP",
            "# DEST-PORT,7,REJECT-DROP      # Echo（放大 ~2x，但量大时危险）",
            "# DEST-PORT,7000,REJECT-DROP   # IRC 备用，僵尸网络常用",
            "# DEST-PORT,7474,REJECT-DROP   # Neo4j HTTP",
            "# DEST-PORT,7547,REJECT-DROP   # TR-069 HTTP",
            "# DEST-PORT,7548,REJECT-DROP   # TR-069 HTTPS",
            "# DEST-PORT,7687,REJECT-DROP   # Neo4j Bolt",
            "# DEST-PORT,7777,REJECT-DROP   # Stratum 备用",
            "# DEST-PORT,8032,REJECT-DROP   # Hadoop YARN ResourceManager",
            "# DEST-PORT,8076,REJECT-DROP",
            "# DEST-PORT,8082,REJECT-DROP   # InfluxDB RPC",
            "# DEST-PORT,8086,REJECT-DROP   # InfluxDB HTTP",
            "# DEST-PORT,8161,REJECT-DROP   # ActiveMQ Web Console（默认无认证）",
            "# DEST-PORT,8333,REJECT-DROP   # Bitcoin P2P（同时被矿机用）",
            "# DEST-PORT,8500,REJECT-DROP   # Consul HTTP API",
            "# DEST-PORT,8554,REJECT-DROP   # RTSP 备用端口",
            "# DEST-PORT,8567,REJECT-DROP   # macOS 恶意软件 C2 高频观测端口",
            "# DEST-PORT,8600,REJECT-DROP   # Consul DNS",
            "# DEST-PORT,8883,REJECT-DROP   # MQTT over TLS（仍不应出站到未知服务器）",
            "# DEST-PORT,8888,REJECT-DROP   # Jupyter Notebook（常无 Token 暴露，可执行任意代码）",
            "# DEST-PORT,8889,REJECT-DROP   # Jupyter 备用端口",
            "# DEST-PORT,9,REJECT-DROP      # Discard（无意义，常被扫描）",
            "# DEST-PORT,9000,REJECT-DROP   # Portainer / SonarQube / MinIO（多种危险服务共用）",
            "# DEST-PORT,9090,REJECT-DROP   # Prometheus UI",
            "# DEST-PORT,9091,REJECT-DROP   # Prometheus Pushgateway",
            "# DEST-PORT,9092,REJECT-DROP   # Apache Kafka",
            "# DEST-PORT,9100,REJECT-DROP   # RAW 打印 / JetDirect（可任意读写文件系统）",
            "# DEST-PORT,9200,REJECT-DROP   # Elasticsearch HTTP API",
            "# DEST-PORT,9300,REJECT-DROP   # Elasticsearch transport",
            "# DEST-PORT,9411,REJECT-DROP   # Zipkin（分布式追踪）",
            "# DEST-PORT,9418,REJECT-DROP   # Git daemon（明文 Git 协议，无认证可 clone/push）",
            "# DEST-PORT,9898,REJECT-DROP   # Dabber worm",
            "# DEST-PORT,9996,REJECT-DROP   # Sasser 蠕虫 FTP 传输",
            "# DEST-PORT,9999,REJECT-DROP   # Stratum 备用",
            "",
            "# High-Priority White-list (Prevent DNS failure)",
            "DOMAIN,dns.alidns.com,DIRECT",
            "DOMAIN,doh.pub,DIRECT",
            "DOMAIN,dns.pub,DIRECT",
            "DOMAIN,dot.pub,DIRECT",
            "DOMAIN,doh.360.cn,DIRECT",
            "DOMAIN,doh.dns.apple.com,DIRECT",
            "# Block app-layer HTTPDNS first",
            f"RULE-SET,{CDN_BASE_URL}rulesets/list/HTTPDNS_Hijack.list,REJECT",
            "# Split REJECT Rulesets (purpose-grouped; see rulesets/AdBlock/README.md)",
        ]

        shards_by_category: Dict[str, List[str]] = {}
        for rs_path in active_rulesets:
            cat = self._category_from_filename(os.path.basename(rs_path))
            shards_by_category.setdefault(cat, []).append(rs_path)

        for category in self.category_names:
            paths = shards_by_category.get(category)
            if not paths:
                continue
            meta = CATEGORY_META[category]
            rule_lines.append(f"# ── {meta['label_zh']} · {meta['desc']} ──")
            for rs_path in sorted(paths, key=self._shard_sort_key):
                encoded_path = urllib.parse.quote(rs_path)
                rule_lines.append(
                    f"RULE-SET,{CDN_BASE_URL}{encoded_path},REJECT,"
                    "extended-matching,pre-matching,update-interval=86400,no-resolve"
                )

        rule_lines.extend(
            [
                "",
                "# SKK Upstream Rulesets",
                f"RULE-SET,{CDN_BASE_URL}rulesets/list/skk_upstream/reject-no-drop.list,"
                "REJECT-NO-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve",
                f"RULE-SET,{CDN_BASE_URL}rulesets/list/skk_upstream/reject-drop.list,"
                "REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve",
                f"RULE-SET,{CDN_BASE_URL}rulesets/list/skk_upstream/BlockHttpDNS.list,"
                "REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve",
            ]
        )

        active_cats = set(self.category_names)
        complex_seen: Set[str] = set()
        for policy in ("REJECT", "REJECT-DROP", "REJECT-NO-DROP"):
            pool = set()
            for cat in active_cats:
                pool.update(self.rules[policy].get(cat, set()))
            if not pool:
                continue
            if policy == "REJECT":
                candidates = [r for r in pool if any(r.startswith(p) for p in complex_prefixes)]
            else:
                candidates = list(pool)
            if not candidates:
                continue
            rule_lines.append(f"# Merged {policy} complex rules ({len(candidates)})")
            for raw in sorted(candidates):
                line = self._attach_policy(raw, policy)
                if line in complex_seen:
                    continue
                complex_seen.add(line)
                rule_lines.append(line)

        functional_sections: List[Tuple[str, List[str]]] = []
        func_counts: List[str] = []
        for sec_name in SECTION_NAMES:
            lines = self.sections.get(sec_name, [])
            if not lines:
                continue
            deduped = dedupe_section_lines(sec_name, lines)
            if deduped:
                functional_sections.append((sec_name, deduped))
                func_counts.append(f"{sec_name}={len(deduped)}")
        if self.mitm_hosts:
            functional_sections.append(
                (
                    "MITM",
                    [f"hostname = %APPEND% {', '.join(sorted(self.mitm_hosts))}"],
                )
            )
            func_counts.append(f"MITM={len(self.mitm_hosts)}")

        if lite_only:
            all_sections = [("Rule", rule_lines)]
            func_summary = "无脚本/重写/MITM"
            name = f"📱 Universal Ad-Blocking Rules (PROMAX Lite) - [{current_date}]"
            desc = f"轻量基础版({shard_count}分片); 保留完整规则集与防火墙，纯净无脚本无MITM，性能拉满"
            tag = "AdBlock, Lite, Basic, Mobile"
        else:
            all_sections = [("Rule", rule_lines)] + functional_sections
            all_sections = merge_mitm_hosts(all_sections)
            func_summary = ", ".join(func_counts) if func_counts else "无脚本层"
            name = f"🚫 Universal Ad-Blocking Rules (PROMAX) - [{current_date}]"
            desc = (
                f"按用途分片({shard_count}片) + 应用内去广告({func_summary}); "
                f"索引 rulesets/AdBlock/catalog.json"
            )
            tag = "AdBlock, Dependency, HTTPDNS, Script"

        header_meta = {
            "name": name, "desc": desc,
            "author": "ScriptHub-Automated",
            "icon": "https://raw.githubusercontent.com/luestr/IconResource/main/Other_icon/120px/KeLee.png",
            "category": GROUP_HEAD_EXPANSE, "tag": tag,
            "date": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        }
        write_file(target_path, format_module(format_header(header_meta), all_sections, dedupe=False))
        Logger.success(f"Module generated: {os.path.basename(target_path)}")

    def sync_shadowrocket_promax_modules(self) -> None:
        """Regenerate SR PROMAX / PROMAX Lite from Surge after functional merge."""
        Logger.section("Syncing Shadowrocket PROMAX Modules")
        script = os.path.join(ROOT, "scripts", "tools", "convert_surge_to_shadowrocket.py")
        if not os.path.isfile(script):
            Logger.warn("convert_surge_to_shadowrocket.py not found; skipping SR sync")
            return
        result = subprocess.run(
            [sys.executable, script, "--promax-only"],
            cwd=ROOT,
        )
        if result.returncode != 0:
            Logger.warn("Shadowrocket PROMAX conversion exited with errors")
        else:
            Logger.success("Shadowrocket PROMAX modules refreshed")

    def merge(self, execute: bool = False):
        Logger.section("AdBlock Module Consolidation (Canonical Rebuild)")
        self.load_whitelist()
        self.load_hashes()
        self.cleanup_local_lists()

        # 1. Load source entries and sort them by category priority
        source_entries = self.load_source_entries()
        
        # Define priority map for sorting
        priority_map = {cat: i for i, cat in enumerate(self.category_names)}
        source_entries.sort(key=lambda e: priority_map.get(self.determine_category(e.source), 99))

        # 2. Sync upstream module sources (extract non-rule sections)
        Logger.section("Syncing Upstream Ad Modules")
        cached_modules, sync_failures = self.sync_module_sources(source_entries)

        # 3. Local sources + nexus ad modules → rules (sections merged later in integrate_functional_sections)
        Logger.section("Integrating Local AdBlock Sources")
        if os.path.isdir(LOCAL_SOURCES_DIR):
            for f in sorted(os.listdir(LOCAL_SOURCES_DIR)):
                if f.endswith(MODULE_SUFFIXES):
                    path = os.path.join(LOCAL_SOURCES_DIR, f)
                    Logger.info(f"  + Rules from local source: {f}")
                    self.extract_from_file(
                        path,
                        default_policy="REJECT",
                        category="Local",
                        include_sections=False,
                        rules_only=True,
                    )

        Logger.section("Integrating App AdBlock Rules (LocalModules)")
        if os.path.isdir(LOCAL_MODULES_DIR):
            for f in sorted(os.listdir(LOCAL_MODULES_DIR)):
                if not f.endswith(MODULE_SUFFIXES):
                    continue
                path = os.path.join(LOCAL_MODULES_DIR, f)
                mode = self.resolve_module_ingest_mode(path)
                if mode == "skip":
                    continue
                Logger.info(f"  + Rules from LocalModules ({mode}): {f}")
                self.extract_from_file(
                    path,
                    default_policy="REJECT",
                    category="Local",
                    include_sections=False,
                    rules_only=True,
                    promax_line_split=True,
                )

        # 4. Fetch and extract from all other canonical sources (sorted by priority)
        Logger.section("Fetching Canonical AdBlock Sources")
        failures = self.process_source_entries(source_entries, cached_contents=cached_modules)

        generated_rulesets = self.generate_ruleset()
        self.functional_stats = self.integrate_functional_sections()
        self.write_catalog(generated_rulesets)
        self.generate_module(generated_rulesets, lite_only=False)
        Logger.info("Generating PROMAX Lite (mobile-friendly) module...")
        self.generate_module(generated_rulesets, lite_only=True)
        self.sync_shadowrocket_promax_modules()
        # Save hashes for change tracking
        local_source_paths = [e.resolved_path for e in source_entries if not e.is_remote]
        self.save_hashes(self.build_input_hashes(local_source_paths, source_entries))

        if sync_failures or failures:
            Logger.warn(
                f"AdBlock merge completed with {len(sync_failures) + len(failures)} failures. Check logs for details."
            )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    AdBlockManager().merge(execute=args.execute)
