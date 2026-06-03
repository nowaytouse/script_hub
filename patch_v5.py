import re

with open("scripts/tools/generate_surge_host_dns.py", "r", encoding="utf-8") as f:
    content = f.read()

new_block = """# Mainland (matches [General] + encrypted-dns baseline)
DOH_CN_ALIDNS = "https://dns.alidns.com/dns-query"
DOH_CN_PUB = "https://doh.pub/dns-query"
DOH_CN_360 = "https://doh.360.cn/dns-query"
DOH_CN_VOLCANO = "https://dns.volcengine.com/dns-query"

# Traditional IPs for Mainland Speed
TRADITIONAL_CN_114 = "114.114.114.114"
TRADITIONAL_CN_ALI = "223.5.5.5"

# Traditional IPs for International Global Speed
TRADITIONAL_GLOBAL = "1.1.1.1, 8.8.8.8"

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
    "DNS_China_114": TRADITIONAL_CN_114,
    "DNS_China_114_manual": TRADITIONAL_CN_114,
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

content = re.sub(r'# Mainland \(matches \[General\].*?RULE_LINE =', new_block, content, flags=re.DOTALL)

with open("scripts/tools/generate_surge_host_dns.py", "w", encoding="utf-8") as f:
    f.write(content)
