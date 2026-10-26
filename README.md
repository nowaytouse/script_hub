# Script Hub

面向 Surge、Shadowrocket、Loon 与 sing-box 的规则集和功能模块仓库。更新、网络抓取、合并、格式转换、校验与发布均由 Rust 完成。

## 广告拦截体系

- **PROMAX** 是唯一推荐安装的广告拦截模块：外部规则按用途分片，并合并经筛选的 URL Rewrite、Map Local、Body Rewrite、Script 与 MITM 功能段。
- **分类隔离**：Advertising、Privacy、Security、AntiAD 与各 ThreatIntel 类别由源清单显式声明；编译器不会按 URL 猜分类。
- **误屏蔽防护**：正常媒体、受保护服务、无对应请求规则的 MITM 主机会进入 `rulesets/AdBlock/quarantine.json`，不会直接发布。
- **防止不完整更新**：稳定广告分类合计不得低于上一份 catalog 的 90%。HaGeZi TIF medium 是会清退过期域名的滚动情报源，仅官方地址、完整元数据、声明条数与逐条域名校验全部通过后才独立计算；去重和安全过滤后的 TIF 分片还须保留本次源条目的至少 80%。检查失败时保留已发布产物，不以旧情报永久冻结上游更新。catalog 记录上次实质更新时的源版本与条数，仅源版本时间戳改变不触发写入。
- **Helper**：保留 DNS 补充、必要的 Host/系统解析映射和关键服务 MITM 排除，不注入分流策略；分流由外部规则集和用户策略组决定。端口阻断仍是独立、按需安装的模块。
- **旧分片兼容**：已发布但不再需要的规则分片保留为空规则兼容文件，不加入新版模块或活动 catalog；旧模块缓存不会因分片缩减而遇到 404，旧拦截条目也不会继续生效。
- **旧 PROMAX 链接保持兼容**：
  `https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/surge/head_expanse/github/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule`

规则索引与分片说明见 `rulesets/AdBlock/README.md` 和 `rulesets/AdBlock/catalog.json`。

所有 PROMAX 上游规则（包括 Sukka 的 `REJECT-DROP` / `REJECT-NO-DROP`）均经过同一白名单和安全检查，再以原拒绝策略合并；模块不直接加载绕过检查的第三方规则集。正常视频 CDN 与整段播放接口的拒绝/空响应会被隔离，官方广告响应处理脚本继续保留。

### Apple 服务与分流

Apple Music、iTunes 与 Apple 自有媒体端点归入 `Apple.list`，不混入通用 `CDN.list` / `GlobalProxy.list`。使用时把 Apple 规则放在通用 CDN、代理兜底规则之前，并在自己的配置中选择策略；文件头的 `Policy` 只是推荐说明，不会改变客户端实际分流。例如希望 Apple 服务直连时：

```ini
# 若单独使用 AppleNews 地区服务策略，应先放 AppleNews，再放 Apple。
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/RULE-SET/Apple.list,DIRECT
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/RULE-SET/Apple_ip.list,DIRECT,no-resolve
# 用户自己的 CDN / GlobalProxy 规则放在后面。
```

账号、iCloud、证书校验与音乐播放/鉴权端点排除 MITM 和固定/第三方 Host 映射；WeatherKit、TV 等明确的功能端点仍保留。此边界用于降低连接破坏风险，不降低 TLS 校验强度，也不能据此认定某次账号锁定的原因。端点依据见 [Apple 网络要求](https://support.apple.com/en-us/101555)。

### 移动端运行开销

保留现有广告源、规则分片和响应处理功能，不通过裁剪规则、强制脚本引擎或统一降低 body 大小上限换取静态指标。编译器会把简单 HTTP 主机正则中的跨路径通配符限制在 URL authority 内，避免路径中的域名文本误触发脚本/重写；MITM 仅在请求主机可被严格证明时收束到实际引用的完整主机，复杂正则保留兼容范围，排除项始终先于包含项。

Surge 已为大型规则集建立索引；规则数量、MITM 主机条数和脚本匹配次数不能直接换算成电池收益。实际耗电需在同一设备、网络、应用负载及缓存状态下对照观测。相关语义见 [Surge 规则集](https://manual.nssurge.com/rules/ruleset.html)、[脚本与 body 缓冲](https://manual.nssurge.com/scripting/overview.html)。

若发布失败且回滚也失败，更新器会报告恢复路径并保留 staging/backup，后续更新会停止覆盖这些恢复数据；检查并恢复后再继续发布，不要直接删除错误信息所指的备份目录。

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

### Rust GUI（Windows / macOS）

原生 Rust 桌面界面面向 Windows 与 macOS；Linux 继续支持命令行处理器，但不构建此 GUI。桌面界面与命令行共用更新和校验实现，不需要 Node.js、Python 或另外安装更新器。界面包含概览、模块/规则/来源目录、任务、仓库、设置和帮助页面：可搜索模块与规则、复制订阅地址、管理 Wool / Weibo / Devtools 自定义来源，并查看任务和仓库状态。目录浏览和仓库状态读取不自动执行网络操作。

```bash
cargo run --release --locked --manifest-path create/processor/Cargo.toml \
  --features gui --bin script-hub-gui -- --root .
```

GUI 需要 Rust 1.95+；`gui` 为可选功能，普通规则更新 CI 不编译图形依赖。[Rust GUI 工作流](https://github.com/nowaytouse/script_hub/actions/workflows/gui.yml)在 Windows 与 macOS 构建。下载时请按制品名称中的操作系统和实际 runner 架构选择；Windows 提供 `.exe`，macOS 提供 `.app` 的 tar.gz 包。选择仓库目录作为工作区；显式传入的 CLI `--root` 优先于已保存的工作区路径。

macOS 包会进行本地 ad-hoc 签名和完整性校验，但开发构建尚无受信任开发者签名或公证，不声称通过平台商店或系统发行认证。GUI 写入任务之间由操作系统文件锁互斥，锁由实际执行更新的子进程持有，退出后自动释放；不要同时从外部命令行运行另一套更新。

浏览与只读校验不联网；联网与写入操作要求确认，每次只运行一个后台任务。支持只读校验、目录刷新、全量/快速更新和游戏规则更新。进度展示来自真实阶段和经过时间；不具备可信测量依据时不显示估算百分比。界面显示真实退出结果、警告和有界日志。联网更新沿用 `https_proxy` / `http_proxy` / `all_proxy` 环境变量，不更改系统代理、不自动提交或推送。

仓库页提供人工触发的 fast-forward-only pull、按明确选择文件的 commit 和非强制 push；不会自动执行 Git 操作。自定义来源保存在工作区 `modules/source/custom_sources.json`，添加和移除均需确认，并在下一次全量更新中重新合并对应合集；快速更新跳过功能合集。未指定 sing-box 时保留现有 SRS，不声称已更新 SRS。

偏好设置保存在 macOS `~/Library/Application Support/Script Hub/settings.json` 或 Windows `%APPDATA%\ScriptHub\settings.json`。设置模型支持主题、强调色、系统/简体中文/英文/日文、80–150% 界面缩放、窗口尺寸和紧凑度；保存的工作区路径不会覆盖显式 CLI `--root`。

默认启动只读取应用自身设置，不自动扫描工作区或探测父目录。可手动加载、拖入项目文件夹，或明确开启“启动时重新打开已保存的工作区”；CLI `--root` 也表示明确打开该路径。目录加载在单个后台线程执行，等待权限时仍可操作界面和关闭窗口，不会重复启动读取任务。

macOS 的桌面、文稿、下载、网络卷和可移动卷受系统权限控制。当前 ad-hoc 开发签名的权限身份与具体构建绑定，更换构建后可能需要重新授权；项目不能绕过系统同意，也不要求完全磁盘访问。无人值守建议使用明确指定、位于上述受保护目录之外的本地仓库，并从固定位置运行同一构建；跨版本稳定授权的发行版需要开发者提供一致的 Developer ID 签名身份。此限制与“记住路径”不同，不能靠保存路径消除。

桌面 GUI 不等于 iOS / Android 原生应用；手机和平板可继续使用 `modules/helper/surge_module_helper.html` 响应式安装助手。该网页包含实际目录界面和无脚本时的可见说明，可下载后本地打开；GitHub Raw / 部分 CDN 可能直接显示源码或下载 HTML，不能当作已部署的网页站点。

### 命令行

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
