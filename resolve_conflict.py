import os

filepath = "module/shadowrocket/amplify_nexus/🌐 DNS & Host Enhanced.module"
with open(filepath, 'r') as f:
    content = f.read()

# Replace conflict block with merged version
# We keep the remote's updated comment but my prioritized DoH list
pattern = r"<<<<<<< HEAD.*?=======.*?>>>>>>> 4fc7ec1f \(feat\(dns\): prioritize Mullvad adblock DoH and add IPv6 bootstrap\)"
replacement = """# Global-first fallback pool with mainland backstops. Sensitive domains are
# hard-locked below so international lookups do not fall into CN DoH and
# critical mainland lookups do not spill overseas when a RULE-SET misses.
doh-server = https://adblock.dns.mullvad.net/dns-query, https://dns.quad9.net/dns-query, https://dns.nextdns.io/7f2fac, https://dns.adguard-dns.com/dns-query, https://cloudflare-dns.com/dns-query, https://dns.google/dns-query, https://doh.libredns.gr/noads, https://freedns.controld.com/p1, https://doh.dns4all.eu/dns-query, https://wikimedia-dns.org/dns-query, https://doh.ffmuc.net/dns-query"""

new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)

with open(filepath, 'w') as f:
    f.write(new_content)
