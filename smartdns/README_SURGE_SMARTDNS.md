# Surge + SmartDNS 高性能架构全纪录 (V44.6 交付版)

## 核心架构
- **PF Redirection (Kernel Level)**: 使用 pfctl 将 `lo0` 的 53 端口 (UDP/TCP) 透明转发至 `6053` 端口，彻底绕过 macOS 权限限制与 DNS 环路。
- **Non-Recursive Steering**: 摒弃递归解析，采用“国内组 (cn) + 代理组 (proxy)”的明确分流策略。
- **Full Protocol Coverage**: 实现了对 TCP 和 UDP 解析请求的 100% 接管，解决了百度等复杂 CNAME 域名的超时难题。

## 解决的关键病灶
1. **Empty DNS answer**: 根本原因为 TCP 请求未被接管以及 Type 65 (HTTPS) 记录解析异常。已通过 PF TCP 转发与 `force-qtype-soa 65` 解决。
2. **DNS Loop**: 通过 address 指令锁定 DoH 服务器物理 IP，断开了解析器解析解析器域名的递归环路。
3. **Speed Race**: 移除了 `speed-check`，利用国内顶级 DoH + UDP 双并发实现了极限回包速度。

## 维护手册
- **添加直连域名**: 编辑 `ruleset/custom_direct.list`，运行 `scripts/merge_shadowrocket_rules.py`。
- **PF 规则持久化**: 指令已集成在 `setup_persistence.sh` 中。
