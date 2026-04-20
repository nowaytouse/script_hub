# Technical Note: Fixing Surge ↔ Mihomo Spawn Failure (Errno 9)

## The Problem
When using Surge's `external, exec="/usr/local/bin/mihomo"` feature, users frequently encounter the following error after reloading configurations or switching DNS modes:

> **Failed to set posix_spawn_file_actions for fd 0 at index 0 with errno 9 (Path: /usr/local/bin/mihomo).**
> *Surge will not attempt to restart this program until the profile is reloaded.*

### Root Cause: `errno 9 (EBADF)`
This specific error is a **process spawning (`posix_spawn`) failure** at the OS level:
1. Surge tries to spawn a child process (`mihomo`).
2. It attempts to duplicate its own standard input (`fd 0`) to the child.
3. However, Surge (as a GUI/Background app) often has its `stdin` (fd 0) closed.
4. On macOS, attempting to `dup2` a closed file descriptor into a child process results in `EBADF`.

This explains the `mihomo` launch failure, but it does **not** explain every "all nodes red" situation by itself. In this repo, a second issue existed at the same time:

1. `Surge DNS -> mosdns -> Surge SOCKS5`
2. `test-timeout=1` on policy checks
3. `system` DNS fallback enabled beside local mosdns

That combination can make policy groups appear red even while cached connections still work.

---

## Solutions

### Solution A: The "Wrapper" Trick (Recommended for SubStore)
Since **SubStore** dynamically generates configurations and injects them via command-line arguments (`args="-config <base64>"`), a static service cannot be used. We must satisfy Surge's requirement for a valid `stdin` by using a wrapper script.

#### Implementation:
1. **Rename the binary**:  
   `sudo mv /usr/local/bin/mihomo /usr/local/bin/mihomo-bin`
2. **Create the wrapper** at the original path `/usr/local/bin/mihomo`:
   ```bash
   #!/bin/bash
   # provide a valid fd 0 via </dev/null
   exec /usr/local/bin/mihomo-bin "$@" </dev/null 2>>/tmp/mihomo.log
   ```
3. **Set Permissions**:  
   `sudo chmod 755 /usr/local/bin/mihomo`

**Result**: Every time Surge or SubStore calls `/usr/local/bin/mihomo`, the wrapper redirects `stdin` to `/dev/null`, preventing the `posix_spawn` failure.

The repo already includes this wrapper template at [mihomo/wrapper.sh](/Users/nyamiiko/Downloads/GitHub/script_hub/mihomo/wrapper.sh:1).

---

### Solution B: Persistent System Service (Best for Static Usage)
If your `mihomo` configuration is static or updated infrequently, running it as a `launchd` service is more stable and decouples it from Surge's reload lifecycle.

#### Implementation:
Create `/Library/LaunchDaemons/io.mihomo.core.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>io.mihomo.core</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/mihomo</string>
    <string>-d</string>
    <string>/Users/nyamiiko/.config/mihomo</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict>
</plist>
```
**Surge Connection**:  
Instead of using `exec`, simply connect to it as a remote proxy:
`ProxyName = external, addr=127.0.0.1, port=7891`

---

## Comparison Summary

| Metric | Solution A (Wrapper) | Solution B (Service) |
| :--- | :--- | :--- |
| **Effort** | Low | Medium |
| **SubStore Compatible** | **Yes (Best)** | No (Requires rework) |
| **Surge Reload Impact** | Restarts process | **Zero Impact** |
| **Log Management** | Redirected in script | Managed by launchd |

## Maintenance Note
When updating `mihomo` via scripts, the binary at `/usr/local/bin/mihomo` is often overwritten. Ensure your update script targets `/usr/local/bin/mihomo-bin` instead to keep the wrapper intact.

## Separate But Related Stability Fix
If Surge still shows all policy groups red after the wrapper is installed, treat that as a DNS/probe configuration issue, not another `posix_spawn` problem.

The stable combination used in this repo is:

- `mihomo` wrapped or run as a persistent service
- Surge `dns-server` pinned to local mosdns only
- if you use the dedicated mosdns Surge module, let that module own `encrypted-dns-follow-outbound-mode = false`
- `PROCESS-NAME,/usr/local/bin/mihomo*,DIRECT`
- relaxed policy probe timeouts (`timeout=5`, `test-timeout=5`)
- Local DNS steering focused on a unified MosDNS instance to minimize stack height
