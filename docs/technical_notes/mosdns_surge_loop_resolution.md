# Technical Note: Resolving MosDNS & Surge Recursive Loops

**Date**: April 19, 2026  
**Status**: Resolved  
**Scope**: DNS Architecture Stability  

## 1. Executive Summary
Persistent system crashes and Surge "Network Buffer Full" errors were traced to a highly aggressive recursive DNS loop between Surge and MosDNS. The resolution involved port migration, bootstrap hardening, and architectural simplification.

## 2. Root Cause Analysis

### A. Port 53 Hijacking & Collision
Surge, by default, implements transparent hijacking of all traffic directed to UDP port 53. 
- **The Loop**: When MosDNS was configured to listen on port 53 and Surge was configured to use `127.0.0.1:53` as its `dns-server`, a recursive dependency was created. 
- **Collision**: Internal requests from MosDNS to public DNS servers (like `223.5.5.5:53`) were intercepted by Surge and sent back to MosDNS on port 53, creating an infinite loop that exhausted system network buffers within seconds.

### B. Hijack Ruleset Interference
The inclusion of granular hijacking rulesets (e.g., `hijack.list`) created unexpected side effects. These rules attempted to intercept and steer DNS traffic that was already being managed by MosDNS, leading to logic conflicts and resolution failures that triggered increased retry rates.

### C. MosDNS Aggressive Retry Logic
MosDNS is designed for high-performance and low-latency, but this results in an aggressive retry posture. When an upstream resolution fails (often due to the underlying loops described above), MosDNS increases the volume and frequency of its requests. In a looping environment, this "stubbornness" compounds the system load exponentially, even when process-level direct routing (binary/app-level bypass) was configured for the MosDNS process itself.

## 3. Implemented Solutions

### 1. Port Migration (The 6053 Shift)
MosDNS was migrated from the standard port 53 to **port 6053**. 
- This prevents Surge's default port 53 hijacking from capturing MosDNS's internal bootstrap traffic.
- Surge now explicitly points to `127.0.0.1:6053` for general resolution.

### 2. Bootstrap Hardening (The `syslib` Barrier)
To ensure MosDNS can always resolve its DoH upstreams (e.g., `dns.google`, `dns.alidns.com`), the Surge `[Host]` section was updated to force these domains to use the system library resolver:
```ini
[Host]
# Critical Bootstrap domains bypass MosDNS to prevent circularity
dns.google = server:syslib
cloudflare-dns.com = server:syslib
dns.alidns.com = server:syslib
```
This serves as a physical barrier that prevents any resolution required for the DoH handshake from ever being routed back into MosDNS.

### 3. Architectural Simplification
- **Architectural Simplification**: The DNS stack was flattened by removing redundant intermediate components (like SmartDNS) to minimize potential failures between local services.
- **Standalone MosDNS**: MosDNS now directly handles both international DoH (via Surge SOCKS5) and mainland DoH/UDP.

## 4. Verification Results
- **System Stability**: "Network buffer full" errors have ceased.
- **Verification Command**:
  ```bash
  sudo launchctl print system/com.mosdns | egrep "state"
  lsof -nP -i:6053
  ```
- **Smoke Test**: Successful resolution via `dig @127.0.0.1 -p 6053 google.com`.

---
*Created by Antigravity AI for nyamiiko.*
