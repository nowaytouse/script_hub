import re

with open("scripts/tools/generate_surge_host_dns.py", "r", encoding="utf-8") as f:
    content = f.read()

new_block = """# Traditional IPs for Mainland Speed (Dual Stack)
TRADITIONAL_CN_ALI = "223.5.5.5, 223.6.6.6, 2400:3200::1, 2400:3200:baba::1"
TRADITIONAL_CN_PUB = "119.29.29.29, 2402:4e00::"
TRADITIONAL_CN_360 = "101.226.4.6, 112.65.69.15, 2402:ab80::6"
TRADITIONAL_CN_VOLCANO = "180.184.1.1, 180.184.2.2, 2402:4e00:1020:1404::10, 2402:4e00:1430:1102::a"

# For generic mainland usage (replacing old legacy 114 with robust dual-stack array)
TRADITIONAL_CN_GENERIC = "119.29.29.29, 223.5.5.5, 2400:3200::1, 2402:4e00::"

# Traditional IPs for International Global Speed
TRADITIONAL_GLOBAL = "1.1.1.1, 2606:4700:4700::1111, 9.9.9.9, 2620:fe::fe, 76.76.10.2, 2606:1a40:1::2"

# International pool (aligned with NyaMiiKo.conf [General] encrypted-dns-server)
DOH_NJALLA = "https://doh.njalla.fo/dns-query"
DOH_CLOUDFLARE = "https://cloudflare-dns.com/dns-query"
DOH_GOOGLE = "https://dns.google/dns-query"
DOH_QUAD9 = "https://dns.quad9.net/dns-query"
DOH_CONTROLD = "https://dns.controld.com/p2"
DOH_TW_TWNIC = "https://dns.twnic.tw/dns-query"
DOH_HE_ORDNS = "https://ordns.he.net/dns-query"
DOH_NEXTDNS = "https://dns.nextdns.io"
DOH_MULLVAD_ADBLOCK = "https://adblock.dns.mullvad.net/dns-query"
DOH_DNS_SB = "https://doh.dns.sb/dns-query"

DNS_MAPPING_DOH: Dict[str, str] = {
    "DNS_China_AliDNS": DOH_CN_ALIDNS,
    "DNS_China_ByteDance": DOH_CN_VOLCANO,
    "DNS_China_360": DOH_CN_360,
    "DNS_China_114": TRADITIONAL_CN_GENERIC,
    "DNS_China_114_manual": TRADITIONAL_CN_GENERIC,
    "DNS_Global_Google": DOH_GOOGLE,
    "DNS_Global_Cloudflare": DOH_CLOUDFLARE,
    "DNS_Global_Microsoft": DOH_CONTROLD,
    "DNS_Global_Apple": "system",
    "DNS_Global_Social": DOH_MULLVAD_ADBLOCK,
    "DNS_Global_Quad9": DOH_QUAD9,
}

SURGE_RULESET_DOH: Dict[str, str] = {
    "NSFW": DOH_NJALLA,
    "SocialMedia": DOH_MULLVAD_ADBLOCK,
    "Bilibili": TRADITIONAL_CN_ALI,
    "Apple": "system",
    "AppleNews": "system",
    "Spotify": TRADITIONAL_GLOBAL,
    "Gaming": TRADITIONAL_GLOBAL,
    "StreamEU": TRADITIONAL_GLOBAL,
    "StreamHK": TRADITIONAL_GLOBAL,
    "StreamJP": TRADITIONAL_GLOBAL,
    "StreamKR": TRADITIONAL_GLOBAL,
    "StreamTW": TRADITIONAL_GLOBAL,
    "StreamUS": TRADITIONAL_GLOBAL,
    "GitHub": DOH_CLOUDFLARE,
    "Google": DOH_GOOGLE,
    "Microsoft": DOH_CONTROLD,
}

RULE_LINE ="""

content = re.sub(r'# Traditional IPs for Mainland Speed.*?RULE_LINE =', new_block, content, flags=re.DOTALL)

with open("scripts/tools/generate_surge_host_dns.py", "w", encoding="utf-8") as f:
    f.write(content)
