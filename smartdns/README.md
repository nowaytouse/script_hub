# smartdns-rs — Stable CN acceleration beneath mosdns

Adds speed-test-based answer ranking, TTL smoothing, and IPv6 preference to the CN bucket in the existing mosdns steering stack.

## Architecture

```
Surge → 127.0.0.1:53 (mosdns — steering brain)
          │
          ├─ Specialty steers (Apple/Google/MS/…) → direct DoH (unchanged)
          ├─ NSFW                                  → Wikimedia DoH (unchanged)
          ├─ Ali/Tencent vendor                    → vendor DoH    (unchanged)
          ├─ CN domain_set    → 127.0.0.1:6353  (smartdns CN group)
          │                      └─ races Ali + Tencent + DNSPod + 360
          │                         speed-tests returned CN IPs, returns fastest
          │
          └─ intl fallback    → direct DoH via mosdns + Surge SOCKS5
```

## What this adds over mosdns alone

| Feature | Where it happens |
|---|---|
| **Fastest-IP selection** | `speed-check-mode ping,tcp:443` on the CN group |
| **TTL smoothing** | `serve-expired yes` with capped stale TTL for hot CN domains |
| **IPv6 preference** | `dualstack-ip-selection yes` (threshold 15ms) |

mosdns's specialty-steered and international paths are left alone — racing those through smartdns is what previously created the unstable Surge feedback loop.

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

This port is internal — nothing outside mosdns should query it directly.

## Verification

```bash
# CN group: returns a CN IP (speed-tested), <50ms first query, near-zero cached
dig @127.0.0.1 -p 6353 baidu.com +stats

# End-to-end through mosdns entry
dig @127.0.0.1 news.ycombinator.com    # hits intl fallback → mosdns direct DoH
dig @127.0.0.1 jd.com                  # hits cn_sequence    → smartdns
dig @127.0.0.1 apple.com               # steered direct to Apple DoH (unchanged)
```

## Management

```bash
# Status
launchctl list | grep com.smartdns
lsof -i :6353

# Logs
tail -f /tmp/smartdns.log           # main log
# Stop / start / restart
launchctl unload ~/Library/LaunchAgents/com.smartdns.plist
launchctl load   ~/Library/LaunchAgents/com.smartdns.plist

# Edit config (then restart)
vim ~/.smartdns/smartdns.conf
```

## Audit log

By default the audit log is disabled to avoid flooding Surge with local DNS noise. mosdns's JSON log (`/tmp/mosdns.log`) remains the main visibility point for end-to-end steering.

## Troubleshooting

**SmartDNS won't start**
```bash
cat /tmp/smartdns.stderr.log
# Test config manually:
$(brew --prefix)/sbin/smartdns run -c ~/.smartdns/smartdns.conf -f
```

**CN queries return non-CN IPs and get rejected by mosdns**
The CN DoH upstreams are being poisoned or 360 is returning garbage. Inspect `smartdns.log` and the mosdns log — if a specific upstream is consistently bad, remove its `server-https` line from `~/.smartdns/smartdns.conf` and restart.

**Port 6353 already in use**
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

Remember to also revert `mosdns/config.yaml` if you remove SmartDNS completely, or mosdns's CN bucket will fail.
