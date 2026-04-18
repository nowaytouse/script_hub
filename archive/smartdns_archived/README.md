# smartdns-rs — Stable CN Acceleration Beneath mosdns

`smartdns-rs` is now only the CN acceleration bucket beneath `mosdns`. It no longer owns the international path, which means the old `Surge -> mosdns -> smartdns -> Surge SOCKS5` loop is gone.

## Architecture

```text
Surge -> mosdns on 127.0.0.1:53 / [::1]:53
           |
           +-> Specialty and intl DoH stay in mosdns
           \-> CN domain bucket -> smartdns on 127.0.0.1:6353 / [::1]:6353
                                    -> AliDNS / doh.pub / dns.pub / 360 DoH
                                    -> speed-check + TTL smoothing + IPv6 preference
```

## What SmartDNS Adds

- Fastest-IP selection for CN answers with `speed-check-mode ping,tcp:443`
- TTL smoothing through `serve-expired yes`
- IPv6 preference through `dualstack-ip-selection yes`
- Lower local noise inside Surge because `audit-enable no`

## Installation

```bash
cd ~/Downloads/GitHub/script_hub/smartdns
./install.sh
```

The installer will:

1. Install or reuse the Homebrew `smartdns` formula.
2. Copy [smartdns.conf](/Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/smartdns.conf:1) to `~/.smartdns/smartdns.conf`.
3. Render [com.smartdns.plist](/Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/com.smartdns.plist:1) to `~/Library/LaunchAgents/com.smartdns.plist`.
4. Reload the `gui/$(id -u)/com.smartdns` LaunchAgent.

If your current install predates April 19, 2026, rerun both the `smartdns` and `mosdns` installers. Older live configs still expose `6354` and keep SmartDNS audit enabled.

## Ports

| Port | Role |
|---|---|
| `127.0.0.1:6353` / `[::1]:6353` | CN-only SmartDNS bucket queried by mosdns |

Nothing outside mosdns should use this port as a primary resolver.

## Verification

Check the live job:

```bash
launchctl print gui/$(id -u)/com.smartdns | rg "state =|program ="
```

Check that the live config is the post-refactor version:

```bash
rg -n "6354|audit-enable yes" ~/.smartdns/smartdns.conf ~/.mosdns/config/config.yaml
# Expected: no matches
```

Check the installed plist:

```bash
plutil -p ~/Library/LaunchAgents/com.smartdns.plist | rg "HardResourceLimits|WorkingDirectory"
```

Smoke-test the CN bucket and then the end-to-end mosdns path:

```bash
dig @127.0.0.1 -p 6353 jd.com +stats
dig @::1 -p 6353 jd.com +stats
dig @127.0.0.1 jd.com
dig @127.0.0.1 news.ycombinator.com
tail -f /tmp/smartdns.log
```

## Management

### Golden Specification for launchd

1. **bootout** → Clean up old instances
2. **bootstrap** → Register service
3. **kickstart** → Run service
4. **print** → Verify status

### Operations

```bash
# 1. Status
launchctl print "gui/$(id -u)/com.smartdns" | egrep "state|program"

# 2. Complete Reload (Golden Lifecycle)
launchctl bootout "gui/$(id -u)" ~/Library/LaunchAgents/com.smartdns.plist 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" ~/Library/LaunchAgents/com.smartdns.plist
launchctl kickstart -k "gui/$(id -u)/com.smartdns"

# 3. Quick Restart (Config reload only)
launchctl kickstart -k "gui/$(id -u)/com.smartdns"
```

### Unified Management Function

Add this to your `.zshrc` or `.bashrc` for easy management:

```bash
reload_service() {
  local domain=$1
  local plist=$2
  local label=$3

  # Use sudo only if domain is 'system'
  local cmd_prefix=""
  [[ "$domain" == "system" ]] && cmd_prefix="sudo "

  $cmd_prefix launchctl bootout "$domain" "$plist" 2>/dev/null || true
  $cmd_prefix launchctl bootstrap "$domain" "$plist"
  $cmd_prefix launchctl kickstart -k "$domain/$label"
}

# Usage:
# reload_service "gui/$(id -u)" ~/Library/LaunchAgents/com.smartdns.plist com.smartdns
```


## Audit and Noise Control

`audit-enable no` is intentional. The older audit-enabled layout could make Surge flag SmartDNS as a local resolver sending very high request volume. The current design keeps SmartDNS limited to the CN bucket and leaves `/tmp/mosdns.log` as the main end-to-end log.

## Troubleshooting

**Surge still reports SmartDNS as red or excessively noisy**

```bash
rg -n "6354|audit-enable yes" ~/.smartdns/smartdns.conf ~/.mosdns/config/config.yaml
```

If that returns matches, the live files were not updated yet.

**SmartDNS will not start**

```bash
cat /tmp/smartdns.stderr.log
plutil -lint ~/Library/LaunchAgents/com.smartdns.plist
launchctl print gui/$(id -u)/com.smartdns
```

**CN answers look obviously wrong**

Inspect the SmartDNS log and temporarily remove the bad upstream from `~/.smartdns/smartdns.conf`, then restart the LaunchAgent.

**Port 6353 is already in use**

```bash
lsof -nP -iTCP:6353 -sTCP:LISTEN
lsof -nP -iUDP:6353
```

## Uninstallation

```bash
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.smartdns.plist
rm ~/Library/LaunchAgents/com.smartdns.plist
rm -rf ~/.smartdns
brew uninstall smartdns
```

If you remove SmartDNS completely, also update `~/.mosdns/config/config.yaml` or rerun the mosdns installer so the CN bucket does not keep pointing at `127.0.0.1:6353`.
