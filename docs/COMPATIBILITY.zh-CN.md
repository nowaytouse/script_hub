# 客户端兼容性

[English](COMPATIBILITY.md) · [简体中文](COMPATIBILITY.zh-CN.md)

预期使用目标是 Shadowrocket、最新版 Surge、支持 Surge 模块的 FlClash RS、使用独立规则提供器的 Mihomo 和最新版 sing-box。客户端版本、平台、设置和上游资源会影响实际加载效果。

`Sources/` 保存构建输入和上游参考，不是可直接使用的客户端配置。

| 目标 | 产物与边界 |
| --- | --- |
| Shadowrocket | 使用 Shadowrocket 专用模块和兼容规则列表。模块转换不保证上游脚本、重写或 MITM 行为逐项等价。 |
| Surge / FlClash RS | 共用 Script Hub 模块目录中的 Surge `.sgmodule` 链接。PROMAX 引用共用外部规则集，FlClash RS 导入后可查看有效和忽略的条目。详见[共用模块指南](FLCLASH_PREVIEW.zh-CN.md)。 |
| Mihomo / FlClash 路由 | 生成的规则提供器位于 `rulesets/Mihomo/NAME.yaml` 或 `rulesets/AdBlock/Mihomo/NAME.yaml`；设置 `behavior: classical` 和 `format: yaml`。`_supported.yaml` 包含遗漏或适配，数量见文件头。应使用规则提供器，而非 Surge 或 Shadowrocket 模块。 |
| 早期 FlClash 预览版 | `modules/flclash/` 下的适配产物针对早期固定版本解析器。详见[早期预览版兼容入口](FLCLASH_PREVIEW.zh-CN.md#早期预览版兼容入口)。 |
| sing-box | 使用生成的 `.srs` 规则集，不要使用 Surge 或 Shadowrocket 模块。支持目标为最新版。带 `_supported` 的文件仅包含支持的子集，不是完整规则集的等价版本。 |

域名、IP 和 extra 分类分开发布。请在客户端选择合适策略，并注意规则顺序与 DNS 解析；IP 分类可能包含 ASN 规则，并非一定是纯 IP-CIDR 集合。子集可能遗漏重要匹配条件。路由规则和二进制规则集均不能复现模块的脚本、HTTP 重写、响应修改或 MITM 效果。

Mihomo 对 Surge `extended-matching` 的适配仅保留基础域名匹配；TLS SNI 或 HTTP Host 匹配取决于目标客户端的嗅探配置，不保证与 Surge 等价。

SRS 转换不保留 Surge 逐条规则的 `no-resolve` 或 `extended-matching` 选项，这类适配产物也使用 `_supported` 名称；请引用当前文件，旧的未标记 SRS 名称不保留。解析行为取决于引用该规则集的 sing-box 路由和 DNS 配置，不由该选项决定。

## 217heidai 完整源

本地导出读取官方完整 DNS 发行版，核对声明条数，再生成一份共用的域名后缀规则。来源放行在去重前处理，并应用项目的服务白名单、直连清单、公共加密 DNS 和当前 PROMAX 的 MITM 主机保护。`manifest.json` 记录来源版本、SHA-256 和各阶段条数。

~~~sh
cargo run --locked --package rust_processor --bin promax -- --root . --export-heidai-bundle /path/outside/repository/new-heidai-bundle --singbox /path/to/sing-box
~~~

每次选择一个新的输出目录。省略 `--singbox` 时，sing-box 仍会收到可直接引用的源格式 JSON。

| 客户端 | 产物与导入 |
| --- | --- |
| Surge | 用 `RULE-SET,<路径>,REJECT` 引用 `.Surge.list`。路径相对配置文件目录，也支持绝对路径。 |
| Shadowrocket | 将 `.Shadowrocket.module` 导入为路由模块。 |
| Mihomo / FlClash 路由 | 在内核工作目录内使用 `.Mihomo.yaml`，配置 `type: file`、`behavior: classical`、`format: yaml`，为引用添加拦截策略。 |
| 早期 FlClash 预览版 | 从原生模块页导入 `.FlClash.sgmodule`，替换原 PROMAX。模块合并完整源的有效域名，并略过 PROMAX 已覆盖的后缀。新版 FlClash RS 的标准 PROMAX 使用 Surge 共用入口。 |
| sing-box | 注册 `.SingBox.srs`（本地二进制）或 `.SingBox.json`（本地源格式），在引用该规则集的路由或 DNS 规则中选择拦截动作。 |
| AdGuard Home | 从服务器允许的目录加载 `.AdGuardHome.txt` 作为 DNS 过滤器。 |

使用客户端对应的产物，并停用该上游的原始订阅，确保共用服务保护生效。完整源聚合了再发布条件不同的第三方数据，合并产物由本地导出器生成。来源放行仅约束这一个来源。

共享编译器保留本地应用、广告、隐私、安全和威胁情报分类。同分类、同动作内，普通域名后缀负责其覆盖的更窄普通规则。带显式匹配选项的规则和独立订阅保有各自的覆盖。217heidai 完整源以独立的混合来源记录。

参考：[Mihomo 规则提供器](https://wiki.metacubex.one/en/config/rule-providers/)、[规则内容](https://wiki.metacubex.one/en/config/rule-providers/content/)、[sing-box 规则集](https://sing-box.sagernet.org/zh/configuration/rule-set/)、[Surge 脚本](https://manual.nssurge.com/scripting/http-request.html)及 [Shadowrocket App Store 页面](https://apps.apple.com/us/app/shadowrocket/id932747118)。
