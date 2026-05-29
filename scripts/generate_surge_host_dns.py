#!/usr/bin/env python3
"""
Expand ruleset/Sources/DNS_mapping/*.list (+ selected Surge rulesets) into Surge [Host] DoH steering.
Surge [Host] does not support RULE-SET / DOMAIN-SET — domains must be listed explicitly.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Set, Tuple

ROOT = Path(__file__).resolve().parent.parent
DNS_DIR = ROOT / "ruleset" / "Sources" / "DNS_mapping"
SURGE_RULESET_DIR = ROOT / "ruleset" / "Surge(Shadowkroket)"

# Mainland (matches [General] + encrypted-dns baseline)
DOH_CN_ALIDNS = "https://dns.alidns.com/dns-query"
DOH_CN_PUB = "https://doh.pub/dns-query"
DOH_CN_APPLE = "system"
DOH_TW_TWNIC = "https://dns.twnic.tw/dns-query"
DOH_HE_ORDNS = "https://ordns.he.net/dns-query"

# International pool (aligned with NyaMiiKo.conf [General] encrypted-dns-server)
DOH_ADGUARD = "https://dns.adguard-dns.com/dns-query"
DOH_NEXTDNS = "https://dns.nextdns.io/7f2fac"
DOH_CONTROL_D = "https://dns.controld.com/p2"
DOH_MULLVAD = "https://dns.mullvad.net/dns-query"
DOH_MULLVAD_ADBLOCK = "https://adblock.dns.mullvad.net/dns-query"
DOH_LIBREDNS = "https://doh.libredns.gr/noads"
DOH_DNS_SB = "https://doh.dns.sb/dns-query"
DOH_TIAR = "https://doh.tiar.app/dns-query"
DOH_NJALLA = "https://doh.njalla.fo/dns-query"
DOH_ARAPURAYIL = "https://dns.arapurayil.com/dns-query"
DOH_JP_BLAH = "https://jp.blahdns.com/dns-query"
DOH_AHA = "https://doh.ahadns.com/dns-query"
DOH_FFMUC = "https://doh.ffmuc.net/dns-query"
DOH_APPLIED_PRIVACY = "https://doh.applied-privacy.net/query"
DOH_DIGITALE_GESELLSCHAFT = "https://dns.digitale-gesellschaft.ch/dns-query"
DOH_SUDO = "https://dns.sudo.is/dns-query"
DOH_CAPTNEMO = "https://dns.captnemo.in/dns-query"
DOH_CLOUDFLARE = "https://cloudflare-dns.com/dns-query"

DNS_MAPPING_DOH: Dict[str, str] = {
    "DNS_China_AliDNS": DOH_CN_ALIDNS,
    "DNS_China_ByteDance": DOH_CN_ALIDNS,
    "DNS_China_114": DOH_CN_PUB,
    "DNS_China_114_manual": DOH_CN_PUB,
    "DNS_China_360": DOH_CN_PUB,
    "DNS_Global_Google": DOH_NEXTDNS,
    "DNS_Global_Cloudflare": DOH_DNS_SB,
    "DNS_Global_Microsoft": DOH_CONTROL_D,
    "DNS_Global_Apple": DOH_CN_APPLE,
    "DNS_Global_AI": DOH_ADGUARD,
    "DNS_Global_Social": DOH_CLOUDFLARE,
    "DNS_Global_Privacy": DOH_LIBREDNS,
    "DNS_Global_Priority": DOH_NEXTDNS,
    "DNS_Global_Quad9": DOH_CONTROL_D,
    "DNS_Global_Infrastructure": DOH_FFMUC,
}

SURGE_RULESET_DOH: Dict[str, str] = {
    "StreamJP": DOH_JP_BLAH,
    "StreamKR": DOH_JP_BLAH,
    "CDN": DOH_DNS_SB,
    "substore": DOH_SUDO,
    "StreamHK": DOH_NEXTDNS,
    "StreamTW": DOH_TW_TWNIC,
    "Cloudflare": DOH_DNS_SB,
    "StreamUS": DOH_NEXTDNS,
    "AppleNews": DOH_CN_APPLE,
    "StreamEU": DOH_DIGITALE_GESELLSCHAFT,
    "Spotify": DOH_DNS_SB,
    "SocialMedia": DOH_CLOUDFLARE,
    "NSFW": DOH_MULLVAD_ADBLOCK,
    "GitHub": DOH_TIAR,
    "Google": DOH_NEXTDNS,
    "Microsoft": DOH_CONTROL_D,
    "AI": DOH_ADGUARD,
    "Bilibili": DOH_CN_PUB,
    "Gaming": DOH_JP_BLAH,
    "Apple": DOH_CN_APPLE,
    "PayPal": DOH_CN_ALIDNS,
    "Binance": DOH_ADGUARD,
}

RULE_LINE = re.compile(
    r"^(DOMAIN(?:-SUFFIX|-KEYWORD)?),(.+)$", re.IGNORECASE
)
_IPV4_HOST = re.compile(r"^\d{1,3}(?:\.\d{1,3}){3}$")

# Telegram DC IP pins + domain keys — never overridden by bulk DoH expansion
TELEGRAM_DC_IPS = {
    "91.108.56.100", "91.108.56.101", "91.108.56.104", "91.108.56.107",
    "91.108.56.120", "91.108.56.125", "91.108.56.126", "91.108.56.128",
    "91.108.56.156",
    "149.154.175.10", "149.154.175.50", "149.154.175.54", "149.154.175.55",
    "149.154.175.56", "149.154.175.57", "149.154.175.100", "149.154.175.101",
    "149.154.175.102", "149.154.175.103", "149.154.175.117", "149.154.175.40",
    "91.108.4.0", "91.108.8.0", "91.108.12.0", "91.108.16.0",
    "149.154.167.0", "149.154.171.0", "149.154.163.0", "149.154.167.40",
}
TELEGRAM_DOMAIN_MARKERS = (
    "telegram.org", "telegram.me", "telegram.dog", "telegram.space",
    "telegram-cdn.org", "telegramdownload.com", "t.me", "telesco.pe",
)
# Surge [Host]: *.suffix, literal FQDN, single-? label, or _special_ names
_VALID_HOST_KEY = re.compile(
    r"^(\*\.[a-zA-Z0-9_]([a-zA-Z0-9._-]*[a-zA-Z0-9_])?"
    r"|[a-zA-Z0-9_]([a-zA-Z0-9_-]*[a-zA-Z0-9_])?(\.[a-zA-Z0-9_]([a-zA-Z0-9_-]*[a-zA-Z0-9_])?)*"
    r"|[a-zA-Z0-9_]?\?([a-zA-Z0-9._?-]*)?)$"
)


def is_reserved_auto_host(key: str) -> bool:
    """Keep Telegram DC pins / domains out of auto DoH lists (first-match wins)."""
    if key in TELEGRAM_DC_IPS:
        return True
    low = key.lower().lstrip("*.")
    for marker in TELEGRAM_DOMAIN_MARKERS:
        if low == marker or low.endswith("." + marker) or marker in low:
            return True
    return False


def is_valid_surge_host_key(key: str) -> bool:
    """Reject ruleset paths, Clash-style globs, and multi-? patterns."""
    if not key or len(key) > 253:
        return False
    if _IPV4_HOST.match(key):
        return True
    lowered = key.lower()
    if any(ch in key for ch in "/\\()"):
        return False
    if ".." in key or lowered.endswith(".list") or "ruleset" in lowered:
        return False
    if re.search(r"\?\?+", key):
        return False
    # DOMAIN,aliyun.* / toutiao???.??? — not valid Surge Host keys
    if key.endswith(".*") or key.endswith(".") or key.startswith("."):
        return False
    if "*" in key and not key.startswith("*."):
        return False
    # Surge allows one '?' per label (e.g. stun?.l.google.com)
    if "?" in key:
        if key.count("?") > 1:
            return False
        if not re.match(r"^[a-zA-Z0-9_.?-]+$", key):
            return False
    elif not _VALID_HOST_KEY.match(key):
        return False
    labels = key[2:].split(".") if key.startswith("*.") else key.split(".")
    for label in labels:
        if not label or len(label) > 63:
            return False
        if label.startswith("-") or label.endswith("-"):
            return False
        if "?" in label and label.count("?") > 1:
            return False
    return True


def parse_list_file(path: Path) -> Iterable[Tuple[str, str]]:
    for raw in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        line = raw.split("//")[0].split("#")[0].strip()
        if not line or line.startswith("#"):
            continue
        m = RULE_LINE.match(line)
        if m:
            yield m.group(1).upper(), m.group(2).strip()


def host_key(rule_type: str, domain: str) -> Optional[str]:
    if not domain or len(domain) > 253:
        return None
    domain = domain.strip().strip('"').strip("'")
    if rule_type == "DOMAIN-SUFFIX":
        candidate = f"*.{domain.lstrip('.')}"
    elif rule_type == "DOMAIN-KEYWORD":
        return None
    else:
        candidate = domain
    return candidate if is_valid_surge_host_key(candidate) else None


def host_line(key: str, doh: str) -> str:
    return f"{key} = server:{doh}"


def collect_hosts(
    sources: List[Tuple[str, Path, str]],
    seen: Set[str],
) -> List[str]:
    out: List[str] = []
    for label, path, doh in sources:
        if not path.is_file():
            continue
        block: List[str] = []
        count = 0
        for rule_type, domain in parse_list_file(path):
            key = host_key(rule_type, domain)
            if not key or key in seen or is_reserved_auto_host(key):
                continue
            seen.add(key)
            block.append(host_line(key, doh))
            count += 1
        if block:
            out.append(f"# --- {label} ({count} hosts) → {doh}")
            out.extend(block)
            out.append("")
    return out


def bootstrap_block() -> List[str]:
    return [
        "# =============================================================================",
        "# SECTION A: DoH bootstrap (resolve provider hostnames without circular DoH)",
        "# =============================================================================",
        "dns.google = 8.8.8.8, 8.8.4.4, 2001:4860:4860::8888, 2001:4860:4860::8844",
        "dns64.dns.google = 2001:4860:4860::6464, 2001:4860:4860::64",
        "cloudflare-dns.com = 104.16.249.249, 104.16.248.249, 2606:4700::6810:f8f9, 2606:4700::6810:f9f9",
        "1dot1dot1dot1.cloudflare-dns.com = 1.1.1.1, 1.0.0.1, 2606:4700:4700::1001, 2606:4700:4700::1111",
        "one.one.one.one = 1.1.1.1, 1.0.0.1, 2606:4700:4700::1001, 2606:4700:4700::1111",
        "dns.quad9.net = 9.9.9.9, 149.112.112.112, 2620:fe::fe, 2620:fe::9",
        "dns.alidns.com = 223.5.5.5, 223.6.6.6, 2400:3200:baba::1, 2400:3200::1",
        "doh.pub = 1.12.12.12, 120.53.53.53",
        "dns.pub = 1.12.12.12, 120.53.53.53",
        "doh.360.cn = 23.6.48.18, 112.65.69.15",
        "dns.baidu.com = 180.76.76.76, 110.242.68.66",
        "dns.twnic.tw = 101.101.101.101, 2001:de4::101",
        "ordns.he.net = 74.82.42.42, 2001:470:20::2",
        "dns.adguard-dns.com = 94.140.14.14, 94.140.15.15, 2a10:50c0::ad1:ff, 2a10:50c0::ad2:ff",
        "dns.adguard.com = 94.140.14.14, 94.140.15.15",
        "doh.libredns.gr = 116.202.176.26",
        "doh.ffmuc.net = 5.1.66.255, 185.150.99.255, 2001:678:e68:f000::, 2001:678:ed0:f000::",
        "dns.mullvad.net = 194.242.2.2, 194.242.2.3",
        "adblock.dns.mullvad.net = 194.242.2.2",
        "freedns.controld.com = 76.76.2.0, 76.76.10.0",
        "dns.controld.com = 76.76.2.0, 76.76.10.0",
        "doh.dns.apple.com = 17.253.1.201, 17.253.1.202",
        "dns.nextdns.io = 45.90.28.0, 45.90.30.0",
        "doh.dns.sb = 185.222.222.222, 185.184.222.222",
        "doh.tiar.app = 139.162.110.150",
        "doh.njalla.fo = 146.255.56.98",
        "dns.arapurayil.com = 185.95.218.42",
        "jp.blahdns.com = 185.150.99.255",
        "doh.ahadns.com = 2a09::, 2a09::1",
        "doh.applied-privacy.net = 2a02:1fb8:0:1::62",
        "dns.digitale-gesellschaft.ch = 2a05:dfc7:5::53",
        "dns.sudo.is = 2400:8902::f03c:91ff:fe06:787f",
        "dns.captnemo.in = 2606:1a40::, 2606:1a40:1::",
        "doh-pure.onedns.net = 117.50.11.11, 52.80.3.111",
        "wikimedia-dns.org = 185.71.138.138",
        "doh.dns4all.eu = 194.0.5.3",
        "dot.360.cn = 101.198.198.198, 101.198.199.200, 101.198.192.33, 112.65.69.15",
        "dns.cn = 1.2.4.8, 210.2.4.8, 2001:dc7:1000::1",
        "dns.tuna.tsinghua.edu.cn = 101.6.6.6, 2001:da8::666",
        "dns.volcengine.com = 180.184.1.1, 180.184.2.2, 2402:4e00:1020:1404::10, 2402:4e00:1430:1102::a",
        "dns6.cfiec.net = 240c:6666::6666, 240c:6644::6644",
        "raw.githubusercontent.com = 185.199.108.133, 185.199.109.133, 185.199.110.133, 185.199.111.133",
        "github.com = 140.82.113.4, 140.82.112.3",
        "",
        "# =============================================================================",
        "# SECTION B: Pinned hosts (Telegram DC / FCM / proxy — must stay above bulk DoH)",
        "# =============================================================================",
        "104.236.69.55 = server:1.1.1.1",
        "91.108.56.100 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.101 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.104 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.107 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.120 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.125 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.126 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.128 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "91.108.56.156 = 91.108.56.147,91.108.56.135,91.108.56.130",
        "149.154.175.10 = 149.154.175.53",
        "149.154.175.50 = 149.154.175.53",
        "149.154.175.54 = 149.154.175.53",
        "149.154.175.55 = 149.154.175.53",
        "149.154.175.56 = 149.154.175.53",
        "149.154.175.57 = 149.154.175.53",
        "149.154.175.100 = 149.154.175.53",
        "149.154.175.101 = 149.154.175.53",
        "149.154.175.102 = 149.154.175.53",
        "149.154.175.103 = 149.154.175.53",
        "149.154.175.117 = 149.154.175.53",
        "91.108.4.0 = 91.108.4.1",
        "91.108.8.0 = 91.108.8.1",
        "91.108.12.0 = 91.108.12.1",
        "91.108.16.0 = 91.108.16.1",
        "149.154.167.0 = 149.154.167.1",
        "149.154.171.0 = 149.154.171.1",
        "149.154.163.0 = 149.154.163.1",
        "149.154.167.40 = 149.154.167.41",
        "149.154.175.40 = 149.154.175.41",
        "talk.google.com = 108.177.125.188",
        "mtalk.google.com = 108.177.125.188, 2404:6800:4008:c07::bc, 142.250.31.188",
        "alt1-mtalk.google.com = 3.3.3.3, 2607:f8b0:4023:c0b::bc, 64.233.171.188",
        "alt2-mtalk.google.com = 3.3.3.3, 142.250.115.188",
        "alt3-mtalk.google.com = 74.125.200.188, 173.194.77.188",
        "alt4-mtalk.google.com = 74.125.200.188, 173.194.219.188",
        "alt5-mtalk.google.com = 3.3.3.3, 2607:f8b0:4023:1::bc, 142.250.112.188",
        "alt6-mtalk.google.com = 3.3.3.3, 172.217.197.188",
        "alt7-mtalk.google.com = 74.125.200.188, 2607:f8b0:4002:c03::bc, 108.177.12.188",
        "alt8-mtalk.google.com = 3.3.3.3",
        "stun.l.google.com = server:force-syslib",
        "stun?.l.google.com = server:force-syslib",
        "aws-linkhy15.liangxin1.xyz = 18.183.7.71",
        "*.liangxin1.xyz = server:system",
        "",
        "# =============================================================================",
        "# SECTION C: Mainland China — DNS_mapping + TLD fallbacks",
        "# =============================================================================",
        "*.cn = server:" + DOH_CN_ALIDNS,
        "*.com.cn = server:" + DOH_CN_ALIDNS,
        "*.net.cn = server:" + DOH_CN_ALIDNS,
        "*.org.cn = server:" + DOH_CN_ALIDNS,
        "*.gov.cn = server:" + DOH_CN_ALIDNS,
        "*.edu.cn = server:" + DOH_CN_ALIDNS,
        "",
        "# =============================================================================",
        "# SECTION D: Taiwan / HK regional TLD & carriers",
        "# =============================================================================",
        "*.cht.com.tw = server:" + DOH_TW_TWNIC,
        "*.hinet.net = server:" + DOH_TW_TWNIC,
        "*.emome.net = server:" + DOH_TW_TWNIC,
        "*.tw = server:" + DOH_TW_TWNIC,
        "*.taipei = server:" + DOH_TW_TWNIC,
        "*.hk = server:" + DOH_NEXTDNS,
        "*.he.net = server:" + DOH_HE_ORDNS,
        "",
        "# =============================================================================",
        "# SECTION E: ruleset/Sources/DNS_mapping (manual Host expansion)",
        "# =============================================================================",
    ]


def infrastructure_block() -> List[str]:
    return [
        "",
        "# =============================================================================",
        "# SECTION F: Rule-aligned Surge rulesets (from [Rule] DOMAIN entries)",
        "# GlobalProxy.list omitted (~37k) — use FINAL proxy group + encrypted-dns pool",
        "# =============================================================================",
    ]


def tail_block() -> List[str]:
    return [
        "",
        "# =============================================================================",
        "# SECTION G: Inline / connectivity / NSFW exceptions",
        "# =============================================================================",
        "hanime1.me = server:" + DOH_MULLVAD_ADBLOCK,
        "3hentai.net = server:" + DOH_MULLVAD_ADBLOCK,
        "18comic.vip = server:" + DOH_MULLVAD_ADBLOCK,
        "connectivitycheck.gstatic.com = server:" + DOH_NEXTDNS,
        "detectportal.firefox.com = server:" + DOH_DNS_SB,
        "msftconnecttest.com = server:" + DOH_NEXTDNS,
        "msftncsi.com = server:" + DOH_NEXTDNS,
        "www.msftncsi.com = server:" + DOH_NEXTDNS,
        "connectivitycheck.android.com = server:" + DOH_NEXTDNS,
        "connectivity-check.ubuntu.com = server:" + DOH_DNS_SB,
        "connectivitycheck.platform.hicloud.com = server:" + DOH_CN_ALIDNS,
        "",
        "# =============================================================================",
        "# SECTION H: OCSP / certificate verification (system resolver)",
        "# =============================================================================",
        "ocsp.digicert.cn = server:system",
        "ocsp.digicert.com = server:system",
        "crl3.digicert.com = server:system",
        "crl4.digicert.com = server:system",
        "ocsp.sectigo.com = server:system",
        "ocsp.verisign.com = server:system",
        "ocsp.globalsign.com = server:system",
        "ocsp.comodoca.com = server:system",
        "ocsp.entrust.net = server:system",
        "ocsp.identrust.com = server:system",
        "ocsp.pki.goog = server:system",
        "ocsp.apple.com = server:system",
        "ocsp2.apple.com = server:system",
        "ocsp-lb.apple.com.akadns.net = server:system",
        "",
        "# =============================================================================",
        "# SECTION I: LAN / router admin / IPv6 literals",
        "# =============================================================================",
        "ip6-localhost = ::1",
        "ip6-loopback = ::1",
        "ip6-localnet = fe00::0",
        "ip6-mcastprefix = ff00::0",
        "ip6-allnodes = ff02::1",
        "ip6-allrouters = ff02::2",
        "ip6-allhosts = ff02::3",
        "*.local = server:system",
        "*.lan = server:system",
        "*.test = server:system",
        "*.localhost = server:system",
        "*.localdomain = server:system",
        "_hotspot_.m2m = server:force-syslib",
        "hotspot.cslwifi.com = server:force-syslib",
        "*.id.ui.direct = server:force-syslib",
        "amplifi.lan = server:force-syslib",
        "router.synology.com = server:force-syslib",
        "sila.razer.com = server:force-syslib",
        "router.asus.com = server:force-syslib",
        "routerlogin.net = server:force-syslib",
        "orbilogin.com = server:force-syslib",
        "www.LinksysSmartWiFi.com = server:force-syslib",
        "LinksysSmartWiFi.com = server:force-syslib",
        "instant.arubanetworks.com = server:force-syslib",
        "setmeup.arubanetworks.com = server:force-syslib",
        "www.miwifi.com = server:force-syslib",
        "miwifi.com = server:force-syslib",
        "mediarouter.home = server:force-syslib",
        "tplogin.cn = server:force-syslib",
        "tplinklogin.net = server:force-syslib",
        "tplinkwifi.net = server:force-syslib",
        "melogin.cn = server:force-syslib",
        "falogin.cn = server:force-syslib",
        "tendawifi.com = server:force-syslib",
        "leike.cc = server:force-syslib",
        "zte.home = server:force-syslib",
        "p.to = server:force-syslib",
        "phicomm.me = server:force-syslib",
        "hiwifi.com = server:force-syslib",
        "peiluyou.com = server:force-syslib",
    ]


def _reserve_keys_from_lines(seen: Set[str], lines: Iterable[str]) -> None:
    for line in lines:
        if " = " in line and not line.strip().startswith("#"):
            key = line.split(" = ", 1)[0].strip()
            if is_valid_surge_host_key(key):
                seen.add(key)
    seen.update(TELEGRAM_DC_IPS)


def build_host_section() -> str:
    seen: Set[str] = set()
    _reserve_keys_from_lines(seen, bootstrap_block())
    _reserve_keys_from_lines(seen, tail_block())

    lines: List[str] = [
        "# Surge [Host] DNS steering — auto-generated by scripts/generate_surge_host_dns.py",
        "# DoH pool mirrors [General] dns-server + encrypted-dns-server in NyaMiiKo.conf",
        "# Regenerate: python3 scripts/generate_surge_host_dns.py --write",
        "",
    ]
    lines.extend(bootstrap_block())

    dns_sources: List[Tuple[str, Path, str]] = []
    for name, doh in DNS_MAPPING_DOH.items():
        dns_sources.append((name, DNS_DIR / f"{name}.list", doh))

    lines.extend(collect_hosts(dns_sources, seen))
    lines.extend(infrastructure_block())

    surge_sources: List[Tuple[str, Path, str]] = []
    for name, doh in SURGE_RULESET_DOH.items():
        surge_sources.append((f"Surge/{name}", SURGE_RULESET_DIR / f"{name}.list", doh))
    lines.extend(collect_hosts(surge_sources, seen))
    lines.extend(tail_block())
    return "\n".join(lines).rstrip() + "\n"


def replace_host_section(conf_path: Path, host_body: str) -> None:
    text = conf_path.read_text(encoding="utf-8")
    pattern = re.compile(r"(?ms)^\[Host\]\n.*?(?=^\[[^\n]+\]\n|\Z)")
    if not pattern.search(text):
        raise ValueError(f"No [Host] section in {conf_path}")
    new_text = pattern.sub("[Host]\n" + host_body + "\n", text, count=1)
    conf_path.write_text(new_text, encoding="utf-8")


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="Generate Surge [Host] DNS steering block")
    parser.add_argument(
        "--write",
        action="store_true",
        help="Write into .claude/NyaMiiKo.conf.conf",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / ".claude" / "generated_host_dns.conf",
        help="Fragment output path",
    )
    args = parser.parse_args()

    body = build_host_section()
    args.output.write_text(body, encoding="utf-8")
    print(f"Wrote fragment ({body.count(chr(10))} lines): {args.output}")

    if args.write:
        target = ROOT / ".claude" / "NyaMiiKo.conf.conf"
        replace_host_section(target, body)
        print(f"Updated [Host] in {target}")


if __name__ == "__main__":
    main()
