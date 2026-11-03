# 客户端兼容性

[English](COMPATIBILITY.md) · [简体中文](COMPATIBILITY.zh-CN.md)

预期使用目标是 Shadowrocket、最新版 Surge、Mihomo 内核（包括使用该内核的 FlClash）和最新版 sing-box。其他客户端不属于发布或兼容性目标。客户端版本、平台、设置和上游资源均可能影响实际效果；仓库检查通过不等于设备实测通过。

`Sources/` 保存构建输入和上游参考，不是可直接使用的客户端配置。

| 目标 | 产物与边界 |
| --- | --- |
| Shadowrocket | 使用 Shadowrocket 专用模块和兼容规则列表。模块转换不保证上游脚本、重写或 MITM 行为逐项等价。 |
| Surge | 使用 Surge 模块和规则列表。支持目标为最新版，不保证历史版本。 |
| Mihomo / FlClash | 生成的规则提供器位于 `rulesets/Mihomo/NAME.yaml` 或 `rulesets/AdBlock/Mihomo/NAME.yaml`；设置 `behavior: classical` 和 `format: yaml`。`_supported.yaml` 包含遗漏或适配，数量见文件头。应使用规则提供器，而非 Surge 或 Shadowrocket 模块。 |
| sing-box | 使用生成的 `.srs` 规则集，不要使用 Surge 或 Shadowrocket 模块。支持目标为最新版。带 `_supported` 的文件仅包含支持的子集，不是完整规则集的等价版本。 |

域名、IP 和 extra 分类分开发布。请在客户端选择合适策略，并注意规则顺序与 DNS 解析；IP 分类可能包含 ASN 规则，并非一定是纯 IP-CIDR 集合。子集可能遗漏重要匹配条件。路由规则和二进制规则集均不能复现模块的脚本、HTTP 重写、响应修改或 MITM 效果。

Mihomo 对 Surge `extended-matching` 的适配仅保留基础域名匹配；TLS SNI 或 HTTP Host 匹配取决于目标客户端的嗅探配置，不保证与 Surge 等价。

SRS 转换不保留 Surge 逐条规则的 `no-resolve` 或 `extended-matching` 选项，这类适配产物也使用 `_supported` 名称；请引用当前文件，旧的未标记 SRS 名称不保留。解析行为取决于引用该规则集的 sing-box 路由和 DNS 配置，不由该选项决定。

参考：[Mihomo 规则提供器](https://wiki.metacubex.one/en/config/rule-providers/)、[规则内容](https://wiki.metacubex.one/en/config/rule-providers/content/)、[sing-box 规则集](https://sing-box.sagernet.org/zh/configuration/rule-set/)、[Surge 脚本](https://manual.nssurge.com/scripting/http-request.html)及 [Shadowrocket App Store 页面](https://apps.apple.com/us/app/shadowrocket/id932747118)。
