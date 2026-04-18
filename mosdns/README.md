# mosdns DNS Setup for Surge

Stable dual-stack DNS entrypoint for Surge. `mosdns` stays on `127.0.0.1:53` and `[::1]:53`, `smartdns-rs` is limited to the CN acceleration bucket on `127.0.0.1:6353`, and international or specialty DoH stays inside mosdns so the old `Surge -> mosdns -> smartdns -> Surge` loop cannot come back.

## Architecture

```text
Surge -> 127.0.0.1:53 / [::1]:53 (mosdns)
           |
           +-> Bootstrap allowlist         -> plain UDP bootstrap only
           +-> Apple / Google / MS / AI    -> direct DoH in mosdns
           +-> TW / Social / NSFW / TikTok -> direct DoH in mosdns
           +-> Ali / Tencent vendor domains -> direct CN DoH in mosdns
           +-> CN domains                  -> 127.0.0.1:6353 (smartdns CN bucket)
           \-> International fallback      -> direct DoH in mosdns via Surge SOCKS5
```

## Features

- Dual-stack local listeners on `127.0.0.1:53` and `[::1]:53`
- DoH prioritized for steady-state resolution; only the bootstrap allowlist uses plain UDP
- `smartdns-rs` restricted to the CN bucket, which removes the old `6354` intl feedback loop
- IPv4 and IPv6 CN IP sets are both used when mosdns checks whether an intl answer actually landed on a CN CDN
- SmartDNS audit stays disabled so Surge does not get flooded with local DNS noise
- Launchd templates for both components include a working directory, persistent restart behavior, and raised file limits

## Installation

Install `smartdns-rs` first:

```bash
cd ~/Downloads/GitHub/script_hub/smartdns
./install.sh
```

Then install `mosdns`:

```bash
cd ~/Downloads/GitHub/script_hub/mosdns
./install.sh
```

If either component was installed before April 19, 2026, rerun both installers or manually copy the current repo config and plist files, then reload both jobs. Older live installs still contain the unstable `6354` topology and `smartdns` audit settings.

The installer will:

1. Download the `mosdns` binary for your architecture.
2. Download `geosite.dat`, `geoip.mmdb`, the CN domain list, and CN IPv4/IPv6 IP lists.
3. Render [config.yaml](/Users/nyamiiko/Downloads/GitHub/script_hub/mosdns/config.yaml:1) into `~/.mosdns/config/config.yaml`.
4. Render [com.mosdns.daemon.plist](/Users/nyamiiko/Downloads/GitHub/script_hub/mosdns/com.mosdns.daemon.plist:1) into `/Library/LaunchDaemons/com.mosdns.plist`.
5. Reload the `system/com.mosdns` LaunchDaemon.

## Surge Configuration

Update your `[General]` section:

```ini
[General]
dns-server = 127.0.0.1, [::1]
# Remove encrypted-dns-server. mosdns owns DoH.
```

Or use a minimal module:

```ini
#!name=mosdns DNS Integration
#!desc=Route DNS to local mosdns on 127.0.0.1 and [::1]

[General]
dns-server = 127.0.0.1, [::1]
```

## Verification

Check that both launchd jobs are current and running:

```bash
launchctl print gui/$(id -u)/com.smartdns | rg "state =|program ="
launchctl print system/com.mosdns | rg "state =|program ="
```

Check for stale pre-refactor markers in the live configs:

```bash
rg -n "6354|audit-enable yes|forward_smartdns_intl" \
  ~/.smartdns/smartdns.conf \
  ~/.mosdns/config/config.yaml
# Expected: no matches
```

Check that the installed plists include the hardening that exists in this repo:

```bash
plutil -p ~/Library/LaunchAgents/com.smartdns.plist | rg "HardResourceLimits|WorkingDirectory"
plutil -p /Library/LaunchDaemons/com.mosdns.plist | rg "HardResourceLimits|WorkingDirectory"
```

Smoke-test both stacks:

```bash
dig @127.0.0.1 jd.com
dig @127.0.0.1 google.com
dig @::1 jd.com
tail -f /tmp/mosdns.log
```

## Management

```bash
# Status
launchctl print gui/$(id -u)/com.smartdns
launchctl print system/com.mosdns

# Stop / start mosdns
sudo launchctl bootout system /Library/LaunchDaemons/com.mosdns.plist
sudo launchctl bootstrap system /Library/LaunchDaemons/com.mosdns.plist

# Restart mosdns after config edits
sudo launchctl kickstart -k system/com.mosdns

# Edit config
vim ~/.mosdns/config/config.yaml
```

## Routing Details

### CN acceleration path

`mosdns` forwards general CN domains to `smartdns-rs` on `127.0.0.1:6353`, where AliDNS, DNSPod, Tencent, and 360 DoH are raced and ranked with `ping,tcp:443`.

### International fallback path

General international lookups stay inside `mosdns` and go to:

- `https://dns.google/dns-query`
- `https://cloudflare-dns.com/dns-query`
- `https://dns.quad9.net/dns-query`

All three run through Surge SOCKS5 on `127.0.0.1:6153`.

### Specialty steering

`mosdns` keeps the direct per-domain DoH paths for Apple, Google/GitHub, Microsoft, Cloudflare/AI, TWNIC, AdGuard Social, Wikimedia NSFW, TikTok, Ali vendor domains, and Tencent vendor domains.

## Troubleshooting

**Surge policies still all red or SmartDNS still looks noisy**

```bash
rg -n "6354|audit-enable yes|forward_smartdns_intl" \
  ~/.smartdns/smartdns.conf \
  ~/.mosdns/config/config.yaml
```

If that returns matches, the live files were not updated to the April 19, 2026 topology yet.

**mosdns will not start**

```bash
cat /tmp/mosdns.stderr.log
plutil -lint /Library/LaunchDaemons/com.mosdns.plist
launchctl print system/com.mosdns
```

**IPv6 path looks wrong**

```bash
dig @::1 jd.com AAAA
dig @127.0.0.1 google.com AAAA
```

If the intl fallback returns a CN CDN IPv6 address repeatedly, update both `cn_ip.txt` and `cn_ip_v6.txt` under `~/.mosdns/data/`.

## Updating Data Files

```bash
cd ~/.mosdns/data

curl -fsSL "https://raw.githubusercontent.com/felixonmars/dnsmasq-china-list/master/accelerated-domains.china.conf" | \
  sed 's/server=\/\(.*\)\/114.114.114.114/\1/' > cn_domain.txt

curl -fsSL -o cn_ip.txt "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"
curl -fsSL -o cn_ip_v6.txt "https://raw.githubusercontent.com/gaoyifan/china-operator-ip/ip-lists/china6.txt"

sudo launchctl kickstart -k system/com.mosdns
```

## Security Notes

- Steady-state upstream resolution is DoH-first.
- The bootstrap allowlist intentionally uses plain UDP to avoid circular dependency during DoH bootstrap.
- `smartdns` audit is disabled by default because Surge can misinterpret that local flood as an abnormal request rate.
- `mosdns` remains the main observability point via `/tmp/mosdns.log`.
