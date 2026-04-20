# SmartDNS (V45 Safer Dual-Stack Edition)

`smartdns-rs` is the maintained replacement for the archived `mosdns` stack in this repo. The current topology keeps SmartDNS as the local DNS entrypoint on `127.0.0.1:6053` and `[::1]:6053`, then splits traffic into direct CN / Apple / bootstrap buckets and proxied specialty buckets.

## What Changed

- Dual-stack local listeners are enabled for both UDP and TCP: `127.0.0.1:6053` and `[::1]:6053`.
- `force-qtype-soa 65` was removed. Returning synthetic SOA for HTTPS/SVCB can make Mail and other system apps slower or behave oddly.
- Audit logging is disabled by default to reduce local I/O noise and query amplification.
- The Surge `Host` bootstrap table is folded into SmartDNS static mappings, including IPv4 and IPv6 addresses for the key upstream resolvers.
- A manual [system_direct.txt](/Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/rules/system_direct.txt) list keeps OCSP, captive portal, router admin, local, and NTP-style domains out of the generic proxy DNS bucket.
- The archived `mosdns` split buckets are restored: `bootstrap`, `airport`, `apple`, `google`, `github`, `microsoft`, `ai_cf`, `tw`, `social`, `tiktok`, `nsfw`, `cn`, and `proxy`.

## Routing Summary

- `bootstrap`: DNS infrastructure, captive portal, Surge health-check style domains. Direct UDP only.
- `system_direct`: certificate validation, local-router, hotspot, and time-sync domains. Forced onto the direct bootstrap path.
- `airport`: subscription / node domains. Proxied international DoH.
- `apple`: Apple domains. Direct Apple DoH.
- `google`: Google-family domains. Proxied Google/Cloudflare DoH.
- `github`: GitHub-family domains. Proxied multi-upstream DoH.
- `microsoft`: Microsoft-family domains. Proxied Quad9/Cloudflare DoH.
- `ai_cf`: AI / Cloudflare-heavy domains. Proxied Cloudflare/Google DoH.
- `tw`: Taiwan stream domains. TWNIC first, Cloudflare fallback.
- `social`: social-media domains. AdGuard first, Cloudflare fallback.
- `tiktok`: TikTok-family domains. Proxied multi-upstream DoH.
- `nsfw`: privacy-sensitive domains. Wikimedia first, Quad9 fallback.
- `cn`: mainland domains. Direct AliDNS / DNSPod / DNSPub plus IPv6 UDP fallback.
- `proxy`: final catch-all for unmatched international domains.

## Local Entry

If you do not use PF redirection, query SmartDNS directly:

```bash
dig @127.0.0.1 -p 6053 baidu.com
dig @::1 -p 6053 baidu.com AAAA
```

The live process should bind all four sockets:

```bash
lsof -nP -iUDP:6053 -iTCP:6053 | rg smartdns
```

Expected shape:

- `UDP 127.0.0.1:6053`
- `TCP 127.0.0.1:6053`
- `UDP [::1]:6053`
- `TCP [::1]:6053`

## PF Redirection

For system-wide port `53` capture on macOS, load the SmartDNS anchor instead of disabling PF:

```bash
echo "rdr pass on lo0 inet proto { udp, tcp } from any to 127.0.0.1 port 53 -> 127.0.0.1 port 6053
rdr pass on lo0 inet6 proto { udp, tcp } from any to ::1 port 53 -> ::1 port 6053" | sudo pfctl -a com.apple/smartdns -f -
sudo pfctl -e 2>/dev/null || true
sudo pfctl -a com.apple/smartdns -s nat
```

If PF is enabled, clients can point to `127.0.0.1:53` and `[::1]:53`. If PF is not enabled, point clients to port `6053` directly.

## Install / Reload

```bash
cd ~/Downloads/GitHub/script_hub/smartdns
./install.sh
launchctl kickstart -k "gui/$(id -u)/com.smartdns"
```

Status and logs:

```bash
launchctl print "gui/$(id -u)/com.smartdns" | egrep "state|program"
tail -f /tmp/smartdns.log
```

## Troubleshooting

`brew upgrade` still hangs:

```bash
dig @127.0.0.1 -p 6053 formulae.brew.sh
tail -f /tmp/smartdns.log | rg "formulae.brew.sh|TLS handshake timed out|dns.google|cloudflare-dns.com|dns.quad9.net"
```

Most stalls are either SOCKS5 `127.0.0.1:6153` not ready or stale live configs still missing the new static upstream mappings.

System / certificate / captive portal lookups still feel wrong:

```bash
dig @127.0.0.1 -p 6053 ocsp.apple.com
dig @127.0.0.1 -p 6053 msftconnecttest.com
dig @127.0.0.1 -p 6053 pool.ntp.org
```

Those domains should now match [system_direct.txt](/Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/rules/system_direct.txt) and stay on the direct bootstrap path instead of being delayed by the proxy bucket.

Mail app still feels slower or less deterministic:

```bash
rg -n "force-qtype-soa|audit-enable yes" ~/.smartdns/smartdns.conf
dig @::1 -p 6053 mail.me.com HTTPS
dig @::1 -p 6053 outlook.office365.com HTTPS
```

The current repo config should return normal HTTPS/SVCB responses again instead of synthetic SOA.

IPv6 still does not work:

```bash
dig @::1 -p 6053 apple.com AAAA
dig @::1 -p 6053 google.com AAAA
lsof -nP -iUDP:6053 -iTCP:6053 | rg smartdns
sudo pfctl -a com.apple/smartdns -s nat
```

If `::1` is not listening or the `inet6` PF rule is absent, the system will silently fall back to IPv4-only behavior.
