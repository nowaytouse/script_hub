# smartdns-rs — Fastest-IP layer beneath mosdns

Adds speed-test-based answer ranking, TTL smoothing, IPv6 preference, and a native audit log to the existing mosdns steering stack.

## Architecture

```
Surge → 127.0.0.1:53 (mosdns — steering brain)
          │
          ├─ Specialty steers (Apple/Google/MS/…) → direct DoH (unchanged)
          ├─ NSFW                                  → Wikimedia DoH (unchanged)
          ├─ Ali/Tencent vendor                    → vendor DoH    (unchanged)
          │
          ├─ CN domain_set    → 127.0.0.1:6353  (smartdns CN group)
          │                      └─ races Ali + Tencent + DNSPod + 360
          │                         speed-tests returned CN IPs, returns fastest
          │
          └─ intl fallback    → 127.0.0.1:6354  (smartdns intl group)
                                 └─ races Google + Cloudflare + Quad9 +
                                    AdGuard + NextDNS + OpenDNS DoH
                                    via Surge SOCKS5 127.0.0.1:6153
                                    (speed-check disabled for proxied traffic)
```

## What this adds over mosdns alone

| Feature | Where it happens |
|---|---|
| **Fastest-IP selection** | `speed-check-mode ping,tcp:443` on the CN group |
| **TTL smoothing** | `prefetch-domain yes` + `serve-expired yes` (6h prefetch window) |
| **IPv6 preference** | `dualstack-ip-selection yes` (threshold 15ms) |
| **Audit log** | `/tmp/smartdns_audit.log`, rotated (16MB × 8 files) |

mosdns's specialty-steered paths are left alone — racing would defeat the intentional single-upstream steering finalized in earlier commits.

## Installation

```bash
cd ~/Downloads/GitHub/script_hub/smartdns
./install.sh
```

This will:
1. `brew install smartdns` (installs `smartdns-rs`)
2. Copy `smartdns.conf` to `~/.smartdns/smartdns.conf`
3. Create `~/Library/LaunchAgents/com.smartdns.plist` with `RunAtLoad` + `KeepAlive`
4. Load the service

After SmartDNS is running, reload mosdns so it picks up the new chain:

```bash
sudo launchctl unload /Library/LaunchDaemons/com.mosdns.plist && \
sudo launchctl load -w /Library/LaunchDaemons/com.mosdns.plist
```

## Ports

| Port | Role |
|---|---|
| `127.0.0.1:6353` | CN group — racing direct to Ali/Tencent/DNSPod/360 |
| `127.0.0.1:6354` | Intl group — racing through Surge SOCKS5 at `127.0.0.1:6153` |

Both ports are internal — nothing outside mosdns should query them directly.

## Verification

```bash
# CN group: returns a CN IP (speed-tested), <50ms first query, near-zero cached
dig @127.0.0.1 -p 6353 baidu.com +stats

# Intl group: returns a non-CN IP via proxy
dig @127.0.0.1 -p 6354 google.com +stats

# End-to-end through mosdns entry
dig @127.0.0.1 news.ycombinator.com    # hits intl fallback → smartdns
dig @127.0.0.1 jd.com                  # hits cn_sequence    → smartdns
dig @127.0.0.1 apple.com               # steered direct to Apple DoH (unchanged)
```

## Management

```bash
# Status
launchctl list | grep com.smartdns
lsof -i :6353 -i :6354

# Logs
tail -f /tmp/smartdns.log           # main log
tail -f /tmp/smartdns_audit.log     # per-query audit

# Stop / start / restart
launchctl unload ~/Library/LaunchAgents/com.smartdns.plist
launchctl load   ~/Library/LaunchAgents/com.smartdns.plist

# Edit config (then restart)
vim ~/.smartdns/smartdns.conf
```

## Audit log

`/tmp/smartdns_audit.log` records every query that reaches SmartDNS along with the IP it returned. Combined with mosdns's JSON log (`/tmp/mosdns.log`, which covers every query *before* steering including the specialty-steered paths), you get full-stack visibility.

## Troubleshooting

**SmartDNS won't start**
```bash
cat /tmp/smartdns.stderr.log
# Test config manually:
$(brew --prefix)/sbin/smartdns run -c ~/.smartdns/smartdns.conf -f
```

**Intl queries time out but direct mosdns paths work**
The Surge SOCKS5 proxy at `127.0.0.1:6153` isn't reachable. Confirm Surge is running and that the SOCKS5 port matches the one in `~/.smartdns/smartdns.conf`.

**CN queries return non-CN IPs and get rejected by mosdns**
The CN DoH upstreams are being poisoned or 360 is returning garbage. Inspect the audit log — if a specific upstream is consistently bad, remove its `server-https` line from `~/.smartdns/smartdns.conf` and restart.

**Port 6353 or 6354 already in use**
```bash
lsof -i :6353
# Kill conflicting process or change the `bind` line in smartdns.conf
```

## Uninstallation

```bash
launchctl unload ~/Library/LaunchAgents/com.smartdns.plist
rm ~/Library/LaunchAgents/com.smartdns.plist
rm -rf ~/.smartdns
brew uninstall smartdns
```

Remember to also revert `mosdns/config.yaml` (replace `$forward_smartdns_cn` / `$forward_smartdns_intl` references with the original `$forward_ali`/`$forward_tencent`/`$forward_intl_general` flow) or mosdns's CN and intl buckets will fail.
