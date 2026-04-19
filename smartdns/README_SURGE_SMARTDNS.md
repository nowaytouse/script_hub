# Surge + SmartDNS 内核级分流实战手册 (V41.0 Production)

本手册详细记录了如何在 macOS (Surge Mac v5.x Beta) 环境下，构建一套基于 **内核级重定向 (PF)** 和 **协议脱敏 (SmartDNS)** 的高性能 DNS 分流系统。

---

## 1. 核心架构设计

### 1.1 分流架构流水线
1.  **Surge VIF (拦截)**：捕捉所有 53 端口流量。
2.  **PF Redirection (内核转发)**：将 `127.0.0.1:53` 的 UDP/TCP 流量无感转发至 `127.0.0.1:6053`。
3.  **SmartDNS (中枢决策)**：
    - **境内域名**：根据 `cn_list` 匹配，走阿里/腾讯/114 直连阵列。
    - **境外域名**：通过 `socks5://127.0.0.1:6153` 回源 Surge 进行代理解析，确保 100% 净化。
4.  **Surge skip-proxy (防回圈)**：确保 SmartDNS 的上游查询 IP 物理直连，防止产生 DNS 递归死循环。

## 2. 关键路径与配置

| 职能 | 物理路径 |
| :--- | :--- |
| **持久化配置** | `~/.config/smartdns/smartdns.conf` |
| **国内域名集** | `~/.config/smartdns/cn.txt` |
| **系统日志** | `/tmp/smartdns.log` |
| **启动项 (LaunchAgent)** | `~/Library/LaunchAgents/com.smartdns.service.plist` |

## 3. 日常维护手册

### 3.1 如何更新规则列表
1.  将新的域名列表覆盖至 `~/.config/smartdns/cn.txt`。
2.  执行重启命令：`launchctl stop com.smartdns.service`（服务会自动拉起并重新加载文件）。

### 3.2 确认重定向状态 (PF)
如果发现本地 `dig @127.0.0.1` 无法工作，请检查 PF 状态：
```bash
sudo pfctl -s nat
# 预期应包含: rdr pass on lo0 proto udp from any to 127.0.0.1 port 53 -> 127.0.0.1 port 6053
```

### 3.3 重启 Mac 后的恢复
如果您没有将 PF 写入系统启动配置，重启后需手动运行分流补全脚本：
```bash
bash /Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/setup_persistence.sh
```

## 4. 故障排查 (XP 记录)

### 4.1 错误: POSIX:61 (Connection Refused)
- **原因**：SmartDNS 未运行，或 53 端口权限被系统锁定。
- **解法**：通过 PF 转发绕过 53 端口所有权争议。

### 4.2 错误: Empty DNS answer
- **原因**：IPv6 (AAAA) 查询超时，或上游并发探测失败。
- **解法**：在配置中启用 `ipv6-reply-empty yes` 并合理平铺 `nameserver`。

---
**开发者寄语**：这套系统结合了内核重定向的隐蔽性和 SmartDNS 的高性能分流能力，是目前 macOS 平台最稳健的 DNS 加速方案。
