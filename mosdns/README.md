# mosdns DNS Setup for Surge

Complete dual-track DNS solution with zero cross-contamination between mainland and international domains.

## Architecture

```
Surge → 127.0.0.1:5335 (mosdns)
          ↓
    ┌─────┴─────┐
    │  mosdns   │
    └─────┬─────┘
          │
    ┌─────┴──────────────┐
    │                    │
CN domains          International
    │                    │
    ↓                    ↓
AliDNS/DNSPod      Cloudflare DoH
(direct)           (via SOCKS5 proxy)
```

## Features

- **Zero cross-contamination**: CN domains never hit international DoH, international domains never hit mainland DoH
- **Anti-pollution**: Validates response IPs against expected region
- **ECS support**: Adds EDNS Client Subnet for mainland queries
- **Caching**: 10K entry cache with lazy TTL and disk persistence
- **Auto-reload**: Domain/IP lists reload automatically on change

## Installation

```bash
cd ~/Downloads/GitHub/script_hub/mosdns
./install.sh
```

This will:
1. Download mosdns binary for your architecture (arm64/amd64)
2. Download geosite.dat, geoip.dat, CN domain list, CN IP list
3. Install configuration to `~/.mosdns/`
4. Create and load launchd service (auto-start on boot)

## Surge Configuration

Update your Surge config `[General]` section:

```ini
[General]
dns-server = 127.0.0.1:5335
# Remove encrypted-dns-server line — mosdns handles all DoH
```

Or use the provided Surge module:

```ini
#!name=mosdns DNS Integration
#!desc=Route all DNS queries to local mosdns (127.0.0.1:5335)

[General]
dns-server = 127.0.0.1:5335
```

## Verification

Test mainland domain:
```bash
dig @127.0.0.1 -p 5335 baidu.com
# Should resolve via AliDNS, return CN IP
```

Test international domain:
```bash
dig @127.0.0.1 -p 5335 google.com
# Should resolve via Cloudflare (via proxy), return non-CN IP
```

Check logs:
```bash
tail -f /tmp/mosdns.log
```

## Management

**Check status:**
```bash
launchctl list | grep mosdns
```

**Stop service:**
```bash
launchctl unload ~/Library/LaunchAgents/com.mosdns.plist
```

**Start service:**
```bash
launchctl load ~/Library/LaunchAgents/com.mosdns.plist
```

**Restart service:**
```bash
launchctl unload ~/Library/LaunchAgents/com.mosdns.plist && \
launchctl load ~/Library/LaunchAgents/com.mosdns.plist
```

**Update configuration:**
```bash
vim ~/.mosdns/config/config.yaml
# Then restart service
```

## Configuration Details

### Mainland DoH Upstreams
- Primary: `https://dns.alidns.com/dns-query`
- Secondary: `https://doh.pub/dns-query`

### International DoH Upstreams (via SOCKS5 proxy)
- Primary: `https://cloudflare-dns.com/dns-query`
- Secondary: `https://dns.google/dns-query`
- Proxy: `127.0.0.1:9810` (Surge SOCKS5 port)

### Domain Matching Logic
1. Check if domain matches CN domain list → mainland sequence
2. Check if domain ends with `.cn` → mainland sequence
3. Otherwise → international sequence

### Anti-Pollution
- Mainland sequence: Rejects responses with non-CN IPs
- International sequence: Rejects responses with CN IPs (pollution indicator)

## Troubleshooting

**mosdns not starting:**
```bash
# Check logs
cat /tmp/mosdns.stderr.log

# Test config manually
~/.mosdns/mosdns start -c ~/.mosdns/config/config.yaml
```

**DNS not resolving:**
```bash
# Verify mosdns is listening
lsof -i :5335

# Test directly
dig @127.0.0.1 -p 5335 example.com
```

**Surge not using mosdns:**
```bash
# Check Surge DNS config
# Ensure dns-server = 127.0.0.1:5335 is set
# Remove any encrypted-dns-server lines
```

## Updating Data Files

Data files auto-reload, but to manually update:

```bash
cd ~/.mosdns/data

# Update geosite/geoip
curl -L -o geosite.dat "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat"
curl -L -o geoip.dat "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat"

# Update CN domain list
curl -L "https://raw.githubusercontent.com/felixonmars/dnsmasq-china-list/master/accelerated-domains.china.conf" | \
    sed 's/server=\/\(.*\)\/114.114.114.114/\1/' > cn_domain.txt

# Update CN IP list
curl -L -o cn_ip.txt "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"
```

## Uninstallation

```bash
# Stop and remove service
launchctl unload ~/Library/LaunchAgents/com.mosdns.plist
rm ~/Library/LaunchAgents/com.mosdns.plist

# Remove installation
rm -rf ~/.mosdns

# Restore Surge DNS config
# Remove dns-server = 127.0.0.1:5335
# Add back encrypted-dns-server line
```

## Performance

- Cache hit rate: ~80-90% for typical usage
- Query latency: <5ms (cache hit), <50ms (cache miss)
- Memory usage: ~50-100MB
- CPU usage: <1% idle, <5% under load

## Security

- All international DoH queries go through Surge SOCKS5 proxy (encrypted)
- Mainland DoH queries go direct (faster, no privacy concern for CN domains)
- Anti-pollution validation prevents DNS hijacking
- No plaintext DNS queries (all DoH)
