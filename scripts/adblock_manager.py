#!/usr/bin/env python3
import argparse
import ipaddress
import os
import re
import subprocess
import urllib.parse
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, List, Optional, Set, Tuple

from lib.common import Logger, get_file_hash, get_project_root, read_file, write_file

# ============================================================================
# CONFIGURATION
# ============================================================================

ROOT = get_project_root()
SURGE_MODULE_DIR = os.path.join(ROOT, "module/surge(main)")
HEAD_EXPANSE_DIR = os.path.join(ROOT, "module/surge(main)/head_expanse")
CACHE_DIR = os.path.join(ROOT, ".cache")
HASH_FILE = os.path.join(CACHE_DIR, "adblock_hashes.txt")
WHITELIST_FILE = os.path.join(ROOT, "ruleset/Sources/adblock_whitelist.txt")
ADBLOCK_SOURCES_FILE = os.path.join(ROOT, "ruleset/Sources/Links/AdBlock_sources.txt")
TARGET_MODULE = os.path.join(
    HEAD_EXPANSE_DIR,
    "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
)
NARROW_PIERCE_DIR = os.path.join(ROOT, "module/surge(main)/narrow_pierce")
ADBLOCK_DIR = os.path.join(ROOT, "ruleset/AdBlock")
ADBLOCK_LIST = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/AdBlock.list")
SKK_UPSTREAM_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/skk_upstream")
SKK_REJECT = os.path.join(SKK_UPSTREAM_DIR, "reject.list")
SKK_HTTPDNS = os.path.join(SKK_UPSTREAM_DIR, "BlockHttpDNS.list")
FIREWALL_MODULE = os.path.join(HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.sgmodule")

GROUP_HEAD_EXPANSE = "『 🔝 Head Expanse › 首端扩域 』"
MODULE_SUFFIXES = (".sgmodule", ".module")
REMOTE_PREFIXES = ("http://", "https://")
SECTION_NAMES = ("URL Rewrite", "Map Local", "Script", "Body Rewrite", "Header Rewrite")
PROTECTED_MODULES = ["🌐 DNS & Host Enhanced.sgmodule"]
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
INVALID_DOMAIN_REGEX_VALUES = {"", "$", ",", "-", ".", "2", "6", "]"}
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
        # rules[policy][category] = set(rules)
        # Priority: Local > Advertising > Privacy > Security > AntiAD > Hagezi > Other
        self.category_names = [
            "Local",
            "Advertising",
            "Privacy",
            "Security",
            "AntiAD",
            "Hagezi",
            "Other"
        ]
        self.rules: Dict[str, Dict[str, Set[str]]] = {
            policy: {cat: set() for cat in self.category_names}
            for policy in ["REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT"]
        }
        # seen_rules[policy] = Set(rendered_rule)
        self.seen_rules: Dict[str, Set[str]] = {
            policy: set() for policy in ["REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT"]
        }
        self.sections: Dict[str, Set[str]] = {name: set() for name in SECTION_NAMES}    
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
            hijack_path = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/HTTPDNS_Hijack.list")
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
        for attempt in range(3):
            try:
                result = subprocess.run(
                    [
                        "curl", "-L", "-s", "-m", "60", "-f",
                        "-H", "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.7778.56 Safari/537.36",
                        url
                    ],
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="replace",
                )
                if result.returncode == 0 and result.stdout.strip():
                    return result.stdout
            except Exception as exc:
                if attempt == 2:
                    Logger.warn(f"Download failed for {url}: {exc}")
        return None

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

    def build_input_hashes(self, module_paths: List[str], source_entries: List[SourceEntry]) -> Dict[str, str]:
        hashes: Dict[str, str] = {}
        tracked_paths = {ADBLOCK_SOURCES_FILE, SKK_REJECT, SKK_HTTPDNS, WHITELIST_FILE}
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

        # Remove policy (case-insensitive) - more aggressive to handle middle-of-line
        rule = re.sub(
            r",(REJECT-NO-DROP|REJECT-DROP|REJECT-TINYGIF|REJECT-IMG|REJECT|DIRECT|PROXY)\b",
            "",
            rule,
            flags=re.IGNORECASE,
        )
        # Remove common parameters (case-insensitive)
        rule = re.sub(r",extended-matching\b", "", rule, flags=re.IGNORECASE)
        rule = re.sub(r",pre-matching\b", "", rule, flags=re.IGNORECASE)
        rule = re.sub(r",no-resolve\b", "", rule, flags=re.IGNORECASE)
        rule = re.sub(r"\s+", " ", rule).strip()

        if not rule:
            return None

        if "," in rule:
            parts = [p.strip() for p in rule.split(",")]
            # If the last part is a policy or a placeholder, remove it
            while parts and (parts[-1].upper() in ("REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT", "PROXY", "REJECT-TINYGIF", "REJECT-IMG") or parts[-1] == "{{{Proxy}}}"):
                parts = parts[:-1]
            rule = ",".join(parts)
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
            # Surge does not support spaces in DOMAIN-REGEX in .list files
            if " " in payload or payload in INVALID_DOMAIN_REGEX_VALUES or len(payload) < 2:
                return None
            return f'DOMAIN-REGEX,"{payload}"'

        if rule.startswith("URL-REGEX,"):
            payload = rule.split(",", 1)[1].strip().strip('"')
            if "," in payload:
                return f'URL-REGEX,"{payload}"'

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
        if "hagezi" in lowered:
            return "Hagezi"
        if "anti-ad" in lowered:
            return "AntiAD"
        if any(kw in lowered for kw in ["privacy", "tracking", "tracking-protection"]):
            return "Privacy"
        if any(kw in lowered for kw in ["hijacking", "phishing", "tif", "threat", "banprogram"]):
            return "Security"
        if any(kw in lowered for kw in ["advertising", "ads", "banad"]):
            return "Advertising"
        if "local_sources" in lowered or "localmodules" in lowered:
            return "Local"
        return "Other"

    def add_rule_line(self, line: str, default_policy: str, category: str = "Other"):
        if not line or line.startswith("#"):
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
        # Ensure policy is present and not duplicated
        # Shadowrocket and Surge both prefer: TYPE,VALUE,POLICY,OPTIONS
        parts = [p.strip() for p in normalized_rule.split(",")]
        
        # Build options set from raw_rule
        options_found = []
        if "extended-matching" in raw_rule.lower():
            options_found.append("extended-matching")
        if "pre-matching" in raw_rule.lower():
            options_found.append("pre-matching")
        if "no-resolve" in raw_rule.lower():
            options_found.append("no-resolve")

        # Result is: TYPE, VALUE, POLICY
        result_parts = parts + [policy]
        if options_found:
            result_parts.extend(options_found)
            
        return ",".join(result_parts)

    def is_enhancement_line(self, line: str) -> bool:
        """Check if a line is an enhancement. Known enhancements take absolute priority."""
        lower_line = line.lower()
        
        # Priority 0: Absolute blocks (known enhancement suites and types)
        absolute_blocks = [
            "zheye", "哲也", "zheye.min.js", 
            "enhanced", "enhancement", "增强", 
            "crack", "破解", "unlock", "解锁", "unblock", "解除",
            "vip", "premium", "会员", "会员解锁",
            "camscanner", "扫描全能王", "bilibili.global", "bilibili.enhanced", "biliintl",
            "youtube增强", "bilibili增强", "reddit增强", "apple服务增强",
            "weatherkit", "wechat_enhance", "eringo", "helper", "助手"
        ]
        if any(kw in lower_line for kw in absolute_blocks):
            return True

        adblock_keywords = [
            "reject", "clean", "strip", 
            "blank", "pixel", "广告", "拦截", "去广告",
            "anti-ad", "adblock", "tracking", "fix", "修复"
        ]
        
        enhancement_keywords = [
            "translate", "translation", "翻译", 
            "hook", "conversion", "转换",
            "feature", "iap", "receipt", "nsfw",
            "优化", "功能增强", "optimize",
            "intl", "region", "bypass", "绕过", "地区",
            "dual", "pip", "画中画", "background", "后台播放",
            "redirect", "重定向"
        ]

        # PRIORITY 1: If it looks like ad-block, it's NOT an enhancement (Keep it!)
        if any(kw in lower_line for kw in adblock_keywords):
            return False
            
        # PRIORITY 2: If it has enhancement keywords, filter it out
        if any(kw in lower_line for kw in enhancement_keywords):
            return True
            
        return False

    def extract_from_text(self, text: str, default_policy: str = "REJECT", category: str = "Other", include_sections: bool = False):
        current_section = None
        in_rule_section = False
        seen_real_header = False

        for line in text.splitlines():
            stripped = line.strip()
            if not stripped:
                continue

            if stripped.startswith("[") and stripped.endswith("]") and not stripped.startswith("#"):
                current_section = stripped[1:-1]
                in_rule_section = current_section in ("Rule", "Adblock Plus")
                seen_real_header = True
                continue

            # Fallback for headerless modules or content before first [Section]
            if not seen_real_header:
                if self.normalize_rule(stripped):
                    self.add_rule_line(stripped, default_policy, category=category)
                    continue

                if include_sections:
                    inferred_section = self.infer_non_rule_section(stripped)
                    if inferred_section in self.sections:
                        self.sections[inferred_section].add(stripped)
                    elif inferred_section == "MITM" and stripped.startswith("hostname"):
                        hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", stripped)
                        self.mitm_hosts.update(host.strip() for host in hosts.split(",") if host.strip())
                continue

            if in_rule_section:
                # IMPORTANT: Always use add_rule_line to ensure policy and normalization
                self.add_rule_line(stripped, default_policy, category=category)
                continue

            if not include_sections or stripped.startswith("#"):
                continue

            if current_section in self.sections:
                # Rule section is already handled above, skip here to prevent duplication
                if current_section == "Rule":
                    continue
                # Filter out enhancements
                if self.is_enhancement_line(stripped):
                    continue
                self.sections[current_section].add(stripped)
            elif current_section == "MITM" and stripped.startswith("hostname"):
                hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", stripped)
                self.mitm_hosts.update(host.strip() for host in hosts.split(",") if host.strip())

    def extract_from_file(self, path: str, default_policy: str = "REJECT", category: str = "Other", include_sections: bool = False):
        if not os.path.exists(path):
            return
        self.extract_from_text("".join(read_file(path)), default_policy=default_policy, category=category, include_sections=include_sections)

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
        candidates = {base_name}
        matches = set(MODULE_TARGET_ALIASES.get(base_name, []))

        for root_dir, _, filenames in os.walk(SURGE_MODULE_DIR):
            if base_name in filenames:
                matches.add(os.path.join(root_dir, base_name))

        return sorted(matches)

    def sync_module_sources(self, source_entries: List[SourceEntry]) -> Tuple[Dict[str, str], List[str]]:
        cached_contents: Dict[str, str] = {}
        failures: List[str] = []

        for entry in source_entries:
            if not self.is_module_source(entry):
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

            include_sections = self.is_module_source(entry)
            self.extract_from_text(content, default_policy=entry.policy, category=category, include_sections=include_sections)
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
        """Generate split rulesets and return list of relative paths."""
        generated_files = []
        if not os.path.exists(ADBLOCK_DIR):
            os.makedirs(ADBLOCK_DIR)

        Logger.info(f"Generating split rulesets in {os.path.relpath(ADBLOCK_DIR, ROOT)}")
        
        # Max size in bytes (20MB = 20 * 1024 * 1024)
        MAX_SIZE = 18 * 1024 * 1024  # Use 18MB as safety margin

        for category in self.category_names:
            rules = self.filter_rules(self.rules["REJECT"][category])
            if not rules:
                continue

            # Check if we need to split based on rule count estimation
            # Roughly 30 bytes per rule average
            rules_per_file = 500000 
            
            chunks = [rules[i:i + rules_per_file] for i in range(0, len(rules), rules_per_file)]
            
            for idx, chunk in enumerate(chunks):
                suffix = f"_{idx+1}" if len(chunks) > 1 else ""
                filename = f"AdBlock_{category}{suffix}.list"
                filepath = os.path.join(ADBLOCK_DIR, filename)
                
                content = [
                    f"# Ruleset: AdBlock {category}{suffix}\n",
                    f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n",
                    f"# Total Rules: {len(chunk)}\n",
                    f"# Category: {category}\n",
                ]
                content.extend(f"{rule}\n" for rule in chunk)
                write_file(filepath, "".join(content))
                
                rel_path = os.path.relpath(filepath, ROOT)
                generated_files.append(rel_path)
                Logger.success(f"  ✓ {filename} ({len(chunk)} rules)")

        # Also update the legacy AdBlock.list with a notice or a subset
        legacy_content = [
            "# Ruleset: AdBlock (LEGACY / REDIRECT)\n",
            "# This file is now split into multiple files in ruleset/AdBlock/\n",
            "# Please update your module to use the new split rulesets.\n",
            f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n",
        ]
        # Include top 1000 rules just in case
        all_reject_rules = []
        for cat in self.category_names:
            all_reject_rules.extend(self.filter_rules(self.rules["REJECT"][cat]))
        
        legacy_content.extend(f"{rule}\n" for rule in all_reject_rules[:1000])
        write_file(ADBLOCK_LIST, "".join(legacy_content))
        
        return generated_files

    def generate_module(self, generated_rulesets: List[str]):
        current_date = datetime.now().strftime('%Y-%m-%d')
        base_url = "https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/"
        
        header = (
            f"#!name=🚫 Universal Ad-Blocking Rules (PROMAX) - [{current_date}]\n"
            "#!desc=Split-ruleset ad-block rebuild from curated rule sources (PROMAX version).\n"
            "#!author=ScriptHub-Automated\n"
            "#!icon=https://raw.githubusercontent.com/luestr/IconResource/main/Other_icon/120px/KeLee.png\n"
            f"#!category={GROUP_HEAD_EXPANSE}\n"
            "#!tag=AdBlock, Dependency, HTTPDNS\n"
            f"#!date={datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n"
            "[Rule]\n"
            "# High-Priority White-list (Prevent DNS failure)\n"
            "DOMAIN,dns.alidns.com,DIRECT\n"
            "DOMAIN,doh.pub,DIRECT\n"
            "DOMAIN,dns.pub,DIRECT\n"
            "DOMAIN,dot.pub,DIRECT\n"
            "DOMAIN,doh.360.cn,DIRECT\n"
            "DOMAIN,doh.dns.apple.com,DIRECT\n"
            "# Block app-layer HTTPDNS first\n"
            f"RULE-SET,{base_url}ruleset/Surge%28Shadowkroket%29/HTTPDNS_Hijack.list,REJECT\n"
        )

        content = [header]
        content.append("# Split REJECT Rulesets\n")
        for rs_path in sorted(generated_rulesets):
            encoded_path = urllib.parse.quote(rs_path)
            content.append(f"RULE-SET,{base_url}{encoded_path},REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve\n")

        content.append("\n# SKK Upstream Rulesets\n")
        content.append(f"RULE-SET,{base_url}ruleset/Surge%28Shadowkroket%29/skk_upstream/reject-no-drop.list,REJECT-NO-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve\n")
        content.append(f"RULE-SET,{base_url}ruleset/Surge%28Shadowkroket%29/skk_upstream/reject-drop.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve\n")
        content.append(f"RULE-SET,{base_url}ruleset/Surge%28Shadowkroket%29/skk_upstream/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve\n\n")

        # Define complex rule prefixes that MUST stay in the [Rule] section
        complex_prefixes = ("URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,")
        
        for policy in ("REJECT", "REJECT-DROP", "REJECT-NO-DROP"):
            all_rules = set()
            for cat in self.category_names:
                all_rules.update(self.rules[policy][cat])
            
            if not all_rules:
                continue
                
            # Only include complex rules in the module for REJECT.
            if policy == "REJECT":
                display_rules = sorted([r for r in all_rules if any(r.startswith(p) for p in complex_prefixes)])
            else:
                display_rules = sorted(all_rules)
                
            if not display_rules:
                continue
                
            content.append(f"# Merged {policy} Complex Rules ({len(display_rules)})\n")
            for r in display_rules:
                if not any(r.endswith(suffix) for suffix in (",REJECT", ",REJECT-DROP", ",REJECT-NO-DROP", ",DIRECT", ",PROXY", ",REJECT-TINYGIF", ",REJECT-IMG")):
                    r = f"{r},{policy}"
                content.append(f"{r}\n")
            content.append("\n")

        for section_name in SECTION_NAMES:
            items = sorted(self.sections[section_name])
            if not items:
                continue
            content.append(f"[{section_name}]\n")
            content.extend(f"{item}\n" for item in items)
            content.append("\n")

        if self.mitm_hosts:
            content.append("[MITM]\n")
            content.append(f"hostname = %APPEND% {', '.join(sorted(self.mitm_hosts))}\n")

        write_file(TARGET_MODULE, "".join(content))
        Logger.success(f"Module generated: {os.path.basename(TARGET_MODULE)}")

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

        # 3. Process Local Sources (highest priority)
        Logger.section("Integrating Local AdBlock Sources")
        # Local dir sources
        local_dir = os.path.join(ROOT, "module/local_sources")
        if os.path.exists(local_dir):
            for f in sorted(os.listdir(local_dir)):
                if f.endswith((".sgmodule", ".module")):
                    path = os.path.join(local_dir, f)
                    Logger.info(f"  + Merging local source: {f}")
                    self.extract_from_file(path, default_policy="REJECT", category="Local", include_sections=True)
        
        # Amplify Nexus blockers
        Logger.section("Scanning Amplify Nexus for Ad-Blockers")
        nexus_dir = os.path.join(ROOT, "module/surge(main)/amplify_nexus")
        if os.path.exists(nexus_dir):
            for f in sorted(os.listdir(nexus_dir)):
                if f.endswith(".sgmodule") and f not in PROTECTED_MODULES:
                    path = os.path.join(nexus_dir, f)
                    with open(path, 'r', encoding='utf-8', errors='ignore') as tf:
                        text = tf.read().lower()
                        if any(kw in text for kw in ["reject", "广告", "拦截", "anti-ad"]):
                            Logger.info(f"  + Merging Nexus ad-block component: {f}")
                            category = self.determine_category(f)
                            self.extract_from_file(path, default_policy="REJECT", category=category, include_sections=True)

        # 4. Fetch and extract from all other canonical sources (sorted by priority)
        Logger.section("Fetching Canonical AdBlock Sources")
        failures = self.process_source_entries(source_entries, cached_contents=cached_modules)

        # 5. Load SKK fallback (lowest priority)
        if os.path.exists(SKK_REJECT):
            Logger.info("Loading local skk reject fallback...")
            self.extract_from_file(SKK_REJECT, default_policy="REJECT", category="Other", include_sections=False)

        generated_rulesets = self.generate_ruleset()
        self.generate_module(generated_rulesets)
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
