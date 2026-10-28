# Script Hub

面向 Surge、Shadowrocket 与 sing-box 的规则集和功能模块仓库。更新、网络抓取、合并、格式转换、校验与发布均由 Rust 完成。

## 广告拦截体系

- **PROMAX** 是唯一推荐安装的广告拦截模块：外部规则按用途分片，并合并经筛选的 URL Rewrite、Map Local、Body Rewrite、Script 与 MITM 功能段。
- **分类隔离**：Advertising、Privacy、Security、AntiAD 与各 ThreatIntel 类别由源清单显式声明；编译器不会按 URL 猜分类。
- **误屏蔽防护**：正常媒体、受保护服务、无对应请求规则的 MITM 主机会进入 `rulesets/AdBlock/quarantine.json`，不会直接发布。
- **防止不完整更新**：稳定广告分类合计不得低于上一份 catalog 的 90%。HaGeZi TIF medium 是会清退过期域名的滚动情报源，仅官方地址、完整元数据、声明条数与逐条唯一域名校验全部通过后才独立计算；跨活动分片的实际域名后缀匹配仍须覆盖本次源条目的至少 80%，不会把在其他分类已去重覆盖的规则误算为丢失。检查失败时保留已发布产物，不以旧情报永久冻结上游更新。catalog 记录上次实质更新时的源版本与条数，仅源版本时间戳改变不触发写入。
- **DNS/Host 助手模块**：保留 DNS 补充、必要的 Host/系统解析映射和关键服务 MITM 排除，不注入分流策略；分流由外部规则集和用户策略组决定。端口阻断仍是独立、按需安装的模块。此功能模块与已移除的网页安装助手不同。
- **仅发布当前版本**：不再生成旧 `AdBlock.list`、已合并的细分类订阅或退役空分片。删除产物前保留必要的内部构建输入，当前模块只引用活动 catalog；旧配置必须迁移，不再以空文件或别名维持旧订阅。
- **当前 PROMAX 入口**：
  `https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/surge/head_expanse/github/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule`

规则索引与分片说明见 `rulesets/AdBlock/README.md` 和 `rulesets/AdBlock/catalog.json`。

所有 PROMAX 上游规则（包括 Sukka 的 `REJECT-DROP` / `REJECT-NO-DROP`）均经过同一白名单和安全检查，再以原拒绝策略合并；模块不直接加载绕过检查的第三方规则集。正常视频 CDN 与整段播放接口的拒绝/空响应会被隔离，官方广告响应处理脚本继续保留。

共享搜索图片代理（百度 `t0`–`t12.baidu.com`、`dss0`–`dss3.bdstatic.com`、`ss0`–`ss3.bdstatic.com`）按精确主机保护，保留 YouTube 已有图片 CDN 保护并补充频道头像主机；整域拒绝与透明图替换不得覆盖正常缩略图。专用广告主机仍参与拦截；这些保护不向整个 `baidu.com`、`bdstatic.com` 或 `googleusercontent.com` 扩大。

### Apple 服务与分流

Apple Music、iTunes、Apple News 与 Apple 自有媒体端点归入 `Apple.list` / `Apple_ip.list`，不混入通用服务分类。`AppleNews.list` 已退役；共享 CDN 的整个父域不会被当成 Apple 专有域名。

规则集只描述匹配范围，不指定路由、节点或拦截动作。所有普通分类头统一标记 `Policy: USER-DEFINED`，不提供默认策略或节点。文件名不代表强制路由；实际动作和规则顺序始终由使用者自己的客户端配置决定。NSFW 作为 JP 组合的内部分类输入，不加入 PROMAX 的广告拒绝输入。

### 服务分类与自动更新

日常配置可优先使用以下五个合并入口。文件名表示匹配集合，不指定节点、路由或拒绝动作；请在自己的客户端配置中选择策略。

| 简化入口 | 包含内容 | 可替代的旧引用 |
| --- | --- | --- |
| `JP.list` | NSFW 分类、日本流媒体、Misskey.io、Pixiv 生态 | `NSFW.list`、`StreamJP.list`、单独的 Misskey/Pixiv 规则 |
| `Apple.list` | Apple 服务、音乐、Apple News | `Apple.list`、`AppleNews.list` |
| `TW.list` | 台湾及香港流媒体 | `StreamTW.list`、`StreamHK.list` |
| `us.list` | Spotify、StreamEU 与 StreamUS | `Spotify.list`、`StreamEU.list`、`StreamUS.list` |
| `GlobalProxy.list` | 原全球集合、CDN、Cloudflare、Sub-Store 与 Binance | `GlobalProxy.list`、`CDN.list`、`Cloudflare.list`、`substore.list`、`Binance.list` |

合并集合按源列表更新、去重并稳定排序。小写 `us` 合并 Spotify、StreamEU 与 StreamUS 的域名和有效 extra；`us_ip.list` 合并这些源中的有效 IP 条目，当前来自 Spotify，不限制未来其他成员的 IP 更新。`us` 包含欧洲流媒体，是用户指定的组合，不表示所有端点位于美国。Binance 并入 GlobalProxy。`JP` 包含按使用偏好合并的全球 NSFW 站点，不表示这些站点全部位于日本；`TW` 同样明确包含香港服务。已替代的公开细分类和 `Gaming.list` 全集已移除，不再保留兼容订阅。上游清单仍自动更新；仅供聚合与游戏细分使用的中间产物保存在 `rulesets/Sources/compiled/`，不应订阅该目录。Pixiv 专属域名由维护源和明确端点收束到 JP，不把共享 CDN 父域整体划为日本。

### 域名、IP 与其他匹配器

- `<分类>.list` 只包含 `DOMAIN`、`DOMAIN-SUFFIX`、`DOMAIN-KEYWORD`、`DOMAIN-WILDCARD`、`DOMAIN-REGEX`。
- `<分类>_ip.list` 只包含 `IP-CIDR`、`IP-CIDR6`、`IP-ASN`、`GEOIP`。仅在存在有效条目时发布，不为凑齐域名/IP 配对生成空文件；没有 IP 文件表示当前没有可发布的 IP 规则。
- `<分类>_extra.list` 只在需要时提供进程、User-Agent、URL 等非地址匹配，按需独立引用，不能混称为域名或 IP。PROMAX 也按相同边界将其活动广告分片拆为域名、`_ip` 与 `_other_extra`，原拒绝动作不变。

所有公开规则文件必须至少包含一条有效规则，`Rules` 按当前文件去重后的有效条目逐条统计，无显示上限或截断采样；PROMAX 的分片大小只决定文件组织，全部规则和各分片真实条数仍完整记录。失效下载或全无有效输入会报错并保留上次有效产物，不以空文件冒充成功更新。

旧静态 `Singbox1.13.0+.conf` 因没有维护链且引用多项已退役规则集而移除；不将新合集机械替换进其旧策略，避免改变使用者路由范围。当前 SRS 以实际发布文件为准。

sing-box 与 Surge 的匹配能力不同。含 User-Agent、URL、GEOIP、ASN 等当前转换器无法无损表示的文件，不会再无声地漏项并冒充完整 SRS：仍能表示的 CIDR、域名、进程等会保存在明确标为 `_supported_Singbox.srs` 的子集中，日志说明覆盖限制，旧的无标注 SRS 移除。完全可转换的列表继续使用常规 SRS 名称；Surge 的完整列表与模块不丢失这些匹配器。

`Profile_Exceptions` 是所给配置中明确的域名例外集合，不是新的通用服务分类，也不指定策略。私人进程路径、端口和地区兜底保留在本地配置的具名 `[Ruleset ...]` 段，不发布到公共仓库。

合并边界只限上表明确列出的成员，不按“当前恰好使用相同策略”把无关服务归为一组。Bilibili、PayPal、Google、Microsoft、AI、社交，以及游戏的下载/联机/网页类别继续独立；Spotify、StreamEU、StreamUS 仅合并为小写 `us`。CDN 与其他分类可能有正常交集，GlobalProxy 因此次明确合并而继承 CDN、Cloudflare、Sub-Store 与 Binance 的交集；规则顺序与动作由用户配置控制，不能把合集当成互斥的服务分类。

- `AI.list`：补充 MetaCubeX 的 Anthropic 分类源。
- `Apple.list`：补充 Apple TV / TV+ 与受维护的 Apple DNS 精确别名；Apple IP 继续独立存放。
- `Bilibili.list`：合并 blackmatrix7 的 BiliBili 服务列表。
- `Google.list` / `Microsoft.list`：分别补充 Google Drive / OneDrive 官方分类源。
- `GlobalProxy.list`：补充 Dropbox 与 MetaCubeX 开发生态聚合源；`SocialMedia.list` 补充通信聚合源，独立广告平台不作为社交服务收录。

新增上游合并进现有订阅文件，不需要新增 `RULE-SET` 引用或更改使用者的策略组。聚合分类可能与服务专集重叠，不是互斥的路由指令；Apple 自有端点按独立所有权边界归入 Apple 分类，共享提供商父域保留。源清单位于现有 `rulesets/Sources/Links/*_sources.list`，由同一个 Rust/CI 更新器自动发现、抓取、校验、生成 GitHub 镜像与 SRS；上游失败时保留上一份有效产物。

Pinterest 的地区独立域名、短链接与专属 CDN 别名随维护中的服务族源合入 `SocialMedia.list`；`jp.pinterest.com` 与 `pinterest.jp` 明确归入 JP。`pinimg.com` 是多个地区共用的图片基础设施，不随日本网页入口划为 JP，也不把 `pinterest.map.fastly.net` 扩大成整个 Fastly 父域。若使用地区入口覆盖，应将 JP 规则放在通用服务／兜底集合之前，实际策略由使用者决定。Cloudflare 的国际 `cloudflareanycast.net` 与中国网络 `cloudflarecn.net`，Google／Microsoft 的已有中国服务域名继续沿原服务族自动更新，不为已被父域覆盖的主机重复造规则。

规则集使用固定元数据字段、去重后的字典序与统一换行；同一输入顺序不同不产生不同产物。模块的 Script/Rewrite 则保留有意义的执行优先级，不为追求视觉排序改变先匹配行为。仅日期、注释或无实质差异的排版变化不触发发布；本次策略推荐字段中立化是一次明确的元数据迁移。

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
- 模块与规则集的查找、筛选和复制统一使用 Rust GUI 资源库。目录按现有产物只读生成，不再维护额外的网页和嵌入 JSON 副本。

Apple 合集从 NSRingo 各官方 GitHub Release 的 `latest` 资源生成；WeatherKit、Maps、News、TV 与 DualSubs 不从公告文本或第三方转载页抓取。

旧快照中的美丽修行与闲鱼脚本引用由 Rust 资源适配层迁移到当前维护源；仅替换这两种已确认失效的资源族，不改请求匹配或脚本参数。每次 PROMAX 编译会对实际引用的两个维护资源做有次数、体积与时间上限的 HTTPS 内容检查，404、TLS 失败或 HTML/空响应都会阻止这轮不完整发布，保留上次有效产物；发布校验也拒绝旧失效引用重新进入 Surge 或 Shadowrocket。脚本继续从维护源加载，不复制无明确授权的第三方脚本，也不降低 TLS 校验。

### 许可清楚的扩充来源

PROMAX 新增 app2smile 的起点、贴吧和腾讯新闻原生 Surge 去广告模块，以及 AdGuard 的 CNAME 伪装追踪、邮件追踪域名源。源模块、配套规则和四个官方脚本依赖随同一更新链检查，脚本仍从作者仓库加载；贴吧不启用隐藏普通视频帖的选项。去重后的伪装追踪域名归入 Privacy_CNAME，App 配套规则归入 Local，脚本/MITM 仍由功能层统一收束。广告/追踪域名不需要额外 HTTPS 解密；不启用会影响正常链接与页面的 CNAME clickthrough/microsites 全集。

测试反馈中的精确广告与统计端点由 `rulesets/Sources/Privacy_Reviewed.list` 补充，并记录核对来源。补充只作用于已审查的完整域名，不扩大为整站、共享 CDN 或 Apple 父域拦截；明确的单主机白名单与 Apple 关键服务保护仍优先。网络规则不能替代浏览器的页面元素隐藏，也不以测试分数为由拦截用途不明的共享服务。

IPCheck 的“未拦截”不能直接当成应封禁清单：当前精确补充包含 Firebase、X、Reddit、TikTok、Amazon 广告以及 Apple 广告/天气分析端点；`static.ads-twitter.com` 的域名拒绝也覆盖其 `/uwt.js` 请求。`metrics.icloud.com` 继续遵循 Apple 网络要求保留；用途不够明确的 `ngfts.lge.com` 不因测试得分而整域封禁。Pinterest 的 `s.pinimg.com/ct/core.js` 是跟踪脚本，但不能封禁整个共享图片 CDN，也不为该测试额外解密图片主机；路径拦截和页面元素隐藏应由浏览器内容拦截器承担。

Pinterest 已确认用于普通图片的 `i.pinimg.com`、网页必要代码的 `s.pinimg.com` 与嵌入 Pin／画板内容接口 `widgets.pinterest.com` 按精确主机保护；后者也承载计数接口，这是共享内容接口的功能取舍。会覆盖这些主机的整段父域拒绝也会被隔离，否则普通内容仍会被父域规则拦截；未保护的兄弟主机及独立 `log`／`ct`／`ads` 跟踪端点仍可拦截，不注入整站放行或分流动作。分流分类与广告拒绝是两条独立链路：页面与图片可能命中不同用户策略，网站卡顿不能仅从规则条数判断，也不能把一次网络失败当成已测量的性能改善。当前未确认 `s-passets.pinimg.com` 的整域拒绝与页面卡顿之间的因果关系，修改该候选前需要对应请求地址与命中记录。

若最终分片已经包含精确拒绝但客户端仍显示放行，应刷新 PROMAX **及其外部规则资源**，并核对客户端请求记录中的实际命中规则；旧缓存、未启用模块或先命中的用户规则不能靠重复添加相同条目解决。本仓库不会通过模块里的放行/分流规则改写使用者的策略组或规则顺序。

Surge 和 Shadowrocket 的现有 PROMAX 入口自动引用全部活动分片，无需新增订阅。Loon 产物、转换器及其目录条目已移除，不再为旧入口保留空文件；规则覆盖继续完整保留在活动分片中。

许可正文、修改范围及未接入 luestr 受限资源的原因见 [来源与许可说明](rulesets/Sources/licenses/README.md)。本次不复制 Loon 专属或要求客户端身份验证的内容、不伪装 User-Agent，既有高德功能保留。配置仍只安装现有 PROMAX 入口，不需要另装一组重复模块。

## 游戏规则分流

`rulesets/RULE-SET/` 提供三个可独立选择策略的规则集，其 `github/` 镜像和 `rulesets/SingBox/` 下的 SRS 在同一 Rust 更新中生成：

| 文件 | 用途 |
| --- | --- |
| `Gaming_Download.list` | 明确的游戏下载、安装包、补丁及更新 CDN（含 Steam 中国下载节点） |
| `Gaming_Play.list` | 明确的游戏网络、匹配发现、好友/在线服务及对局相关端点 |
| `Gaming_Web.list` | 官网、商店、账号、社区，以及无法按域名拆分的共享平台兜底 |

三组是独立分类，使用哪些策略、怎样排列以及如何兜底，均由使用者的客户端配置决定；仓库不生成默认路由或策略组。

`Gaming_Web` 的平台父域名可包含 Download/Play 的子域名，故顺序是分流语义的一部分。域名无法区分同一主机上的网页、登录和对局；这类共享域名保留在平台兜底，不宣称是纯联机流量。专用联机组也不覆盖任意 IP 的 P2P 对局：如需此类覆盖，在用户配置中按具体游戏进程/设备追加规则，不全局匹配 UDP 或 443。

通用 Microsoft 登录域名 `login.live.com`、`logincdn.msauth.net` 不进入专用游戏组，以免游戏策略覆盖其他应用的账号登录；由用户的 Microsoft/普通分流规则处理。旧 `Gaming_ip.list` 也不自动并入专用联机组，其共享云 IP 无法仅凭地址确定是游戏流量。

`Gaming` 全集仅作为内部分类输入，不再公开发布。游戏下载组吸收 Sukka 官方 game-download 规则和本地精确端点；联机分类端点维护于 `rulesets/Sources/custom/Gaming_Play_manual.list`。上游的共享平台 IP 和启动器进程保留在 `Gaming_Web_ip` / `Gaming_Web_extra`，不冒充纯对局流量，默认示例不启用这些宽泛兜底。更新时由 Rust 合并上游与手动分类，再生成镜像和 SRS；专用对局组不含启动器整进程、公司域名关键词或写死的策略。

## Rust 更新入口

### CI 与网络调度

自动更新仍为每天三次，更新任务串行发布，不因省时而降低上游检查频率。Guard 只浅拉当前提交和比较所需的基准提交；文档等无关改动保留检查结果，但不启动 Rust 构建。相关改动仍运行格式、严格 Clippy、测试、生成树检查与模块校验；Windows/macOS GUI 构建、测试和无界面 worker 校验保持不变。新提交可取消过时的只读检查，不能中断正在发布的更新任务。

GitHub 对公开仓库使用标准托管 runner 的运行分钟免费；账户通知中的私有仓库免费分钟额度与本仓库的公开标准 runner 不应混算。若额度告警，先在账户用量明细确认仓库和 SKU，不通过关闭质量检查、降低上游更新频率或更改账单预算来掩盖问题。缓存、制品存储及大型 runner 有独立计费边界，见 [GitHub Actions 官方计费说明](https://docs.github.com/en/billing/concepts/product-billing/github-actions)。

三套工作流显式统一 `CARGO_TARGET_DIR` 与 Rust 缓存目录，避免从仓库根目录使用 `--manifest-path` 时忽略子目录 Cargo 配置、写入另一份构建目录。缓存按平台、工具链和依赖区分，仅 master 保存成功运行的缓存；PR 只恢复。所有编译与运行使用锁定依赖。sing-box 下载不再嵌套多层重试，主站和镜像各有次数/时间上限，缓存命中和新下载都必须校验 SHA-256，HTTPS 重定向不得降级。

Rust 下载默认并发为 4、硬上限为 6；GitHub 资源族保留共享的 125 ms 请求间隔。不同阶段/下载器共享进程内冷却状态：403/429，以及带 `Retry-After` 的 503 会暂停同源后续请求；其他来源可继续工作。冷却时间超出批次剩余时间时直接报告失败，不空等到截止点，也不通过新建下载器或切换 GitHub 镜像绕过冷却。默认重试次数、请求/批次截止时间和体积预算继续生效。

HTTPS 来源的重定向必须保持 HTTPS，证书验证不关闭；含用户凭据的下载 URL 被拒绝，查询/片段 URL 不转送其他镜像。下载诊断隐藏 URL 中的用户凭据、查询参数和片段，但保留来源路径及 HTTP 状态用于排错。此调度只协调当前进程，不控制其他程序或同一出口上的流量，也不能保证不触发服务方风控。没有引入跨运行的源内容缓存，不会把未经检查的旧源当成本轮成功下载。

HaGeZi 的 TIF Medium 与 Ultimate 两条官方 GitLab 镜像不可用时，可在原有重试／截止时间内回退到同作者的官方 GitHub 原始清单；只匹配这两个完整 HTTPS 路径，不推广到任意 GitLab 仓库，内容与滚动清单完整性检查仍然执行。两处均失败时停止发布，不跳过来源。

### Rust GUI（Windows / macOS）

原生 Rust 桌面界面面向 Windows 与 macOS；Linux 继续支持命令行处理器，但不构建此 GUI。桌面界面与命令行共用更新和校验实现，不需要 Node.js、Python 或另外安装更新器。界面包含概览、模块/规则/来源目录、任务、仓库、设置和帮助页面：可搜索模块与规则、复制订阅地址、管理 Wool / Weibo / Devtools 自定义来源，并查看任务和仓库状态。目录浏览和仓库状态读取不自动执行网络操作。

```bash
cargo run --release --locked --manifest-path create/processor/Cargo.toml \
  --features gui --bin script-hub-gui -- --root .
```

GUI 需要 Rust 1.95+；`gui` 为可选功能，普通规则更新 CI 不编译图形依赖。[Rust GUI 工作流](https://github.com/nowaytouse/script_hub/actions/workflows/gui.yml)在 Windows 与 macOS 构建。下载时请按制品名称中的操作系统和实际 runner 架构选择；Windows 提供含 `.exe` 与合规说明的 zip，macOS 提供含说明文件的 `.app` tar.gz 包。选择仓库目录作为工作区；显式传入的 CLI `--root` 优先于已保存的工作区路径。

许可与数据处理说明见 [LICENSE](LICENSE)、[隐私说明](PRIVACY.md)、[安全报告说明](SECURITY.md) 和 [第三方内容声明](THIRD_PARTY_NOTICES.md)。桌面制品包含这些说明及当前已登记的上游 MIT 许可文本；第三方来源和依赖的完整许可清单尚未完成，不能据此视为完整合规审计。GPLv3 许可只声明适用于项目作者有权许可的项目自有代码，不改变嵌入或引用的第三方内容所适用的许可。

macOS 包会进行本地 ad-hoc 签名和完整性校验，但开发构建尚无受信任开发者签名或公证，不声称通过平台商店或系统发行认证。GUI 写入任务之间由操作系统文件锁互斥，锁由实际执行更新的子进程持有，退出后自动释放；不要同时从外部命令行运行另一套更新。

浏览与只读校验不联网；联网与写入操作要求确认，每次只运行一个后台任务。支持只读校验、目录刷新、全量/快速更新和游戏规则更新。进度展示来自真实阶段和经过时间；不具备可信测量依据时不显示估算百分比。界面显示真实退出结果、警告和有界日志。联网更新沿用 `https_proxy` / `http_proxy` / `all_proxy` 环境变量，不更改系统代理、不自动提交或推送。

仓库页提供人工触发的 fast-forward-only pull、按明确选择文件的 commit 和非强制 push；不会自动执行 Git 操作。自定义来源保存在工作区 `modules/source/custom_sources.json`，添加和移除均需确认，并在下一次全量更新中重新合并对应合集；快速更新跳过功能合集。未指定 sing-box 时保留现有 SRS，不声称已更新 SRS。

偏好设置保存在 macOS `~/Library/Application Support/Script Hub/settings.json` 或 Windows `%APPDATA%\ScriptHub\settings.json`。设置模型支持主题、强调色、系统/简体中文/英文/日文、80–150% 界面缩放、窗口尺寸和紧凑度；保存的工作区路径不会覆盖显式 CLI `--root`。

默认启动只读取应用自身设置，不自动扫描工作区或探测父目录。可手动加载、拖入项目文件夹，或明确开启“启动时重新打开已保存的工作区”；CLI `--root` 也表示明确打开该路径。目录加载在单个后台线程执行，等待权限时仍可操作界面和关闭窗口，不会重复启动读取任务。

macOS 的桌面、文稿、下载、网络卷和可移动卷受系统权限控制。当前 ad-hoc 开发签名的权限身份与具体构建绑定，更换构建后可能需要重新授权；项目不能绕过系统同意，也不要求完全磁盘访问。无人值守建议使用明确指定、位于上述受保护目录之外的本地仓库，并从固定位置运行同一构建；跨版本稳定授权的发行版需要开发者提供一致的 Developer ID 签名身份。此限制与“记住路径”不同，不能靠保存路径消除。

桌面 GUI 不等于 iOS / Android 原生应用。网页安装助手已移除；在 Windows / macOS GUI 的资源库勾选项目，批量复制 CDN / GitHub 链接后粘贴到客户端。规则片段需要先填写自己的策略或策略组名称，程序不代选路由。DNS/Host 助手模块仍按原有模块入口提供。

资源库的“选择筛选结果”只追加当前搜索/分类命中的项目，跳过已合并的模块，保留此前隐藏的勾选；“已选”统计全部勾选，“清除选择”则清空全部选择。每个复选框在辅助功能中关联具体项目名称，筛选不会把行焦点复用给其他项目。

已知验收缺口：本地 macOS 开发构建在资源库标签切换后出现过中央区域辅助功能节点缺失，而画面和常规鼠标操作仍正常。无界面测试已检查原始辅助功能树的根节点可达性，但不等于 VoiceOver 或 macOS 适配层验收；该现象尚未确认修复，不宣称完整无障碍支持。

### 命令行

```bash
cargo run --release --manifest-path create/processor/Cargo.toml --bin update -- \
  --root . --execute --singbox /path/to/sing-box

cargo run --manifest-path create/processor/Cargo.toml --bin validate -- --root .
```

`--quick` 仅跳过可选的功能模块上游刷新，PROMAX 规则仍会完整抓取、编译和验证。生成器会保留仅日期、注释、排版或顺序变化的旧文件，避免没有实质更新时产生自动提交。

下载器拒绝空响应和以 HTML 文档标记开头的错误页；GitHub 限流遵守 `Retry-After` 与已耗尽额度的重置时间，没有明确等待时间时从 60 秒开始退避，并受批次时限约束。通用规则集解析后若没有有效规则会报错并保留旧产物；若某一类型没有有效规则，则移除该类型的旧产物，不生成零规则占位。

功能合集先在缓存中完成合并、安全处理与校验，再成组发布；YouTube 功能和广告源一起提交，后处理或任一输出失败时保留旧版。规则清理、镜像同步和 Shadowrocket 遍历的 I/O 失败会传到主更新，不再记作成功；去重至零的列表及对应旧 SRS 会移除，不冒充有覆盖的有效产物。下载遇到 404 时只在既有次数预算内切换到未尝试的备选入口，不重复请求已确认 404 的地址；发布时将显式 `refs/heads`、`refs/tags` 引用规范化，避免产生 `@refs/...` CDN 链接。

`--gaming-only` 用当前的 `Gaming.list`、游戏下载源结果与本地分类端点重新生成三类游戏规则及镜像，不下载上游源。带 `--singbox` 时同时同步 SRS；适合轻量验证游戏分类改动。

规则源在 `rulesets/Sources/Links/AdBlock_sources.list` 中以 `SOURCE|POLICY|KIND|CATEGORY` 明确登记；脚本/重写源在 `AdBlock_functional_sources.list` 中维护。项目自有更新流程不再使用 Go、Python 或 Shell 脚本。
