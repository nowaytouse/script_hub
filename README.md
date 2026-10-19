# Script Hub

面向 Surge、Shadowrocket、Loon 与 sing-box 的规则集和功能模块仓库。更新、网络抓取、合并、格式转换、校验与发布均由 Rust 完成。

## 广告拦截体系

- **PROMAX** 是唯一推荐安装的广告拦截模块：外部规则按用途分片，并合并经筛选的 URL Rewrite、Map Local、Body Rewrite、Script 与 MITM 功能段。
- **分类隔离**：Advertising、Privacy、Security、AntiAD 与各 ThreatIntel 类别由源清单显式声明；编译器不会按 URL 猜分类。
- **误屏蔽防护**：正常媒体、受保护服务、无对应请求规则的 MITM 主机会进入 `rulesets/AdBlock/quarantine.json`，不会直接发布。
- **Helper**：保留 DNS 补充、必要的 Host/系统解析映射和关键服务 MITM 排除，不注入分流策略；分流由外部规则集和用户策略组决定。端口阻断仍是独立、按需安装的模块。
- **旧 PROMAX 链接保持兼容**：
  `https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/surge/head_expanse/github/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule`

规则索引与分片说明见 `rulesets/AdBlock/README.md` 和 `rulesets/AdBlock/catalog.json`。

## 功能模块

- `modules/surge/amplify_nexus/`：可独立安装的增强、翻译、解锁与工具合集。
- `modules/shadowrocket/amplify_nexus/`：由 Rust 从已校验的 Surge 合集同步生成。
- `modules/source/local/`、`rulesets/Sources/LocalModules/`：编译输入，不应直接当作公开安装目录。
- 混合模块只把明确的广告行拆入 PROMAX；非广告增强功能继续留在独立模块中。
- 模块安装助手：`modules/helper/surge_module_helper.html`。其目录、兼容性和 Shadowrocket 状态均在同一次 Rust 更新中确定性刷新。

Apple 合集从 NSRingo 各官方 GitHub Release 的 `latest` 资源生成；WeatherKit、Maps、News、TV 与 DualSubs 不从公告文本或第三方转载页抓取。

## 游戏规则分流

`rulesets/RULE-SET/` 提供三个可独立选择策略的规则集，其 `github/` 镜像和 `rulesets/SingBox/` 下的 SRS 在同一 Rust 更新中生成：

| 文件 | 用途 |
| --- | --- |
| `Gaming_Download.list` | 明确的游戏下载、安装包、补丁及更新 CDN（含 Steam 中国下载节点） |
| `Gaming_Play.list` | 明确的游戏网络、匹配发现、好友/在线服务及对局相关端点 |
| `Gaming_Web.list` | 官网、商店、账号、社区，以及无法按域名拆分的共享平台兜底 |

Surge 按 **Download → Play → Web → Direct/GlobalProxy → FINAL** 排列，三个策略名称由用户自己的配置决定，例如：

```ini
RULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/RULE-SET/Gaming_Download.list,游戏下载
RULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/RULE-SET/Gaming_Play.list,游戏联机
RULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/RULE-SET/Gaming_Web.list,游戏平台
```

`Gaming_Web` 的平台父域名可包含 Download/Play 的子域名，故顺序是分流语义的一部分。域名无法区分同一主机上的网页、登录和对局；这类共享域名保留在平台兜底，不宣称是纯联机流量。专用联机组也不覆盖任意 IP 的 P2P 对局：如需此类覆盖，在用户配置中按具体游戏进程/设备追加规则，不全局匹配 UDP 或 443。

通用 Microsoft 登录域名 `login.live.com`、`logincdn.msauth.net` 不进入专用游戏组，以免游戏策略覆盖其他应用的账号登录；由用户的 Microsoft/普通分流规则处理。旧 `Gaming_ip.list` 也不自动并入专用联机组，其共享云 IP 无法仅凭地址确定是游戏流量。

旧 `Gaming.list` 继续保留兼容全集，包括既有进程/关键词规则；迁移到上述三个文件后，应移除旧全集引用，避免其抢先匹配。游戏下载组同时吸收 Sukka 官方 game-download 规则和本地精确端点；联机分类端点维护于 `rulesets/Sources/custom/Gaming_Play_manual.list`。更新时由 Rust 合并上游与手动分类，再生成镜像和 SRS。专用组不含启动器整进程、公司域名关键词或写死的 DIRECT/PROXY 策略。

## Rust 更新入口

```bash
cargo run --release --manifest-path create/processor/Cargo.toml --bin update -- \
  --root . --execute --singbox /path/to/sing-box

cargo run --manifest-path create/processor/Cargo.toml --bin validate -- --root .
```

`--quick` 仅跳过可选的功能模块上游刷新，PROMAX 规则仍会完整抓取、编译和验证。生成器会保留仅日期、注释、排版或顺序变化的旧文件，避免没有实质更新时产生自动提交。

下载器拒绝空响应和以 HTML 文档标记开头的错误页；GitHub 限流遵守 `Retry-After` 与已耗尽额度的重置时间，没有明确等待时间时从 60 秒开始退避，并受批次时限约束。通用规则集解析后若没有有效规则会报错并保留旧产物；若仅域名或 IP 一类变为空，则保留既有订阅路径并原子写入零条规则的占位文件，避免删除公开文件导致 404。

功能合集先在缓存中完成合并、安全处理与校验，再成组发布；YouTube 功能和广告源一起提交，后处理或任一输出失败时保留旧版。规则清理、镜像同步和 Shadowrocket 遍历的 I/O 失败会传到主更新，不再记作成功；去重至零的规则集保留公开路径，对应 SRS 使用空规则数组而不是空匹配器。下载遇到 404 时只在既有次数预算内切换到未尝试的备选入口，不重复请求已确认 404 的地址；发布时将显式 `refs/heads`、`refs/tags` 引用规范化，避免产生 `@refs/...` CDN 链接。

`--gaming-only` 用当前的 `Gaming.list`、游戏下载源结果与本地分类端点重新生成三类游戏规则及镜像，不下载上游源。带 `--singbox` 时同时同步 SRS；适合轻量验证游戏分类改动。

规则源在 `rulesets/Sources/Links/AdBlock_sources.list` 中以 `SOURCE|POLICY|KIND|CATEGORY` 明确登记；脚本/重写源在 `AdBlock_functional_sources.list` 中维护。项目自有更新流程不再使用 Go、Python 或 Shell 脚本。
