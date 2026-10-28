# PROMAX 扩充来源与许可

## 本次新增的许可来源

| 来源 | 用途 | 许可与原始内容 |
| --- | --- | --- |
| app2smile/rules | 起点、贴吧、腾讯新闻的 Surge 响应去广告模块，贴吧规则配套 | [MIT](app2smile-MIT.txt)；[官方模块与源代码](https://github.com/app2smile/rules) |
| AdguardTeam/cname-trackers | CNAME 伪装追踪、邮件追踪域名；与已有广告域名分类分开 | [MIT](AdGuard-CNAME-MIT.txt)；[分类说明](https://github.com/AdguardTeam/cname-trackers) |

app2smile 模块由现有 Rust 编译器下载、去重、筛选广告功能并合入 PROMAX；保留官方脚本 URL，不镜像脚本。贴吧默认不启用过滤普通视频帖，模板参数在合并时固定为 `0`。外部 RULE-SET 不能绕过本仓库编译器的白名单、分类和语法校验；贴吧配套列表先编入本地活动分片。自动更新继续沿用现有频率与网络限速，不为每个源创建独立 workflow。

AdGuard 仅启用广告、追踪和邮件追踪类别；不启用会影响正常跳转或页面的 clickthrough/microsites 全集。重复域名和已有父域覆盖不会重复发布，Apple、媒体与正常图片等安全保护继续生效。这些列表不需要 HTTPS 解密，新增脚本解密范围仅限各 App 的精确请求主机。

CNAME 追踪单独归入 `Privacy_CNAME`，Surge / Shadowrocket 自动引用完整分片；已退役的 Loon 产物不再生成或提供兼容入口，不截断或伪报规则数量。

上述模块的变更包括格式转换、默认参数落地、安全隔离、MITM 按实际引用收束，以及重复匹配的优先级处理；不代表上游作者为本项目或所有客户端提供运行保证。合并产物中的对应内容保留上游 MIT 许可，本项目不更改这些内容的许可。

## Pinterest 服务族规则来源

`SocialMedia_sources.list` 通过现有 Rust 更新器跟随 [MetaCubeX 的 Pinterest 数据](https://github.com/MetaCubeX/meta-rules-dat/blob/meta/geo/geosite/pinterest.list)，覆盖地区独立域名、短链接及 Pinterest 专属 CDN 别名，不扩展到整个 Fastly 或 Cloudflare 父域。该输入按 MetaCubeX 仓库的 [GPL-3.0 许可](https://github.com/MetaCubeX/meta-rules-dat/blob/master/LICENSE) 使用，GPLv3 正文已随本仓库根目录 `LICENSE` 提供；不推定其中的条目获得其他许可或重新许可。生成时仅转换匹配格式、去重、稳定排序和执行本仓库的分类保护，不添加用户路由策略。

日本网页入口单独由 `JP_manual.list` 维护；共享 `pinimg.com` 不划为日本。服务族用于分流匹配，不能作为广告拒绝名单，本文也不声称已审查所有既有第三方来源的许可。

## luestr 资源的接入边界

[ProxyResource 官方 README](https://github.com/luestr/ProxyResource) 说明仓库是索引、只对配置模板开源，并附带转载条件；[官方插件中心](https://hub.kelee.one/) 的受限资源不能当作可自由再分发的脚本。当前没有获得适用于 PROMAX 聚合发布的额外许可，因此不新增复制、转换或镜像，也不伪装新版 Loon User-Agent 获取资源。可在 Loon 中按作者的官方说明单独安装；这不等同于 Surge 兼容。高德现有功能继续保留，本次不将另一份未经许可的高德脚本叠加进来。
