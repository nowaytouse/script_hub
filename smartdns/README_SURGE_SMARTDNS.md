# Surge + SmartDNS (V45 Safer Dual-Stack)

This repo now treats `smartdns-rs` as the maintained local DNS engine. The recommended local listeners are `127.0.0.1:6053` and `[::1]:6053`, with optional PF redirection from port `53`.

## Recommended Topology

```text
Surge -> 127.0.0.1:53 / [::1]:53   (when PF redirect is enabled)
      -> 127.0.0.1:6053 / [::1]:6053 (when querying SmartDNS directly)
             |
             +-> bootstrap / cn / apple  -> direct DNS
             \-> specialty / fallback     -> DoH via socks5://127.0.0.1:6153
```

## Surge General Section

With PF redirection:

```ini
[General]
dns-server = 127.0.0.1:53, [::1]:53
encrypted-dns-server =
```

Without PF redirection:

```ini
[General]
dns-server = 127.0.0.1:6053, [::1]:6053
encrypted-dns-server =
```

## PF Rules

```bash
echo "rdr pass on lo0 inet proto { udp, tcp } from any to 127.0.0.1 port 53 -> 127.0.0.1 port 6053
rdr pass on lo0 inet6 proto { udp, tcp } from any to ::1 port 53 -> ::1 port 6053" | sudo pfctl -a com.apple/smartdns -f -
sudo pfctl -e 2>/dev/null || true
sudo pfctl -a com.apple/smartdns -s nat
```

## Why This Revision Exists

- `127.0.0.1`-only listening was not enough for apps or clients that prefer `::1`.
- TCP listening now matches the PF rules. Truncated DNS replies can fall back cleanly.
- `force-qtype-soa 65` was removed so HTTPS/SVCB lookups stop breaking Mail and similar system services.
- Static upstream mappings were expanded from the Surge `Host` section, and a `system_direct` bucket was added for OCSP, captive portal, local-router, and time-sync domains.

## Verification

```bash
dig @127.0.0.1 -p 6053 baidu.com
dig @::1 -p 6053 apple.com AAAA
lsof -nP -iUDP:6053 -iTCP:6053 | rg smartdns
launchctl print "gui/$(id -u)/com.smartdns" | egrep "state|program"
```

## If Something Still Feels Wrong

```bash
rg -n "force-qtype-soa|audit-enable yes|6353" ~/.smartdns/smartdns.conf ~/Library/LaunchAgents/com.smartdns.plist
tail -f /tmp/smartdns.log | rg "TLS handshake timed out|invalid HTTP method|query failed"
```

The live install should no longer contain `6353`, `force-qtype-soa 65`, or `audit-enable yes`.
