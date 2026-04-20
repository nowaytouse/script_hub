# MosDNS v5 生产级运维手册 (Operations Manual)

本项目已成功从 SmartDNS-RS 迁移至 **MosDNS v5.3.4**，实现了 12w+ 规则的秒级匹配与极致的多样化分流。

## 1. 核心架构逻辑
*   **本地监听**：`127.0.0.1:6053` (UDP/TCP)。
*   **规则分流**：
    *   **国内**：并发查询阿里、腾讯、字节、百度、360、114 等节点，取最快结果。
    *   **国际**：按域名桶分流至 Google, Cloudflare, MS (Wikimedia), AdGuard, TWNIC 等节点。
*   **Surge 联动**：所有国际查询均通过 `127.0.0.1:6153` (SOCKS5) 转发，并利用备向 IP (如 8.8.4.4) 绕过 Surge 的劫持审计。

## 2. 自动化规则同步 (12w 规则更新)
当您的原始规则文件（在 `archive` 目录下）或 Surge 的 `Direct.list` 更新时，请执行：

```bash
# 1. 运行同步脚本
python3 generate_mosdns_rules.py

# 2. 重启 MosDNS 使规则生效 (大约需要 15 秒重新加载 Trie 树)
sudo killall mosdns
```

## 3. 服务启停与自启
服务已注册为 LaunchAgent，并开启了 `KeepAlive` (故障自动重启)。

*   **启动服务**：`launchctl start com.mosdns.service`
*   **停止服务**：`launchctl stop com.mosdns.service`
*   **完全卸载任务**：`launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.mosdns.service.plist`
*   **重新注册任务**：`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.mosdns.service.plist`

## 4. 日志审计与排错
*   **系统日志**：`tail -f /tmp/mosdns.log` (查看解析记录与分流匹配)。
*   **语法/启动报错**：`tail -f /tmp/mosdns.stderr.log`。

---
**当前状态：稳定运行 | 规避审计：已开启 | 上游多样性：极高**
