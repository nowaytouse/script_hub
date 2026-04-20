#!/usr/bin/env python3
"""
Sync local `.claude` client templates from the Surge-first DNS design.

Rules:
1. Surge remains the primary desktop profile.
2. Shadowrocket and Sing-box must use the non-mosdns DNS baseline:
   `🌐 DNS & Host Enhanced (Strict Privacy)`.
3. Shadowrocket syncs DNS-oriented sections from the converted SR module.
4. Sing-box keeps its own native DNS stack, but mirrors the same steering
   intent with remote DoH rules instead of local mosdns.
"""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
CLAUDE_DIR = ROOT / ".claude"
SURGE_TEMPLATE = CLAUDE_DIR / "NyaMiiKo Max 💻「mosdns」.conf.conf"
SHADOWROCKET_TEMPLATE = CLAUDE_DIR / "shadowroket.conf"
SINGBOX_TEMPLATE = ROOT / "scripts" / "Substore" / "Singbox1.13.0+.conf"
SHADOWROCKET_DNS_MODULE = ROOT / "module" / "shadowrocket" / "amplify_nexus" / "🌐 DNS & Host Enhanced.module"

KEY_VALUE_RE = re.compile(r"^(\s*)([A-Za-z0-9_-]+)(\s*=\s*)(.*)$")
MITM_HOSTNAME_RE = re.compile(r"(?m)^(\s*hostname\s*=\s*)(.*)$")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, content: str) -> None:
    path.write_text(content.rstrip() + "\n", encoding="utf-8")


def get_section_body(text: str, section: str) -> str:
    pattern = re.compile(
        rf"(?ms)^\[{re.escape(section)}\]\n(.*?)(?=^\[[^\n]+\]\n|\Z)"
    )
    match = pattern.search(text)
    if not match:
        raise ValueError(f"Missing section [{section}]")
    return match.group(1)


def replace_section_body(text: str, section: str, new_body: str) -> str:
    pattern = re.compile(
        rf"(?ms)(^\[{re.escape(section)}\]\n)(.*?)(?=^\[[^\n]+\]\n|\Z)"
    )
    match = pattern.search(text)
    if not match:
        raise ValueError(f"Missing section [{section}]")
    return text[: match.start(2)] + new_body.rstrip() + "\n\n" + text[match.end(2) :]


def upsert_section(text: str, section: str, new_body: str, *, before_section: str | None = None) -> str:
    header = f"[{section}]\n"
    if re.search(rf"(?m)^\[{re.escape(section)}\]\n", text):
        return replace_section_body(text, section, new_body)

    insertion = header + new_body.rstrip() + "\n\n"
    if before_section:
        marker = f"\n[{before_section}]\n"
        idx = text.find(marker)
        if idx != -1:
            return text[: idx + 1] + insertion + text[idx + 1 :]
    return text.rstrip() + "\n\n" + insertion


def get_general_value(text: str, key: str) -> str | None:
    body = get_section_body(text, "General")
    for line in body.splitlines():
        match = KEY_VALUE_RE.match(line)
        if match and match.group(2) == key:
            return match.group(4)
    return None


def update_shadowrocket_general(text: str, surge_text: str) -> str:
    general_body = get_section_body(text, "General")
    replacements = {
        "ipv6": get_general_value(surge_text, "ipv6") or "true",
        "prefer-ipv6": "true",
        "hijack-dns": get_general_value(surge_text, "hijack-dns") or "*:53",
        "dns-server": "223.5.5.5, 1.1.1.1, system",
        "doh-server": "https://cloudflare-dns.com/dns-query, https://dns.google/dns-query, https://dns.quad9.net/dns-query",
        "dns-direct-fallback-proxy": "false",
    }

    seen = set()
    out_lines = []

    for line in general_body.splitlines():
        match = KEY_VALUE_RE.match(line)
        if not match:
            out_lines.append(line)
            continue

        indent, key, sep = match.group(1), match.group(2), match.group(3)
        if key in replacements:
            out_lines.append(f"{indent}{key}{sep}{replacements[key]}")
            seen.add(key)
        else:
            out_lines.append(line)

    missing_lines = [f"{key} = {replacements[key]}" for key in replacements if key not in seen]
    if missing_lines:
        if out_lines and out_lines[-1] != "":
            out_lines.append("")
        out_lines.extend(missing_lines)

    return replace_section_body(text, "General", "\n".join(out_lines))


def split_csv(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def merge_csv(*values: str) -> str:
    merged: list[str] = []
    seen: set[str] = set()
    for value in values:
        for item in split_csv(value):
            if item not in seen:
                merged.append(item)
                seen.add(item)
    return ", ".join(merged)


def merge_mitm_hostname(template_mitm_body: str, module_mitm_body: str) -> str:
    template_match = MITM_HOSTNAME_RE.search(template_mitm_body)
    module_match = MITM_HOSTNAME_RE.search(module_mitm_body)
    if not template_match:
        raise ValueError("Template MITM section is missing hostname =")
    if not module_match:
        raise ValueError("DNS module MITM section is missing hostname =")

    merged = merge_csv(module_match.group(2), template_match.group(2))
    return MITM_HOSTNAME_RE.sub(rf"\1{merged}", template_mitm_body, count=1)


def build_singbox_dns() -> dict:
    provider_domains = [
        "dns.google",
        "dns.cloudflare.com",
        "dns.quad9.net",
        "dns.adguard-dns.com",
        "dns.adguard.com",
        "dns.nextdns.io",
        "doh.dns.apple.com",
        "dns.twnic.tw",
        "wikimedia-dns.org",
        "adblock.dns.mullvad.net",
        "doh.pub",
        "dns.pub",
        "doh.360.cn",
        "dns.alidns.com",
    ]

    httpdns_domains = [
        "httpdns.alicdn.com",
        "httpdns-api.aliyuncs.com",
        "httpdns-sc.aliyuncs.com",
        "httpsdns.baidu.com",
        "httpdns.baidu.com",
        "httpdns.baidubce.com",
        "httpdns.bilivideo.com",
        "httpdns.calorietech.com",
        "httpdns.meituan.com",
        "httpdnsvip.meituan.com",
        "httpdns.yunxindns.com",
        "httpdns.n.netease.com",
        "httpdns.music.163.com",
        "music.httpdns.c.163.com",
        "lofter.httpdns.c.163.com",
        "httpdns.push.oppomobile.com",
        "httpdns.volcengineapi.com",
        "dns.weibo.cn",
        "dns.weixin.qq.com",
        "dns.weixin.qq.com.cn",
        "httpdns.c.cdnhwc2.com",
        "dns.jd.com",
    ]

    def https_server(tag: str, server: str, detour: str, server_name: str | None = None) -> dict:
        tls = {
            "enabled": True,
            "min_version": "1.3",
            "max_version": "1.3",
            "curve_preferences": ["x25519", "p256", "p384"],
            "alpn": ["h2", "http/1.1"],
        }
        if server_name:
            tls["server_name"] = server_name
        return {
            "tag": tag,
            "type": "https",
            "server": server,
            "detour": detour,
            "tls": tls,
        }

    return {
        "strategy": "prefer_ipv4",
        "disable_cache": False,
        "disable_expire": False,
        "independent_cache": True,
        "reverse_mapping": True,
        "servers": [
            {
                "tag": "fake_dns",
                "type": "fakeip",
                "inet4_range": "28.0.0.0/8",
                "inet6_range": "fc00::/18",
            },
            {
                "tag": "local_dns",
                "type": "local",
                "prefer_go": True,
            },
            https_server("aliyun_dns", "223.5.5.5", "direct-select", "dns.alidns.com"),
            https_server("dnspod_dns", "1.12.12.12", "direct-select", "doh.pub"),
            https_server("dns_360_dns", "101.198.198.198", "direct-select", "doh.360.cn"),
            https_server("cloudflare_dns", "1.1.1.1", "🌍 海外通用 🌍", "cloudflare-dns.com"),
            https_server("google_dns", "8.8.8.8", "🌍 海外通用 🌍", "dns.google"),
            https_server("quad9_dns", "9.9.9.9", "🌍 海外通用 🌍", "dns.quad9.net"),
            https_server("apple_dns", "17.253.14.125", "🌍 海外通用 🌍", "doh.dns.apple.com"),
            https_server("twnic_dns", "101.101.101.101", "🌍 海外通用 🌍", "dns.twnic.tw"),
            https_server("mullvad_adblock", "194.242.2.2", "🌍 海外通用 🌍", "adblock.dns.mullvad.net"),
            https_server("adguard_dns", "94.140.14.14", "🌍 海外通用 🌍", "dns.adguard-dns.com"),
            https_server("wikimedia_dns", "185.71.138.138", "🌍 海外通用 🌍", "wikimedia-dns.org"),
        ],
        "rules": [
            {"clash_mode": "🎯 直连模式 🎯", "server": "aliyun_dns"},
            {"clash_mode": "🌍 全局模式 🌍", "server": "cloudflare_dns"},
            {"clash_mode": "📋 规则模式 📋", "server": "cloudflare_dns"},
            {"domain": provider_domains, "server": "local_dns"},
            {"domain_suffix": [".local", ".localhost", ".lan", ".test", ".localdomain"], "server": "local_dns"},
            {"domain_suffix": ["httpdns.pro"], "action": "reject"},
            {"domain": httpdns_domains, "action": "reject"},
            {"rule_set": "surge-chinadirect", "server": "dnspod_dns"},
            {"domain_suffix": [".cn", ".gov", ".edu", ".mil", ".int", ".arpa"], "server": "aliyun_dns"},
            {"rule_set": "surge-streamjp", "server": "google_dns"},
            {"rule_set": "surge-streamkr", "server": "google_dns"},
            {"rule_set": "surge-cdn", "server": "cloudflare_dns"},
            {"rule_set": "surge-substore", "server": "quad9_dns"},
            {"rule_set": "surge-streamhk", "server": "google_dns"},
            {"rule_set": "surge-streamtw", "server": "twnic_dns"},
            {"rule_set": "surge-cloudflare", "server": "cloudflare_dns"},
            {"rule_set": "surge-streamus", "server": "google_dns"},
            {"rule_set": "surge-applenews", "server": "apple_dns"},
            {"rule_set": "surge-streameu", "server": "quad9_dns"},
            {"rule_set": "surge-spotify", "server": "cloudflare_dns"},
            {"rule_set": "surge-socialmedia", "server": "cloudflare_dns"},
            {"rule_set": "surge-nsfw", "server": "mullvad_adblock"},
            {"rule_set": "surge-github", "server": "google_dns"},
            {"rule_set": "surge-google", "server": "google_dns"},
            {"rule_set": "surge-microsoft", "server": "quad9_dns"},
            {"rule_set": "surge-ai", "server": "cloudflare_dns"},
            {"rule_set": "surge-binance", "server": "cloudflare_dns"},
            {"rule_set": "surge-globalproxy", "server": "quad9_dns"},
            {"domain_suffix": ["cht.com.tw", "hinet.net", "emome.net", "tw", "taipei"], "server": "twnic_dns"},
            {"domain_suffix": ["he.net"], "server": "quad9_dns"},
            {"domain": ["connectivitycheck.gstatic.com", "msftconnecttest.com", "msftncsi.com", "www.msftncsi.com", "connectivitycheck.android.com"], "server": "google_dns"},
            {"domain": ["detectportal.firefox.com", "connectivity-check.ubuntu.com"], "server": "cloudflare_dns"},
            {"domain": ["connectivitycheck.platform.hicloud.com"], "server": "aliyun_dns"},
            {"domain": ["hanime1.me", "3hentai.net", "18comic.vip"], "server": "mullvad_adblock"},
            {"domain": ["stun.l.google.com"], "server": "local_dns"},
            {"domain_suffix": [".l.google.com"], "server": "local_dns"},
        ],
        "final": "cloudflare_dns",
        "fakeip": {},
    }


def replace_top_level_object(text: str, key: str, new_object: dict) -> str:
    key_pattern = re.compile(rf'(?m)^  "{re.escape(key)}":\s*\{{')
    match = key_pattern.search(text)
    if not match:
        raise ValueError(f'Missing top-level "{key}" object')

    object_start = text.find("{", match.start())
    if object_start == -1:
        raise ValueError(f'Broken top-level "{key}" object')

    in_string = False
    escaped = False
    depth = 0
    object_end = -1

    for idx in range(object_start, len(text)):
        ch = text[idx]
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
            continue

        if ch == '"':
            in_string = True
        elif ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                object_end = idx
                break

    if object_end == -1:
        raise ValueError(f'Unable to locate end of top-level "{key}" object')

    block_start = match.start()
    block_end = object_end + 1
    if block_end < len(text) and text[block_end] == ",":
        block_end += 1

    new_block = f'  "{key}": ' + json.dumps(new_object, ensure_ascii=False, indent=2).replace("\n", "\n  ")
    if block_end <= len(text) and text[block_end - 1] == ",":
        new_block += ","

    return text[:block_start] + new_block + text[block_end:]


def sync_shadowrocket(surge_text: str) -> None:
    module_text = read_text(SHADOWROCKET_DNS_MODULE)
    shadowrocket_text = read_text(SHADOWROCKET_TEMPLATE)
    shadowrocket_text = update_shadowrocket_general(shadowrocket_text, surge_text)
    shadowrocket_text = replace_section_body(shadowrocket_text, "Host", get_section_body(module_text, "Host"))
    shadowrocket_text = upsert_section(
        shadowrocket_text,
        "Script",
        get_section_body(module_text, "Script"),
        before_section="MITM",
    )
    shadowrocket_text = replace_section_body(
        shadowrocket_text,
        "MITM",
        merge_mitm_hostname(
            get_section_body(shadowrocket_text, "MITM"),
            get_section_body(module_text, "MITM"),
        ),
    )
    write_text(SHADOWROCKET_TEMPLATE, shadowrocket_text)


def sync_singbox() -> None:
    singbox_text = read_text(SINGBOX_TEMPLATE)
    singbox_text = replace_top_level_object(singbox_text, "dns", build_singbox_dns())
    singbox_text, route_count = re.subn(
        r'(?m)^    "default_domain_resolver": "[^"]+",$',
        '    "default_domain_resolver": "cloudflare_dns",',
        singbox_text,
        count=1,
    )
    if route_count != 1:
        raise ValueError("Missing route.default_domain_resolver in Sing-box template")
    json.loads(singbox_text)
    write_text(SINGBOX_TEMPLATE, singbox_text)


def main() -> None:
    surge_text = read_text(SURGE_TEMPLATE)
    sync_shadowrocket(surge_text)
    sync_singbox()


if __name__ == "__main__":
    main()
