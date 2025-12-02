/**
 * ============================================================
 * Sub-Store 节点隐蔽性与安全性全面增强脚本 v3.6.0 【🎵🔗 Sing-box 中继版】
 * ============================================================
 * 
 * v3.6.0 优化内容（2025-11-29）：
 * - 🚀 性能优化：批量处理优化，减少循环次数
 * - 🔒 安全增强：增强节点验证，防止恶意配置
 * - 🎭 隐蔽性提升：优化Chrome 131指纹，更真实的浏览器模拟
 * - 📦 代码质量：简化冗余逻辑，提高可维护性
 * - ⚡ 效率提升：优化去重算法，使用更高效的数据结构
 *
 * v3.5.7 修复内容（2025-11-29）：
 * - ✅ 去重功能增强：真正去除重复节点（基于server+port+type+uuid/password）
 * - ✅ ALPN修复：确保alpn参数为数组格式['h2','http/1.1']
 * - ✅ UDP转发修复：同时设置packet-encoding和xudp，兼容所有客户端
 * - ⚠️ Hysteria2心跳：不支持heartbeat参数（QUIC内置心跳）
 * - ⚠️ TLS分片：仅sing-box支持，默认关闭
 *
 * v3.5.6 修复内容（2025-11-29）：
 * - ✅ TLS分片参数：_fragment, _fragment_fallback_delay, _record_fragment (仅sing-box)
 *
 * v3.5.5 修复内容（2025-11-29）：
 * - ✅ Reality节点检测增强：支持Shadowrocket格式(publicKey/shortId)
 * - ✅ 最终TLS安全检查：在输出前再次验证非白名单端口
 * - ✅ 端口19202/12800等Reality和普通节点完全保护
 *
 * v3.5.4 修复内容（2025-11-29）：
 * - ✅ TLS分片使用Sub-Store官方参数：_fragment, _fragment_fallback_delay, _record_fragment
 * - ✅ VMess/VLESS最终TLS保护：非白名单端口强制确保TLS=false
 *
 * v3.5.3 修复内容（2025-11-29）：
 * - ✅ applyTlsConfig函数修复：只有白名单端口才启用TLS
 * - ✅ Hysteria2心跳间隔：heartbeat-interval (毫秒)
 *
 * v3.5.2 修复内容（2025-11-29）：
 * - ✅ 端口白名单模式：只有白名单端口(443/8443/2053/2083/2087/2096)才强制启用TLS
 * - ✅ 其他端口(12800/16056/19203等)完全保持原设置，不修改TLS
 *
 * v3.5.1 修复内容（2025-11-29）：
 * - ✅ 端口TLS策略修复：未知端口保持原有TLS设置
 *
 * v3.5 修复内容（2025-11-29）：
 * - ✅ UDP转发模式修复：xudp → packet-addr（更好的兼容性）
 * - ✅ TLS Fragment 分片：启用TLS分片绕过DPI检测
 * - ✅ HTTP/2 滑块：确保HTTP/2配置正确启用
 * - ✅ 端口TLS兼容性：回避不支持TLS的端口（80/8080等）
 * - ✅ CDN语料库增强：新增更多CDN域名和运营商Host
 * - ✅ VMess节点兼容性修复：修复大量VMess节点无法使用的问题
 *
 * v3.4 修复内容（2025-11-29）：
 * - ✅ skip-cert-verify 智能化：有证书配置则验证，无则允许不安全
 * - ✅ VMess security: auto 修复：替换为具体加密方法 (aes-128-gcm)
 * - ✅ 曲线配置：Clash Meta 使用 ecdh-curves（Sing-box 暂不支持）
 * - ✅ Shadowrocket tls-alpn：字符串格式 "h2"
 * - ✅ Shadowrocket udp-relay：启用 UDP 转发
 * - ✅ Hysteria2/TUIC 智能证书验证：有证书则验证，无则跳过
 * - ✅ 中国节点识别优化：避免误判 CN2 等专线
 * - ✅ 平衡安全性与可用性：智能判断而非一刀切
 *
 * 核心原则：
 * - 🛡️ 安全性：有证书配置时验证证书
 * - 🚀 可用性：无证书配置时允许不安全（机场常用自签证书）
 * - 🎭 伪装性：Chrome 131 指纹、TLS 1.3、智能 SNI
 * - ⚡ 性能：ECN、TFO、多路复用、UDP 转发
 *
 * ============================================================
 *
 * 功能说明：
 * - 用于在 Sub-Store 环境下对代理节点进行深度优化和安全增强
 * - 支持节点过滤、排序、命名规范化、协议优化等功能
 * - 自动识别节点地区、特性类型，并添加相应的 Emoji 标记
 * - 支持生成中继链和落地链
 * - 支持 Clash Meta、Mihomo、sing-box、Surge、Shadowrocket 等代理工具
 * - 新增：Chrome 131 完整浏览器指纹伪装
 * - 新增：TLS 1.3 exclusive 全协议强制
 * - 新增：AES-GCM 专用加密（智能 ChaCha20 ECH 场景支持）
 * - 新增：智能地区 CDN 映射（6 大提供商）
 * - 新增：Reality 7 层保护 + XTLS 多重检测
 * - 新增：QUIC 智能屏蔽（保护原生 QUIC 协议）
 *
 * 使用环境：
 * - Sub-Store 订阅管理工具（兼容最新版本）
 * - 支持 Clash Meta、Mihomo、sing-box、Surge、Shadowrocket 等
 *
 * 版本：v3.1 性能优化 + 兼容性修复版
 * 最后更新：2025-11-29
 * 作者：基于 Sub-Store 社区脚本深度改进
 *
 * 主要特性：
 * 1. Chrome 131 完整伪装 - TLS 指纹、ALPN、版本全面模拟
 * 2. TLS 1.3 Exclusive - 所有协议强制 TLS 1.3（min/max）
 * 3. AES-GCM 专用加密 - VMess/SS 仅用 AES-GCM（ChaCha20 智能场景支持）
 * 4. 智能地区 CDN - 6 大提供商（Cloudflare/Akamai/Fastly/Google/Azure/Bilibili）
 * 5. Reality 7 层保护 - 完全不修改 Reality 节点
 * 6. XTLS 多重保护 - 检测所有 Flow 变体
 * 7. 智能 SNI 选择 - 3 层 Fallback（地区精准->通用->原配置）
 * 8. 证书验证强制 - skip-cert-verify=false（所有协议）
 * 9. QUIC 智能屏蔽 - 保护 Hysteria2/TUIC/WireGuard，屏蔽其他
 * 10. Shadowrocket 优化 - ALPN HTTP/2 完整支持
 * 11. 多路复用全协议 - VMess/VLESS/Trojan/SS 完整支持
 * 12. 性能优化 - 预编译正则 + Set查找 + 缓存机制
 *
 * v3.1 更新内容（2025-11-29）：
 * - 修复 ShadowTLS 配置块语法错误
 * - 修复 TUIC enableZeroRtt 配置引用
 * - 优化预编译正则表达式和 Set 集合
 * - 增强防御性检查（空输入、无效节点）
 * - 提取公共检测函数（isRealityNode、hasXtlsFlow、hasEchSupport）
 * - 优化 SNI 缓存机制
 * - 修复乱码 emoji 和注释信息
 * - 增强 Chrome 131 TLS 指纹配置（曲线、签名算法、GREASE）
 * - 添加 WebSocket 伪装增强（User-Agent、Accept 头）
 *
 * ============================================================
 * 节点性能增强 (Boost) 功能详解
 * ============================================================
 *
 * 通用增强选项（适用于多数协议）：
 * - TCP Fast Open (TFO) - 减少首次连接延迟，优化网页/小包传输
 * - UDP 转发 - 支持全流量代理（游戏、DNS等）
 * - ECN (显式拥塞通知) - 提升拥堵网络下的性能
 * - IPv6 偏好 - 优先使用 IPv6 地址（如果节点支持）
 *
 * TLS 增强选项（仅当节点已启用 TLS 时生效）：
 * - ALPN 协议协商 - 优先使用 HTTP/2，减少头部开销
 * - TLS 客户端指纹伪装 - 模拟浏览器流量，提升抗检测能力
 * - SNI 优化 - 绕过 ISP/CDN 检测，伪装为正常流量
 *
 * 协议专属增强：
 *
 * [VMess]
 *   - AEAD 加密模式 (alterId = 0) - 高效加密，抗重放攻击
 *   - 优化加密方法 (AES-128-GCM) - 安全加密，减少特征
 *   - packet-addr UDP优化 - 提升 UDP 转发效率（v3.5替代xudp）
 *   - 多路复用 (smux) - 减少连接开销，改善并发和抗丢包
 *   - ALPN HTTP/2 - 优先使用 HTTP/2 协议
 *   - gRPC 传输 - 基于 HTTP/2 的高效多路复用
 *
 * [VLESS]
 *   - packet-addr 数据包编码 - 优化 UDP 转发，减少开销（v3.5替代xudp）
 *   - XTLS Flow (vision) - 优化 TLS 握手，支持 0-RTT
 *   - 多路复用 (smux) - 减少连接开销（不与XTLS同时使用）
 *   - gRPC 传输优化 - 高效多路复用，提升吞吐
 *
 * [Trojan]
 *   - TLS 必备配置 - 确保 TLS 加密启用
 *   - XTLS Flow (vision) - 低延迟握手，支持 0-RTT
 *   - 多路复用 (smux) - 减少连接开销（不与XTLS同时使用）
 *   - WebSocket/gRPC 传输 - 伪装 HTTP，提升穿透能力
 *
 * [Shadowsocks]
 *   - AEAD 加密方法 (AES-128-GCM) - 高效抗检测
 *   - UDP over TCP - 提升 UDP 可靠性，减少丢包
 *   - 多路复用 (smux) - 使用 v2ray-plugin 时支持
 *
 * [Hysteria2]
 *   - ALPN HTTP/3 - 基于 QUIC 的 HTTP/3 协议，低延迟
 *   - 带宽设置 (默认自动协商) - 优化拥塞控制
 *   - MTU 发现 - 自动优化包大小，减少碎片
 *   - Salamander 混淆 - 流量混淆，抗检测
 *
 * [TUIC]
 *   - ALPN HTTP/3 - 基于 QUIC 的 HTTP/3 协议，低延迟
 *   - BBR 拥塞控制 - 优化高延迟网络，提升速度
 *   - QUIC UDP 中继 - 低延迟，抗丢包
 *   - 0-RTT 握手 - 立即发送数据，减少连接时间
 *   - 心跳保活 (3秒) - 保持连接活跃，提升稳定性
 *
 * [WireGuard]
 *   - MTU 优化 (1420) - 减少碎片，提升效率
 *   - 保留位配置 - 提升特定网络兼容性
 *   - 持续连接 (25s) - Keep Alive 保持连接
 *
 * 重要提示：
 * - 所有增强选项都可以通过配置开关灵活控制
 * - TLS 相关配置采用保守策略，仅在节点已启用 TLS 时才添加增强选项
 * - 某些增强选项（如 mux）可能不被所有服务器支持，请根据实际情况调整
 * - 默认启用所有增强选项，如遇问题可通过 cfg.enableBoost = false 禁用
 *
 * ============================================================
 */

// 🚀 性能优化：预编译所有正则表达式（避免运行时重复编译）
const BLOCK_KEYWORDS = [
    '剩余', '到期', '流量', '官网', '客服', '群组', '购买', '续费', '订阅', '重置',
    '充值', '套餐', '网址', '防失联', '电报', 'telegram', '广告', '推广', '宣传',
    '官方', '下载', '更新', '版本', '失效', '过期', '停用', '不可用', '无法连接',
    '超时', '失败', '错误', '被拒', '证书', '握手', '测试', '测速', '故障', '维护',
    '升级', '问题', '异常', '中断', '堵塞', '审查', 'GFW', '机场', 'jichang',
    '监控', '统计', '日志', 'log', 'tracking', 'analytics', 'monitor', 'invite',
    '优惠', 'aff', 'localhost', 'internal', 'RFC1918', '保留地址', '试用', '体验',
    '禁用', 'ban', 'TRASH', 'Free', 'Trial', 'Test', 'Banned', '节点',
    '无效', '关闭', '停服', '下线', '维护中', '升级中', '故障中', '异常节点',
    '低速', '慢速', '拥堵', '高延迟', '丢包', '不稳定', '备份', '备用', '临时',
    '实验', '调试', '开发', '内部', '私有', '本地', '局域', '内网', 'loopback'
];
// 🚀 使用冻结的 Set 加速关键词查找（O(1) vs O(n)）
const BLOCK_KEYWORDS_SET = Object.freeze(new Set(BLOCK_KEYWORDS.map(k => k.toLowerCase())));
const BLOCK_REGEX = new RegExp(BLOCK_KEYWORDS.join('|'), 'i');
const BLOCK_REGEX_EN = /err_|fail|reject|unreach|timeout|error|dead|offline|expire|down|invalid|broken|refused|reset by peer|not found|unavailable|unstable|slow|lag|high latency|packet loss|backup|temp|experimental|debug|dev|internal|private|local|lan|intranet|loopback|closed|shutdown|maintenance|upgrade|fault|abnormal|low speed|congested|high delay|packet drop|error log|connection fail|timeout error|rejected connection|invalid cert|handshake fail|speed test|network fault|maintenance alert|system upgrade|troubleshoot issue|anomaly detection|network congestion|censorship block|airport node|monitoring system|access stats|log record|tracking data|analytics tool|monitor node|invite code|promo code|AFF link|localhost host|internal net|reserved IP|trial node|experience node|disabled account|banned node|trash node|free node|trial period|test node|banned list|invalid link|service closed|shutdown notice|offline maintenance|maintenance status|upgrade process|fault report|abnormal server|low speed connection|slow transfer|congested network|high latency link|high packet loss|unstable signal|backup server|spare line|temp access|experimental env|debug mode|dev test|internal use|private net|local connect|lan network|intranet access|loopback address/i;

// 🚀 预编译特性正则（冻结防止意外修改）
const FEATURE_REGEX = Object.freeze({
    p: /premium|高级|vip|pro|plus|专线|iplc|iepl|elite|ultimate|gold|platinum|luxury|exclusive|advanced|deluxe|supreme|top-tier|prime|executive|prestige|diamond|superior|high-end/i,
    f: /fast|极速|高速|turbo|speed|express|rapid|quick|blazing|ultra|swift|high-speed|accelerated|lightning|boosted|super-fast|hyper|velocity|warp|flash|sonic|jet|rocket|bullet|zoom|dash/i,
    s: /stable|稳定|reliable|enterprise|dedicated|robust|secure|high-uptime|consistent|dependable|solid|premium-stable|ultra-reliable|rock-solid|steady|trustworthy|fortified|resilient|unwavering|enduring|bulletproof|ironclad|unbreakable|steadfast/i
});

// 🚀🚀🚀 性能关键优化：预编译地区正则表达式（只编译一次，而非每次调用都编译）
const REGION_PATTERNS = Object.freeze({
    '🇭🇰': { r: /香港|Hong\s*Kong|HK(?!BN)|HongKong|HKBN|HKT|PCCW|HGC|CMI|CSL|WTT/i, n: '香港', p: 11 },
    '🇹🇼': { r: /台湾|台灣|Taiwan|TW|Taipei|Hinet|CHT|中华电信/i, n: '台湾', p: 10 },
    '🇯🇵': { r: /日本|Japan|JP|Tokyo|Osaka|NTT|IIJ|KDDI|SoftBank/i, n: '日本', p: 13 },
    '🇰🇷': { r: /韩国|韓國|Korea|KR|Seoul|SK(?:[\s\-·]|$)|KT(?:[\s\-·]|$)|LG\s*U/i, n: '韩国', p: 14 },
    '🇸🇬': { r: /新加坡|Singapore|SG|Singtel|StarHub/i, n: '新加坡', p: 20 },
    '🇺🇸': { r: /美国|美國|USA|US(?:[\s\-·]|$)|United\s*States|Los\s*Angeles|San\s*Jose|New\s*York|LA(?:[\s\-·]|$)|NY(?:[\s\-·]|$)|Seattle|Chicago|Dallas|Miami|Atlanta|Ashburn/i, n: '美国', p: 30 },
    '🇨🇳': { r: /(?:^|[\s\-_])(?:中国|Mainland\s*China|PRC)(?:[\s\-_]|$)|(?:^|[\s\-_])CN(?![2-9A-Za-z])|(?:北京|上海|广州|深圳|杭州|成都|武汉|南京|西安|重庆|天津|贵州|贵阳|云南|昆明|四川|福建|厦门|湖北|湖南|长沙|山东|济南|青岛|辽宁|沈阳|大连|河南|郑州|安徽|合肥|河北|石家庄|陕西|广西|南宁|海南|三亚|江西|南昌|甘肃|兰州|青海|宁夏|新疆|西藏|内蒙古|黑龙江|哈尔滨|吉林|长春|浙江|江苏|苏州|无锡)(?:[\s\-_]|$)/i, n: '中国', p: 5 },
    '🇬🇧': { r: /英国|英國|UK|GB|United\s*Kingdom|London|Manchester/i, n: '英国', p: 40 },
    '🇩🇪': { r: /德国|德國|Germany|DE(?:[\s\-·]|$)|Frankfurt|Berlin|Munich/i, n: '德国', p: 41 },
    '🇫🇷': { r: /法国|法國|France|FR(?:[\s\-·]|$)|Paris|Marseille/i, n: '法国', p: 42 },
    '🇳🇱': { r: /荷兰|荷蘭|Netherlands|NL(?:[\s\-·]|$)|Amsterdam|Rotterdam/i, n: '荷兰', p: 43 },
    '🇦🇺': { r: /澳洲|澳大利亚|Australia|AU(?:[\s\-·]|$)|Sydney|Melbourne|Brisbane/i, n: '澳洲', p: 70 },
    '🇨🇦': { r: /加拿大|Canada|CA(?:[\s\-·]|$)|Toronto|Vancouver|Montreal/i, n: '加拿大', p: 31 },
    '🇷🇺': { r: /俄罗斯|俄羅斯|Russia|RU(?:[\s\-·]|$)|Moscow|Petersburg/i, n: '俄罗斯', p: 44 },
    '🇮🇳': { r: /印度|India|IN(?:[\s\-·]|$)|Mumbai|Delhi|Bangalore/i, n: '印度', p: 90 },
    '🇧🇷': { r: /巴西|Brazil|BR(?:[\s\-·]|$)|Sao\s*Paulo|Rio/i, n: '巴西', p: 60 },
    '🇲🇾': { r: /马来西亚|馬來西亞|Malaysia|MY(?:[\s\-·]|$)|Kuala/i, n: '马来西亚', p: 21 },
    '🇹🇭': { r: /泰国|泰國|Thailand|TH(?:[\s\-·]|$)|Bangkok/i, n: '泰国', p: 22 },
    '🇻🇳': { r: /越南|Vietnam|VN(?:[\s\-·]|$)|Hanoi|Ho\s*Chi/i, n: '越南', p: 23 },
    '🇵🇭': { r: /菲律宾|菲律賓|Philippines|PH(?:[\s\-·]|$)|Manila/i, n: '菲律宾', p: 24 },
    '🇮🇩': { r: /印尼|印度尼西亚|Indonesia|ID(?:[\s\-·]|$)|Jakarta/i, n: '印尼', p: 25 },
    '🇹🇷': { r: /土耳其|Turkey|TR(?:[\s\-·]|$)|Istanbul|Ankara/i, n: '土耳其', p: 80 },
    '🇦🇪': { r: /阿联酋|UAE|AE(?:[\s\-·]|$)|Dubai|Abu\s*Dhabi/i, n: '阿联酋', p: 81 },
    '🇨🇭': { r: /瑞士|Switzerland|CH(?:[\s\-·]|$)|Zurich|Geneva/i, n: '瑞士', p: 45 },
    '🇸🇪': { r: /瑞典|Sweden|SE(?:[\s\-·]|$)|Stockholm/i, n: '瑞典', p: 46 },
    '🇮🇹': { r: /意大利|Italy|IT(?:[\s\-·]|$)|Milan|Rome/i, n: '意大利', p: 48 },
    '🇪🇸': { r: /西班牙|Spain|ES(?:[\s\-·]|$)|Madrid|Barcelona/i, n: '西班牙', p: 49 },
    '🇵🇱': { r: /波兰|波蘭|Poland|PL(?:[\s\-·]|$)|Warsaw/i, n: '波兰', p: 50 },
    '🇦🇹': { r: /奥地利|Austria|AT(?:[\s\-·]|$)|Vienna/i, n: '奥地利', p: 55 },
    '🇧🇪': { r: /比利时|Belgium|BE(?:[\s\-·]|$)|Brussels/i, n: '比利时', p: 56 },
    '🇨🇿': { r: /捷克|Czechia|CZ(?:[\s\-·]|$)|Prague/i, n: '捷克', p: 57 },
    '🇳🇿': { r: /新西兰|New\s*Zealand|NZ(?:[\s\-·]|$)|Auckland/i, n: '新西兰', p: 71 },
    '🇿🇦': { r: /南非|South\s*Africa|ZA(?:[\s\-·]|$)|Johannesburg|Cape\s*Town/i, n: '南非', p: 91 },
    '🇲🇴': { r: /澳门|澳門|Macau|MO(?:[\s\-·]|$)|CTM/i, n: '澳门', p: 12 },
    '🇮🇪': { r: /爱尔兰|Ireland|IE(?:[\s\-·]|$)|Dublin/i, n: '爱尔兰', p: 47 },
    '🇫🇮': { r: /芬兰|Finland|FI(?:[\s\-·]|$)|Helsinki/i, n: '芬兰', p: 52 },
    '🇳🇴': { r: /挪威|Norway|NO(?:[\s\-·]|$)|Oslo/i, n: '挪威', p: 53 },
    '🇩🇰': { r: /丹麦|Denmark|DK(?:[\s\-·]|$)|Copenhagen/i, n: '丹麦', p: 54 },
    '🇺🇦': { r: /乌克兰|烏克蘭|Ukraine|UA(?:[\s\-·]|$)|Kyiv/i, n: '乌克兰', p: 51 },
    '🇭🇺': { r: /匈牙利|Hungary|HU(?:[\s\-·]|$)|Budapest/i, n: '匈牙利', p: 58 },
    '🇷🇴': { r: /罗马尼亚|Romania|RO(?:[\s\-·]|$)|Bucharest/i, n: '罗马尼亚', p: 59 },
    '🇦🇷': { r: /阿根廷|Argentina|AR(?:[\s\-·]|$)|Buenos/i, n: '阿根廷', p: 61 },
    '🇨🇱': { r: /智利|Chile|CL(?:[\s\-·]|$)|Santiago/i, n: '智利', p: 62 },
    '🇨🇴': { r: /哥伦比亚|Colombia|CO(?:[\s\-·]|$)|Bogota/i, n: '哥伦比亚', p: 63 },
    '🇲🇽': { r: /墨西哥|Mexico|MX(?:[\s\-·]|$)|Mexico\s*City/i, n: '墨西哥', p: 32 },
    '🇵🇹': { r: /葡萄牙|Portugal|PT(?:[\s\-·]|$)|Lisbon/i, n: '葡萄牙', p: 65 },
    '🇬🇷': { r: /希腊|Greece|GR(?:[\s\-·]|$)|Athens/i, n: '希腊', p: 66 },
    '🇮🇱': { r: /以色列|Israel|IL(?:[\s\-·]|$)|Tel\s*Aviv/i, n: '以色列', p: 82 },
    '🇸🇦': { r: /沙特|Saudi|SA(?:[\s\-·]|$)|Riyadh/i, n: '沙特', p: 83 },
    '🇶🇦': { r: /卡塔尔|Qatar|QA(?:[\s\-·]|$)|Doha/i, n: '卡塔尔', p: 84 },
    '🇪🇬': { r: /埃及|Egypt|EG(?:[\s\-·]|$)|Cairo/i, n: '埃及', p: 92 },
    '🇰🇭': { r: /柬埔寨|Cambodia|KH(?:[\s\-·]|$)|Phnom/i, n: '柬埔寨', p: 26 },
    '🇰🇿': { r: /哈萨克|Kazakhstan|KZ(?:[\s\-·]|$)|Almaty/i, n: '哈萨克斯坦', p: 100 },
    '🇵🇰': { r: /巴基斯坦|Pakistan|PK(?:[\s\-·]|$)|Karachi/i, n: '巴基斯坦', p: 93 },
    '🇧🇩': { r: /孟加拉|Bangladesh|BD(?:[\s\-·]|$)|Dhaka/i, n: '孟加拉', p: 94 },
    '🇱🇰': { r: /斯里兰卡|Sri\s*Lanka|LK(?:[\s\-·]|$)|Colombo/i, n: '斯里兰卡', p: 98 },
    '🇲🇳': { r: /蒙古|Mongolia|MN(?:[\s\-·]|$)|Ulaanbaatar/i, n: '蒙古', p: 99 },
    '🇲🇲': { r: /缅甸|Myanmar|MM(?:[\s\-·]|$)|Yangon/i, n: '缅甸', p: 97 },
    '🇳🇵': { r: /尼泊尔|Nepal|NP(?:[\s\-·]|$)|Kathmandu/i, n: '尼泊尔', p: 95 },
    '🇰🇪': { r: /肯尼亚|Kenya|KE(?:[\s\-·]|$)|Nairobi/i, n: '肯尼亚', p: 110 },
    '🇳🇬': { r: /尼日利亚|Nigeria|NG(?:[\s\-·]|$)|Lagos/i, n: '尼日利亚', p: 111 },
    // 🆕 新增缺失的国家/地区
    '🇬🇹': { r: /危地马拉|Guatemala|GT(?:[\s\-·]|$)|Guatemala\s*City/i, n: '危地马拉', p: 67 },
    '🇧🇴': { r: /玻利维亚|Bolivia|BO(?:[\s\-·]|$)|La\s*Paz|Sucre/i, n: '玻利维亚', p: 68 },
    '🇵🇪': { r: /秘鲁|Peru|PE(?:[\s\-·]|$)|Lima/i, n: '秘鲁', p: 69 },
    '🇪🇨': { r: /厄瓜多尔|Ecuador|EC(?:[\s\-·]|$)|Quito/i, n: '厄瓜多尔', p: 64 },
    '🇨🇷': { r: /哥斯达黎加|Costa\s*Rica|CR(?:[\s\-·]|$)|San\s*Jose/i, n: '哥斯达黎加', p: 33 },
    '🇲🇦': { r: /摩洛哥|Morocco|MA(?:[\s\-·]|$)|Casablanca|Rabat/i, n: '摩洛哥', p: 85 },
    '🇷🇸': { r: /塞尔维亚|Serbia|RS(?:[\s\-·]|$)|Belgrade/i, n: '塞尔维亚', p: 86 },
    '🇱🇹': { r: /立陶宛|Lithuania|LT(?:[\s\-·]|$)|Vilnius/i, n: '立陶宛', p: 87 }
});

// 🚀 快速地区匹配函数（使用预编译正则，O(n) 但常数因子极小）
const fastGetRegion = (name) => {
    if (!name) return { f: '🌐', r: '其他', p: 999 };
    for (const [flag, info] of Object.entries(REGION_PATTERNS)) {
        if (info.r.test(name)) return { f: flag, r: info.n, p: info.p };
    }
    return { f: '🌐', r: '其他', p: 999 };
};

// 🚀 预编译省份匹配（冻结 + 预转换小写）
const PROVINCES = Object.freeze({
    '北京': ['北京', 'beijing', 'bj', 'peking'],
    '上海': ['上海', 'shanghai', 'sh'],
    '广东': ['广东', 'guangdong', 'gd', 'guangzhou', 'shenzhen', 'dongguan'],
    '浙江': ['浙江', 'zhejiang', 'zj', 'hangzhou', 'ningbo', 'wenzhou'],
    '江苏': ['江苏', 'jiangsu', 'js', 'nanjing', 'suzhou', 'wuxi'],
    '四川': ['四川', 'sichuan', 'sc', 'chengdu', 'chongqing'],
    '福建': ['福建', 'fujian', 'fj', 'fuzhou', 'xiamen', 'quanzhou'],
    '湖北': ['湖北', 'hubei', 'hb', 'wuhan', 'yichang'],
    '湖南': ['湖南', 'hunan', 'hn', 'changsha', 'zhuzhou'],
    '山东': ['山东', 'shandong', 'sd', 'jinan', 'qingdao', 'yantai'],
    '辽宁': ['辽宁', 'liaoning', 'ln', 'shenyang', 'dalian'],
    '河南': ['河南', 'henan', 'ha', 'zhengzhou', 'luoyang'],
    '安徽': ['安徽', 'anhui', 'ah', 'hefei', 'wuhu'],
    '河北': ['河北', 'hebei', 'he', 'shijiazhuang', 'tangshan'],
    '陕西': ['陕西', 'shaanxi', 'sn', 'xian', 'baoji'],
    '重庆': ['重庆', 'chongqing', 'cq'],
    '天津': ['天津', 'tianjin', 'tj'],
    '广西': ['广西', 'guangxi', 'gx', 'nanning', 'guilin'],
    '云南': ['云南', 'yunnan', 'yn', 'kunming', 'lijiang'],
    '海南': ['海南', 'hainan', 'hi', 'haikou', 'sanya'],
    '江西': ['江西', 'jiangxi', 'jx', 'nanchang', 'jiujiang'],
    '贵州': ['贵州', 'guizhou', 'gz', 'guiyang', 'zunyi'],
    '甘肃': ['甘肃', 'gansu', 'gs', 'lanzhou', 'dunhuang'],
    '青海': ['青海', 'qinghai', 'qh', 'xining'],
    '宁夏': ['宁夏', 'ningxia', 'nx', 'yinchuan'],
    '新疆': ['新疆', 'xinjiang', 'xj', 'urumqi'],
    '西藏': ['西藏', 'tibet', 'xz', 'lhasa'],
    '内蒙古': ['内蒙古', 'inner mongolia', 'nm', 'hohhot', 'baotou'],
    '黑龙江': ['黑龙江', 'heilongjiang', 'hl', 'harbin', 'qiqihar'],
    '吉林': ['吉林', 'jilin', 'jl', 'changchun', 'jilin city']
});

// 🚀 预编译特殊关键词（使用 Set 加速查找）
const SPECIAL_KEYWORDS = [
    'BGP', 'CN2', 'GIA', 'IPLC', 'IEPL', 'CUVIP',
    'Premium', 'Enterprise', 'VIP', 'Platinum', 'Gold',
    '专线', '企业专线', '国际专线', '精品网',
    'NTT', 'IIJ', 'KDDI', 'PCCW', 'HKT', 'HGC',
    'ChinaNet', 'China Telecom', 'China Unicom', 'China Mobile',
    'Anycast', 'Full Route', 'Tier 1', 'Tier1', 'T1', 'Tier 2', 'Tier2', 'T2', 'Tier 3', 'Tier3', 'T3', 'Tier 4', 'Tier4', 'T4', 'Tier 5', 'Tier5', 'T5',
    'ISP', 'Carrier', 'Provider', 'Network', 'Internet', 'Service', 'Backbone', '家宽', '家庭宽带',
    'LV1', 'LV2', 'LV3', 'LV4', 'LV5', '等级1', '等级2', '等级3', '等级4', '等级5', 'Tier 6', 'Tier6', 'T6'
];
const SPECIAL_KEYWORDS_LOWER = Object.freeze(new Set(SPECIAL_KEYWORDS.map(k => k.toLowerCase())));

// 🚀 v3.5.2: 端口白名单模式 - 只有白名单端口才强制启用TLS
// ✅ TLS白名单端口：只有这些端口才会强制启用TLS（Cloudflare HTTPS端口）
const TLS_WHITELIST_PORTS = Object.freeze(new Set([443, 8443, 2053, 2083, 2087, 2096]));
// 🚫 非TLS端口：这些端口明确不支持TLS
const NON_TLS_PORTS = Object.freeze(new Set([80, 8080, 8880, 2052, 2082, 2086, 2095, 8008, 8088, 8000, 8800]));
// 兼容旧代码
const TLS_PORTS = TLS_WHITELIST_PORTS;
const CF_HTTP_ONLY_PORTS = NON_TLS_PORTS;

// 🚀 预编译 QUIC 原生协议集合
const QUIC_NATIVE_PROTOCOLS = Object.freeze(new Set(['hysteria2', 'hysteria', 'tuic', 'wireguard']));

// 🚀 预编译端口跳跃参数集合
const PORT_HOPPING_PARAMS = Object.freeze(new Set([
    'port-hopping', 'port-randomization', 'hopping-interval', 'ports', 'port-hop',
    'hop-interval', 'random-port', 'port-range', 'mport', 'port-hopping-interval'
]));

// 🚀 预编译清理属性集合
const CLEANUP_PROPS = Object.freeze(['_priority', '_index', '_originalName', '_originalServer', '_testLatency', '_passedEndpoint', '_skip_reason', '_cipher_reason', '_quic-blocked']);



async function operator(proxies = []) {
    try {
        // 🛡️ 防御性检查：确保输入有效
        if (!Array.isArray(proxies) || proxies.length === 0) {
            console.log('[node_rules_entrance] 输入为空，返回空数组');
            return [];
        }

        console.log('[node_rules_entrance] 开始处理，输入节点数:', proxies.length);

        const _ = lodash;

        if (!_) {
            console.log('[node_rules_entrance] 错误: lodash 未定义');
            return proxies;
        }

        // --- 用户配置区域 ---
        // 通过修改以下 `cfg` 对象中的值，可以自定义脚本的行为。
        const cfg = {
            // true: 启用节点过滤，会根据下方的关键词、协议等规则筛选节点。
            // false: 禁用节点过滤，保留所有原始节点。
            filterMode: true,

            // true: 启用排序，会根据 `regions` 中定义的优先级 `p` 来排序节点。
            // false: 禁用排序，节点将保持其原始顺序。
            sortEnabled: true,

            // true: 优先级高的地区排在前面 (例如，香港 > 美国)。
            // false: 优先级高的地区排在后面。
            reverseSort: true,

            // ============================================================
            // 🚫 QUIC 屏蔽配置（阻止 UDP 443/QUIC 协议）
            // ============================================================

            // true: 启用 QUIC 屏蔽，强制所有流量使用传统 TCP/TLS
            // false: 允许 QUIC 协议（可能被某些网络环境限制或干扰）
            blockQuic: true,

            // QUIC 屏蔽选项
            quicBlockOptions: {
                // true: 为所有节点添加 QUIC 屏蔽规则
                enableForAllNodes: true,

                // 需要屏蔽的 QUIC 端口列表
                blockedPorts: [443, 80, 8443],

                // true: 禁用 HTTP/3（基于 QUIC）
                disableHttp3: true,

                // true: 禁用 0-RTT（某些协议的快速握手，基于 QUIC）
                disableZeroRtt: false,  // 保留以兼容 TUIC 等协议

                // 屏蔽方法: 'block-udp' (阻止 UDP), 'force-tcp' (强制 TCP), 'both' (两者都用)
                blockMethod: 'force-tcp'
            },

            // ============================================================
            // 🚀 节点性能增强 (Boost) 配置
            // ============================================================

            // true: 启用节点性能增强（通用优化 + 协议专属优化）
            // false: 禁用所有增强选项，保持原始配置
            enableBoost: true,

            // 通用增强选项
            boostOptions: {
                // true: 启用 TCP Fast Open (减少首次连接延迟)
                enableTcpFastOpen: true,

                // true: 启用 UDP 转发（支持游戏/DNS等）
                enableUdp: true,

                // true: 为 VMess 启用多路复用 (减少连接开销，某些服务器可能不支持)
                enableMux: true,

                // TLS 增强选项（仅当节点已启用 TLS 时生效）
                tlsBoost: {
                    // true: 添加 ALPN 协议协商 (优先 HTTP/2，Chrome 131 标准顺序)
                    enableAlpn: true,

                    // true: 启用 TLS 客户端指纹伪装
                    enableClientFingerprint: true,

                    // 🎭 v3.6.1: 智能指纹随机化配置
                    enableSmartFingerprint: true,
                    regionalFingerprints: {
                        '中国': ['qq', 'safari', 'chrome', 'edge', '360'],
                        '香港': ['chrome', 'safari', 'edge', 'firefox'],
                        '台湾': ['chrome', 'safari', 'edge', 'firefox'],
                        '澳门': ['chrome', 'safari', 'qq', 'edge'],
                        '日本': ['chrome', 'safari', 'edge', 'firefox'],
                        '韩国': ['chrome', 'safari', 'edge', 'firefox'],
                        '美国': ['chrome', 'safari', 'firefox', 'edge'],
                        '英国': ['chrome', 'safari', 'firefox', 'edge'],
                        '德国': ['chrome', 'firefox', 'safari', 'edge'],
                        '新加坡': ['chrome', 'safari', 'edge', 'firefox'],
                        '马来西亚': ['chrome', 'safari', 'edge'],
                        'default': ['chrome', 'safari', 'firefox', 'edge']
                    },
                    fingerprintType: 'chrome',

                    // 🔒 TLS 版本: 仅 1.3（Chrome 131 默认）
                    tlsMinVersion: '1.3',
                    tlsMaxVersion: '1.3',

                    // 🔒 skip-cert-verify: 默认允许不安全
                    // 原因：很多机场使用自签证书，强制验证会导致大量节点无法使用
                    // true = 允许不安全（推荐，保证可用性）
                    // false = 验证证书（安全但可能导致节点不可用）
                    skipCertVerify: true,

                    // 🎭 Chrome 131 完整 TLS 扩展配置
                    // 椭圆曲线组（按 Chrome 131 优先顺序）
                    curves: ['X25519', 'P-256', 'P-384'],

                    // 签名算法（Chrome 131 标准）
                    signatureAlgorithms: [
                        'ecdsa_secp256r1_sha256',
                        'rsa_pss_rsae_sha256',
                        'rsa_pkcs1_sha256'
                    ],

                    // 启用 GREASE 扩展（Chrome 特有，增加隐蔽性）
                    enableGrease: true,

                    // 启用 PSK 会话恢复（TLS 1.3 特性）
                    enablePsk: true,

                    // 🔒 椭圆曲线偏好 (Sing-box/Xray)
                    curves: ['X25519', 'P-256', 'P-384'],

                    // 🆕 v3.5: TLS Fragment 分片配置（绕过DPI检测）
                    // ⚠️ 重要：此功能仅 sing-box 支持！
                    // Clash Meta / Mihomo / Shadowrocket 都不支持 TLS 分片
                    // 如果你使用 Clash 订阅，此选项无效
                    enableTlsFragment: false,  // 默认关闭，仅 sing-box 用户手动开启
                    tlsFragmentOptions: {
                        // 分片长度范围（字节）
                        length: '100-200',
                        // 分片间隔（毫秒）
                        interval: '10-20',
                        // 分片模式: 'tlshello' 仅分片ClientHello, 'all' 分片所有TLS包
                        mode: 'tlshello'
                    },

                    // 🆕 v3.5: HTTP/2 增强配置
                    enableHttp2: true,
                    http2Options: {
                        // 启用HTTP/2多路复用
                        enableMultiplex: true,
                        // 启用HTTP/2服务器推送
                        enableServerPush: false,
                        // 初始窗口大小
                        initialWindowSize: 65535
                    }
                },

                // 传输层优化选项
                transportBoost: {
                    // true: 为 gRPC 传输添加服务名
                    enableGrpcOptimization: true,

                    // true: 为 VLESS/VMess 启用 packet-addr 数据包编码（v3.5替代xudp）
                    enableXudp: true,  // 配置名保持兼容，实际使用 packet-addr

                    // 🎭 WebSocket 伪装增强
                    wsHeaders: {
                        // 模拟真实浏览器 User-Agent
                        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
                        // Accept 头（匹配 Chrome 标准）
                        'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
                        // 语言偏好
                        'Accept-Language': 'en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7'
                    }
                },

                // 协议专属增强
                protocolSpecific: {
                    // VMess 专属
                    vmess: {
                        // true: 强制使用 AEAD 加密 (alterId = 0)
                        forceAead: true,

                        // 🔒 默认加密方法: 仅使用 AES-GCM（Chrome 131 兼容，不用 chacha20）
                        defaultCipher: 'aes-128-gcm',

                        // Fallback 加密方法（如 aes-128-gcm 不支持）
                        fallbackCipher: 'aes-256-gcm',

                        // 🎭 全局填充（增加流量随机性）
                        enableGlobalPadding: true
                    },

                    // Shadowsocks 专属
                    shadowsocks: {
                        // 🔒 默认加密方法: 仅使用 AES-GCM（Chrome 131 兼容，不用 chacha20）
                        defaultCipher: 'aes-128-gcm',

                        // Fallback 加密方法
                        fallbackCipher: 'aes-256-gcm',

                        // true: 启用 UDP over TCP
                        enableUdpOverTcp: true
                    },

                    // Hysteria2 专属
                    hysteria2: {
                        // 默认上传带宽 (如未配置，留空让节点自行协商)
                        defaultUpBandwidth: '',

                        // 默认下载带宽 (如未配置，留空让节点自行协商)
                        defaultDownBandwidth: '',

                        // true: 启用 MTU 发现
                        enableMtuDiscovery: true

                        // ⚠️ v3.5.7说明：Hysteria2 不支持 heartbeat 参数
                        // Hysteria2 使用 QUIC 协议内置的心跳机制，无需手动配置
                        // 如需心跳控制，请使用 TUIC 协议
                    },

                    // TUIC 专属
                    tuic: {
                        // 拥塞控制算法: 'bbr', 'cubic'
                        congestionController: 'bbr',

                        // UDP 中继模式: 'quic', 'native'
                        udpRelayMode: 'quic',

                        // true: 启用 0-RTT 握手
                        enableZeroRtt: true,

                        // 心跳间隔（秒）
                        heartbeatInterval: 3
                    },

                    // WireGuard 专属
                    wireguard: {
                        // 默认 MTU 值
                        defaultMtu: 1420
                    }
                }
            },

            // true: 为节点启用 ECN (显式拥塞通知)，可能在拥堵网络下提升性能。
            // false: 禁用 ECN。
            enableECN: true,

            // true: 优先使用 IPv6 地址进行连接（如果节点支持）。
            // false: 不改变节点的 IPV6 偏好。
            forceIPv6: true,

            // true: 为支持的协议 (VLESS, Trojan, VMess) 强制开启 TLS 加密。
            // false: 保持节点原有的 TLS 设置。
            forceTls: true,

            // true: 为支持的协议 (VLESS, Trojan, VMess) 强制使用 WebSocket 作为传输方式进行伪装。
            // false: 保持节点原有的传输方式。
            forceWsObfs: false,

            // true: 强制覆盖 SNI，即使原节点已有 SNI 值。
            // false: 仅在原节点无 SNI 时添加。
            forceSniOverride: true,

            // true: 强制覆盖 WebSocket 的 Host 头，即使原节点已有 Host 值。
            // false: 仅在原节点无 Host 时添加。
            forceObfsOverride: false,

            // true: 开启 ShadowTLS 扩展 (仅限 v2 或 v3)。
            // false: 禁用 ShadowTLS。
            shadowTlsEnabled: true,

            // ShadowTLS 版本: 2 或 3。
            shadowTlsVersion: 3,

            // true: 自动生成中继链 (入口 -> 落地)。
            // false: 不生成中继链。
            generateRelayChains: true,

            // 中继链使用的入口策略组名称，需要与你的 Clash/Substore 配置对应。
            relayEntryGroupName: '♻️ 自动入口 🧠',

            // true: 自动生成落地链 (中继 -> 落地)。
            // false: 不生成落地链。
            generateLandingChains: false,

            // 落地链使用的中继策略组名称，需要与你的 Clash/Substore 配置对应。
            landingEntryGroupName: '🚶 中继路径 🔐',

            // 控制最终输出的节点类型。
            // 'proxies_only': 输出处理后的入站节点（原始节点经过优化过滤和命名）。
            // 'relay_only': 只输出中继链节点。
            // 'landing_only': 只输出落地链节点。
            // 'airport_only': ✈️ 机场节点标识（只修改名称添加✈️，内部配置保持原始）。
            // 'dns_resolve': 将域名解析为 IP 地址。
            outputMode: 'relay_only',

            // 节点协议白名单，只有出现在此列表中的协议类型才会被处理和保留。
            protocols: ['vless', 'vmess', 'trojan', 'snell', 'hysteria2', 'hysteria', 'tuic', 'wireguard', 'https', 'ss', 'shadowsocks', 'http', 'socks5'],

            // 🌐 智能 SNI 选择策略（6 大 CDN 提供商 + 地区映射）
            // 根据节点地区智能匹配对应 CDN，提升隐私性和真实性
            regionalCdnMapping: {
                // 🇯🇵 日本节点
                '日本': [
                    'cloudflare.net', 'cdnjs.cloudflare.com',
                    'akamaized.net', 'akamaihd.net', 'edgesuite.net',
                    'googlevideo.com', 'gstatic.com', 'googleapis.com',
                    'fastly.net', 'global.fastly.net'
                ],
                // 🇰🇷 韩国节点
                '韩国': [
                    'cloudflare.com', 'workers.dev',
                    'edgesuite.net', 'edgekey.net', 'akamaized.net',
                    'azureedge.net', 'azure.microsoft.com',
                    'googleapis.com', 'gstatic.com'
                ],
                // 🇺🇸 美国节点
                '美国': [
                    'cloudfront.net', 'cdn.cloudflare.net', 'd1.awsstatic.com',
                    's3.amazonaws.com', 'ec2.amazonaws.com',
                    'akamai.net', 'akadns.net', 'edgesuite.net',
                    'fastly.net', 'fastlylb.net'
                ],
                // 🇭🇰 香港节点
                '香港': [
                    'cloudflare.com', 'one.one.one.one',
                    'akamaized.net', 'edgesuite.net',
                    'googlevideo.com', 'gstatic.com',
                    'upos-hz-mirrorakam.akamaized.net'  // Bilibili 海外 CDN
                ],
                // 🇹🇼 台湾节点
                '台湾': [
                    'cdnjs.cloudflare.com', 'cloudflare.net',
                    'gstatic.com', 'googleapis.com',
                    'akamaihd.net', 'edgekey.net'
                ],
                // 🇸🇬 新加坡节点
                '新加坡': [
                    'cloudflare.net', 'cdn.cloudflare.net',
                    's3-ap-southeast-1.amazonaws.com',
                    'akamaized.net', 'edgesuite.net'
                ],
                // 通用 CDN（用于其他地区或 Fallback）
                'default': [
                    'cloudflare.net', 'cdnjs.cloudflare.com', 'cloudfront.net',
                    'akamaized.net', 'edgesuite.net', 'fastly.net',
                    'gstatic.com', 'googleapis.com', 'youtube.com',
                    'azureedge.net', 'azure.microsoft.com'
                ]
            },

            // 🆕 v3.5增强：用于 TLS 伪装的 SNI (服务器名称指示) 域名列表（通用 Fallback）
            sni: [
                // AWS CDN
                'cloudfront.net', 'd1.awsstatic.com', 's3.amazonaws.com', 's3-us-west-2.amazonaws.com', 'ec2.amazonaws.com',
                'elasticbeanstalk.com', 'elb.amazonaws.com', 'lambda.amazonaws.com',
                // Azure CDN
                'azureedge.net', 'cdn.azureedge.net', 'azurewebsites.net', 'blob.core.windows.net', 'azure.microsoft.com',
                'trafficmanager.net', 'cloudapp.azure.com', 'vo.msecnd.net',
                // Akamai CDN
                'akamaized.net', 'edgesuite.net', 'edgekey.net', 'akamai.net', 'akadns.net',
                'akamaihd.net', 'akamaistream.net', 'akamaitechnologies.com',
                // Fastly CDN
                'fastly.net', 'global.fastly.net', 'fastlylb.net', 'fastly.com',
                // Cloudflare CDN
                'cdn.cloudflare.net', 'cdnjs.cloudflare.com', 'one.one.one.one', 'cloudflare.com', 'workers.dev',
                'cloudflare-dns.com', 'pages.dev', 'r2.cloudflarestorage.com',
                // Google CDN
                'gstatic.com', 'fonts.gstatic.com', 'googleapis.com', 'storage.googleapis.com', 'youtube.com',
                'googlevideo.com', 'ytimg.com', 'ggpht.com', 'googleusercontent.com', 'google.com',
                'gvt1.com', 'gvt2.com', 'gvt3.com', 'android.com', 'chrome.com',
                // Meta/Facebook CDN
                'fbcdn.net', 'xx.fbcdn.net', 'cdninstagram.com', 'instagram.com', 'facebook.com',
                'whatsapp.net', 'whatsapp.com', 'fb.com', 'fbsbx.com',
                // Apple CDN
                'apple.com', 'icloud.com', 'mzstatic.com', 'apple-cloudkit.com', 'cdn-apple.com',
                // Microsoft CDN
                'microsoft.com', 'msn.com', 'live.com', 'office.com', 'office365.com',
                'microsoftonline.com', 'windows.net', 'windowsupdate.com', 'bing.com',
                // 其他知名CDN
                'jsdelivr.net', 'unpkg.com', 'bootcdn.net', 'staticfile.org',
                'twimg.com', 'twitter.com', 'x.com', 't.co'
            ],

            // 🆕 v3.5增强：用于 WebSocket (ws) 伪装的 Host 请求头域名列表
            obfs: [
                // Microsoft 系列
                'www.bing.com', 'www.microsoft.com', 'update.microsoft.com', 'download.microsoft.com',
                'delivery.windowsupdate.com', 'windowsupdate.com', 'login.microsoftonline.com',
                'outlook.office365.com', 'teams.microsoft.com', 'onedrive.live.com',
                // 网易系列
                'cdn-go.cn', 'd1.music.126.net', 'music.163.com', 'netease.com',
                'api.m.163.com', 'interface.music.163.com', 'clientlog.music.163.com',
                // IBM/Softlayer
                'cdn.softlayer.net', 'ibm.com', 'akadns.net',
                // 中国联通系列 (10010)
                'yunpanlive.chinaunicomvideo.cn', 'mbh.chinaunicomvideo.cn', 'woshipin.chinaunicomvideo.cn',
                'tjtn.pan.wo.cn', 'pan.wo.cn', 'panservice.mail.wo.cn', 'm.wo.com.cn', 'img1.wo.com.cn', 'video.wo.com.cn',
                'pull.free.video.10010.com', 'free.video.10010.com', 'm.client.10010.com', 'img.client.10010.com',
                'shoutingtoutiao1.10010.com', 'shoutingtoutiao2.10010.com', 'shoutingtoutiao3.10010.com',
                'shoutingtoutiao4.10010.com', 'partner.iread.wo.com.cn', 'iread.wo.com.cn',
                'm1.ad.10010.com', 'ad.10010.com', 'wap.10010.com', 'uac.10010.com',
                // 中国移动系列 (10086)
                'touch.10086.cn', 'img.10086.cn', 'cmail.10086.cn', 'music.10086.cn',
                'wap.10086.cn', 'androm.10086.cn', 'mmarket.10086.cn', 'shop.10086.cn',
                'service.10086.cn', 'app.10086.cn', 'client.10086.cn', 'video.10086.cn',
                // 中国电信系列 (189)
                'www.189.cn', 'login.189.cn', 'e.189.cn', 'cloud.189.cn',
                'open.e.189.cn', 'api.cloud.189.cn', 'h5.cloud.189.cn',
                // 阿里系列
                'www.taobao.com', 'www.tmall.com', 'www.aliyun.com', 'cdn.aliyun.com',
                'g.alicdn.com', 'img.alicdn.com', 'assets.alicdn.com',
                // 腾讯系列
                'www.qq.com', 'weixin.qq.com', 'wx.qq.com', 'mp.weixin.qq.com',
                'cdn.qq.com', 'gtimg.com', 'qpic.cn', 'myqcloud.com',
                // 百度系列
                'www.baidu.com', 'pan.baidu.com', 'bce.baidu.com', 'cdn.bcebos.com',
                // 京东系列
                'www.jd.com', 'cdn.jd.com', 'img10.360buyimg.com', 'img11.360buyimg.com'
            ],

            // 地区识别与排序优先级配置。
            // 格式: '国旗Emoji': { n: ['关键词列表'], r: '重命名后的地区名', p: 优先级数字 (越小越靠前) }
            regions: {
                '🇨🇳': { n: ['中国', 'China', 'CN', 'PRC', 'Mainland', '北京', 'Beijing', '上海', 'Shanghai', '广东', 'Guangdong', '浙江', 'Zhejiang', '江苏', 'Jiangsu', '四川', 'Sichuan', '福建', 'Fujian', '湖北', 'Hubei', '湖南', 'Hunan', '山东', 'Shandong', '辽宁', 'Liaoning', '河南', 'Henan', '安徽', 'Anhui', '河北', 'Hebei', '陕西', 'Shaanxi', '重庆', 'Chongqing', '天津', 'Tianjin', '广西', 'Guangxi', '云南', 'Yunnan', '海南', 'Hainan', '江西', 'Jiangxi', '贵州', 'Guizhou', '甘肃', 'Gansu', '青海', 'Qinghai', '宁夏', 'Ningxia', '新疆', 'Xinjiang', '西藏', 'Tibet', '内蒙古', 'Inner Mongolia', '黑龙江', 'Heilongjiang', '吉林', 'Jilin', 'CMCC', 'China Mobile', 'CUCC', 'China Unicom', 'CTCC', 'China Telecom', 'Guangzhou', 'Shenzhen', 'Hangzhou', 'Nanjing', 'Suzhou', 'Chengdu', 'Wuhan', 'Changsha', 'Jinan', 'Qingdao', 'Shenyang', 'Dalian', 'Zhengzhou', 'Hefei', 'Shijiazhuang', 'Xian', 'Nanning', 'Kunming', 'Haikou', 'Nanchang', 'Guiyang', 'Lanzhou', 'Xining', 'Yinchuan', 'Urumqi', 'Lhasa', 'Hohhot', 'Harbin', 'Changchun'], r: '中国', p: 5 },
                '🇹🇼': { n: ['台湾', '台灣', 'Taiwan', 'TW', 'Taipei', 'Hinet', 'CHT', '中华电信', '新北', '高雄', '台中', '台南', 'Seednet', 'FarEasTone', 'Chunghwa', 'Taiwan Mobile', 'Kaohsiung', 'Taichung', 'Tainan', 'New Taipei', 'Hsinchu', 'Chiayi', 'Pingtung', 'Taoyuan', 'Yilan', 'Hualien', 'Taitung', 'Penghu', 'Kinmen', 'Matsu', 'TW Telecom', 'APTG', 'FET', 'TWM'], r: '台湾', p: 10 },
                '🇭🇰': { n: ['香港', 'Hong Kong', 'HK', 'HongKong', 'HKT', 'HGC', 'WTT', 'PCCW', 'CMI', 'CSL', 'SmarTone', 'Kowloon', 'New Territories', 'Hong Kong Broadband', 'Netvigator', 'Tsuen Wan', 'Sha Tin', 'Tai Po', 'Yuen Long', 'Tuen Mun', 'Kwun Tong', 'Wan Chai', 'Central', 'Causeway Bay', 'Mong Kok', 'HKBN', 'Hutchison'], r: '香港', p: 11 },
                '🇲🇴': { n: ['澳门', '澳門', 'Macau', 'MO', 'CTM', 'Macao', 'Companhia de Telecomunicacoes de Macau', 'Taipa', 'Coloane', 'Cotai', 'Macau Peninsula', 'Macau Telecom', 'MTEL'], r: '澳门', p: 12 },
                '🇯🇵': { n: ['日本', 'Japan', 'JP', 'Tokyo', 'Osaka', 'Saitama', 'Fukuoka', 'Nagoya', 'Sapporo', 'IIJ', 'NTT', 'KDDI', 'SoftBank', 'Rakuten', 'Kyoto', 'Hiroshima', 'Yokohama', 'Kobe', 'Sendai', 'Docomo', 'AU', 'Chiba', 'Kanagawa', 'Aichi', 'Hyogo', 'Shizuoka', 'Okayama', 'Kumamoto', 'Kagoshima', 'Okinawa', 'NTT Docomo', 'au by KDDI', 'Rakuten Mobile'], r: '日本', p: 13 },
                '🇰🇷': { n: ['韩国', '韓國', 'Korea', 'KR', 'Seoul', 'SK', 'KT', 'LG', 'Busan', 'Incheon', 'Daegu', 'Gwangju', 'Daejeon', 'Ulsan', 'SK Broadband', 'LG U+', 'Suwon', 'Jeju', 'Changwon', 'Cheongju', 'Ansan', 'Anyang', 'Goyang', 'Seongnam', 'Yongin', 'SK Telecom', 'KT Corp', 'LG Uplus'], r: '韩国', p: 14 },
                '🇸🇬': { n: ['新加坡', 'Singapore', 'SG', 'Singtel', 'StarHub', 'M1', 'Jurong', 'Changi', 'ViewQwest', 'MyRepublic', 'Orchard', 'Marina Bay', 'Sentosa', 'Punggol', 'Woodlands', 'Yishun', 'Tampines', 'Pasir Ris', 'Bedok', 'SingTel', 'StarHub Mobile', 'M1 Limited'], r: '新加坡', p: 20 },
                '🇲🇾': { n: ['马来西亚', '馬來西亞', 'Malaysia', 'MY', 'Kuala Lumpur', 'Penang', 'Johor', 'TM', 'Maxis', 'Celcom', 'U Mobile', 'Digi', 'Ipoh', 'Kuching', 'Kota Kinabalu', 'Shah Alam', 'Malacca', 'Klang', 'Subang Jaya', 'Petaling Jaya', 'George Town', 'Time dotCom', 'Maxis Broadband', 'Celcom Axiata', 'U Mobile Sdn Bhd', 'DiGi Telecommunications'], r: '马来西亚', p: 21 },
                '🇹🇭': { n: ['泰国', '泰國', 'Thailand', 'TH', 'Bangkok', 'Chiang Mai', 'Phuket', 'AIS', 'True', 'DTAC', 'Pattaya', 'Krabi', 'TrueMove', 'Hua Hin', 'Ayutthaya', 'Koh Samui', 'Chiang Rai', 'Hat Yai', 'Surat Thani', 'Nakhon Ratchasima', 'Udon Thani', 'Advanced Info Service', 'True Corporation', 'Total Access Communication'], r: '泰国', p: 22 },
                '🇻🇳': { n: ['越南', 'Vietnam', 'VN', 'Hanoi', 'Ho Chi Minh', 'Da Nang', 'Viettel', 'VNPT', 'FPT', 'Hai Phong', 'Can Tho', 'Nha Trang', 'Hue', 'Vung Tau', 'Bien Hoa', 'Thu Duc', 'Long An', 'Binh Duong', 'Dong Nai', 'Viettel Group', 'Vietnam Posts and Telecommunications', 'FPT Telecom'], r: '越南', p: 23 },
                '🇵🇭': { n: ['菲律宾', '菲律賓', 'Philippines', 'PH', 'Manila', 'Cebu', 'Globe', 'Smart', 'PLDT', 'Davao', 'Quezon', 'Makati', 'Tagaytay', 'Boracay', 'Angeles', 'Baguio', 'Batangas', 'Iloilo', 'Cagayan de Oro', 'Globe Telecom', 'Smart Communications', 'PLDT Inc'], r: '菲律宾', p: 24 },
                '🇮🇩': { n: ['印尼', '印度尼西亚', 'Indonesia', 'ID', 'Jakarta', 'Surabaya', 'Bali', 'Telkomsel', 'Indosat', 'XL Axiata', 'Bandung', 'Medan', 'Yogyakarta', 'Semarang', 'Makassar', 'Palembang', 'Batam', 'Denpasar', 'Pekanbaru', 'Padang', 'Telkom Indonesia', 'Indosat Ooredoo', 'XL Axiata'], r: '印尼', p: 25 },
                '🇰🇭': { n: ['柬埔寨', 'Cambodia', 'KH', 'Phnom Penh', 'Siem Reap', 'Cellcard', 'Smart Axiata', 'Metfone', 'Sihanoukville', 'Battambang', 'Kampong Cham', 'Poipet', 'Kampot', 'Kep', 'Sisophon', 'Krong Preah Sihanouk', 'Mobitel', 'Smart Axiata Co Ltd', 'Vietnamese Metfone'], r: '柬埔寨', p: 26 },
                '🇺🇸': { n: ['美国', '美國', 'USA', 'US', 'United States', 'America', 'Los Angeles', 'San Jose', 'SJ', 'LA', 'New York', 'NY', 'Ashburn', 'Seattle', 'Chicago', 'Dallas', 'Miami', 'Atlanta', 'SEA', 'CHI', 'DAL', 'MIA', 'ATL', 'San Francisco', 'SF', 'Boston', 'Houston', 'Phoenix', 'Denver', 'Las Vegas', 'Verizon', 'AT&T', 'Comcast', 'T-Mobile', 'Philadelphia', 'Washington DC', 'Orlando', 'Portland', 'Austin', 'San Diego', 'Detroit', 'Minneapolis', 'Tampa', 'Charlotte', 'Verizon Wireless', 'AT&T Mobility', 'Comcast Cable', 'T-Mobile USA'], r: '美国', p: 30 },
                '🇨🇦': { n: ['加拿大', 'Canada', 'CA', 'Toronto', 'Vancouver', 'Montreal', 'Ottawa', 'Calgary', 'Bell', 'Rogers', 'Telus', 'Edmonton', 'Quebec', 'Winnipeg', 'Halifax', 'Victoria', 'Mississauga', 'Brampton', 'Surrey', 'Laval', 'London ON', 'Bell Canada', 'Rogers Communications', 'Telus Communications'], r: '加拿大', p: 31 },
                '🇲🇽': { n: ['墨西哥', 'Mexico', 'MX', 'Mexico City', 'Guadalajara', 'Monterrey', 'Telcel', 'Movistar', 'Tijuana', 'Puebla', 'Cancun', 'Merida', 'Leon', 'Juarez', 'Zapopan', 'Naucalpan', 'Chihuahua', 'America Movil', 'Telefonica Movistar', 'AT&T Mexico'], r: '墨西哥', p: 32 },
                '🇬🇧': { n: ['英国', '英國', 'UK', 'GB', 'United Kingdom', 'England', 'London', 'Manchester', 'Edinburgh', 'Glasgow', 'EE', 'Vodafone', 'O2', 'Three UK', 'Birmingham', 'Liverpool', 'Leeds', 'Sheffield', 'Bristol', 'Leicester', 'Coventry', 'Bradford', 'Cardiff', 'Belfast', 'EE Limited', 'Vodafone UK', 'O2 UK', 'Three UK'], r: '英国', p: 40 },
                '🇩🇪': { n: ['德国', '德國', 'Germany', 'DE', 'Frankfurt', 'Berlin', 'Munich', 'Düsseldorf', 'Hamburg', 'Deutsche Telekom', 'Vodafone DE', 'O2 DE', 'Cologne', 'Stuttgart', 'Dresden', 'Leipzig', 'Nuremberg', 'Duisburg', 'Bochum', 'Wuppertal', 'Bielefeld', 'Bonn', 'Deutsche Telekom AG', 'Vodafone GmbH', 'Telefonica Germany'], r: '德国', p: 41 },
                '🇫🇷': { n: ['法国', '法國', 'France', 'FR', 'Paris', 'Marseille', 'Lyon', 'Orange', 'SFR', 'Bouygues', 'Toulouse', 'Nice', 'Nantes', 'Strasbourg', 'Montpellier', 'Bordeaux', 'Lille', 'Rennes', 'Reims', 'Le Havre', 'Orange S.A.', 'SFR Group', 'Bouygues Telecom'], r: '法国', p: 42 },
                '🇳🇱': { n: ['荷兰', '荷蘭', 'Netherlands', 'NL', 'Amsterdam', 'Rotterdam', 'The Hague', 'KPN', 'Ziggo', 'T-Mobile NL', 'Utrecht', 'Eindhoven', 'Groningen', 'Tilburg', 'Breda', 'Nijmegen', 'Apeldoorn', 'Haarlem', 'Arnhem', 'Zaanstad', 'KPN Telecom', 'Ziggo B.V.', 'T-Mobile Netherlands'], r: '荷兰', p: 43 },
                '🇷🇺': { n: ['俄罗斯', '俄羅斯', 'Russia', 'RU', 'Moscow', 'Saint Petersburg', 'Novosibirsk', 'MTS', 'Beeline', 'MegaFon', 'Yekaterinburg', 'Kazan', 'Nizhny Novgorod', 'Chelyabinsk', 'Samara', 'Omsk', 'Rostov', 'Ufa', 'Krasnoyarsk', 'Voronezh', 'Mobile TeleSystems', 'VimpelCom', 'MegaFon'], r: '俄罗斯', p: 44 },
                '🇨🇭': { n: ['瑞士', 'Switzerland', 'CH', 'Zurich', 'Geneva', 'Basel', 'Swisscom', 'Sunrise', 'Salt', 'Bern', 'Lausanne', 'Lucerne', 'St. Gallen', 'Winterthur', 'Biel', 'Thun', 'Koniz', 'La Chaux-de-Fonds', 'Schaffhausen', 'Swisscom AG', 'Sunrise Communications', 'Salt Mobile'], r: '瑞士', p: 45 },
                '🇸🇪': { n: ['瑞典', 'Sweden', 'SE', 'Stockholm', 'Gothenburg', 'Malmö', 'Telia', 'Tele2', 'Telenor SE', 'Uppsala', 'Linköping', 'Örebro', 'Västerås', 'Helsingborg', 'Norrkoping', 'Jonkoping', 'Lund', 'Umea', 'Gavle', 'Telia Company', 'Tele2 AB', 'Telenor Sverige'], r: '瑞典', p: 46 },
                '🇮🇪': { n: ['爱尔兰', 'Ireland', 'IE', 'Dublin', 'Cork', 'Vodafone IE', 'Three', 'Eir', 'Galway', 'Limerick', 'Waterford', 'Drogheda', 'Dundalk', 'Swords', 'Bray', 'Navan', 'Kilkenny', 'Ennis', 'Vodafone Ireland', 'Three Ireland', 'Eircom'], r: '爱尔兰', p: 47 },
                '🇮🇹': { n: ['意大利', 'Italy', 'IT', 'Milan', 'Rome', 'Naples', 'TIM', 'Vodafone IT', 'Wind Tre', 'Turin', 'Palermo', 'Genoa', 'Bologna', 'Florence', 'Bari', 'Catania', 'Venice', 'Verona', 'Messina', 'Telecom Italia', 'Vodafone Italia', 'Wind Tre S.p.A.'], r: '意大利', p: 48 },
                '🇪🇸': { n: ['西班牙', 'Spain', 'ES', 'Madrid', 'Barcelona', 'Valencia', 'Movistar', 'Orange ES', 'Vodafone ES', 'Seville', 'Bilbao', 'Malaga', 'Zaragoza', 'Murcia', 'Palma', 'Las Palmas', 'Alicante', 'Cordoba', 'Valladolid', 'Telefonica Movistar', 'Orange Espagne', 'Vodafone Espana'], r: '西班牙', p: 49 },
                '🇵🇱': { n: ['波兰', '波蘭', 'Poland', 'PL', 'Warsaw', 'Krakow', 'Wroclaw', 'Plus', 'Play', 'Orange PL', 'Poznan', 'Gdansk', 'Szczecin', 'Bydgoszcz', 'Lublin', 'Katowice', 'Bialystok', 'Gdynia', 'Czestochowa', 'Radom', 'Polkomtel Plus', 'P4 Play', 'Orange Polska'], r: '波兰', p: 50 },
                '🇺🇦': { n: ['乌克兰', '烏克蘭', 'Ukraine', 'UA', 'Kyiv', 'Lviv', 'Odessa', 'Kyivstar', 'Vodafone UA', 'Lifecell', 'Kharkiv', 'Dnipro', 'Donetsk', 'Zaporizhzhia', 'Ivano-Frankivsk', 'Mykolaiv', 'Vinnytsia', 'Zhytomyr', 'Sumy', 'Chernivtsi', 'Kyivstar PJSC', 'Vodafone Ukraine', 'Lifecell LLC'], r: '乌克兰', p: 51 },
                '🇫🇮': { n: ['芬兰', 'Finland', 'FI', 'Helsinki', 'Espoo', 'Tampere', 'Elisa', 'DNA', 'Telia FI', 'Turku', 'Oulu', 'Jyväskylä', 'Lahti', 'Kuopio', 'Pori', 'Kouvola', 'Joensuu', 'Lappeenranta', 'Hameenlinna', 'Elisa Oyj', 'DNA Oyj', 'Telia Finland'], r: '芬兰', p: 52 },
                '🇳🇴': { n: ['挪威', 'Norway', 'NO', 'Oslo', 'Bergen', 'Stavanger', 'Telenor', 'Telia NO', 'Ice', 'Trondheim', 'Kristiansand', 'Tromsø', 'Drammen', 'Fredrikstad', 'Sandnes', 'Sarpsborg', 'Skien', 'Alesund', 'Tonsberg', 'Telenor Norge', 'Telia Norge', 'Ice Communication'], r: '挪威', p: 53 },
                '🇩🇰': { n: ['丹麦', 'Denmark', 'DK', 'Copenhagen', 'Aarhus', 'Odense', 'TDC', 'Telenor DK', 'Telia DK', 'Aalborg', 'Esbjerg', 'Randers', 'Kolding', 'Horsens', 'Vejle', 'Roskilde', 'Herning', 'Silkeborg', 'Naestved', 'TDC Group', 'Telenor Denmark', 'Telia Denmark'], r: '丹麦', p: 54 },
                '🇦🇹': { n: ['奥地利', 'Austria', 'AT', 'Vienna', 'Graz', 'Linz', 'A1', 'Magenta', 'Three AT', 'Salzburg', 'Innsbruck', 'Klagenfurt', 'Villach', 'Wels', 'Sankt Polten', 'Dornbirn', 'Wiener Neustadt', 'Steyr', 'Feldkirch', 'A1 Telekom Austria', 'Magenta Telekom', 'Three Austria'], r: '奥地利', p: 55 },
                '🇧🇪': { n: ['比利时', 'Belgium', 'BE', 'Brussels', 'Antwerp', 'Ghent', 'Proximus', 'Telenet', 'Orange BE', 'Charleroi', 'Liege', 'Bruges', 'Namur', 'Leuven', 'Mons', 'Mechelen', 'Aalst', 'Kortrijk', 'Hasselt', 'Proximus Group', 'Telenet Group', 'Orange Belgium'], r: '比利时', p: 56 },
                '🇨🇿': { n: ['捷克', 'Czechia', 'CZ', 'Prague', 'Brno', 'Ostrava', 'O2 CZ', 'Vodafone CZ', 'T-Mobile CZ', 'Plzen', 'Olomouc', 'Liberec', 'Ceske Budejovice', 'Hradec Kralove', 'Usti nad Labem', 'Pardubice', 'Havirov', 'Zlin', 'Kladno', 'O2 Czech Republic', 'Vodafone Czech', 'T-Mobile Czech'], r: '捷克', p: 57 },
                '🇭🇺': { n: ['匈牙利', 'Hungary', 'HU', 'Budapest', 'Debrecen', 'Szeged', 'Telekom HU', 'Vodafone HU', 'Digi HU', 'Miskolc', 'Pecs', 'Gyor', 'Nyiregyhaza', 'Kecskemet', 'Szekesfehervar', 'Szombathely', 'Szolnok', 'Tatabanya', 'Kaposvar', 'Magyar Telekom', 'Vodafone Hungary', 'Digi Communications'], r: '匈牙利', p: 58 },
                '🇷🇴': { n: ['罗马尼亚', 'Romania', 'RO', 'Bucharest', 'Cluj', 'Timisoara', 'Orange RO', 'Vodafone RO', 'Digi RO', 'Iasi', 'Constanta', 'Craiova', 'Brasov', 'Galati', 'Ploiesti', 'Oradea', 'Braila', 'Arad', 'Pitesti', 'Orange Romania', 'Vodafone Romania', 'Digi Communications RO'], r: '罗马尼亚', p: 59 },
                '🇧🇷': { n: ['巴西', 'Brazil', 'BR', 'Sao Paulo', 'Rio de Janeiro', 'Brasilia', 'Vivo', 'Claro', 'TIM BR', 'Salvador', 'Fortaleza', 'Belo Horizonte', 'Manaus', 'Curitiba', 'Recife', 'Porto Alegre', 'Belem', 'Goiania', 'Guarulhos', 'Vivo Telefonica', 'Claro Brasil', 'TIM Brasil'], r: '巴西', p: 60 },
                '🇦🇷': { n: ['阿根廷', 'Argentina', 'AR', 'Buenos Aires', 'Cordoba', 'Rosario', 'Movistar AR', 'Claro AR', 'Personal', 'Mendoza', 'Tucuman', 'La Plata', 'Mar del Plata', 'Salta', 'Santa Fe', 'San Juan', 'Resistencia', 'Neuquen', 'Corrientes', 'Movistar Argentina', 'Claro Argentina', 'Personal Telecom'], r: '阿根廷', p: 61 },
                '🇨🇱': { n: ['智利', 'Chile', 'CL', 'Santiago', 'Valparaiso', 'Concepcion', 'Entel', 'Movistar CL', 'WOM', 'Antofagasta', 'Vina del Mar', 'Temuco', 'La Serena', 'Iquique', 'Talca', 'Arica', 'Puerto Montt', 'Coyhaique', 'Punta Arenas', 'Entel Chile', 'Movistar Chile', 'WOM Chile'], r: '智利', p: 62 },
                '🇨🇴': { n: ['哥伦比亚', 'Colombia', 'CO', 'Bogota', 'Medellin', 'Cali', 'Claro CO', 'Movistar CO', 'Tigo', 'Barranquilla', 'Cartagena', 'Bucaramanga', 'Pereira', 'Santa Marta', 'Manizales', 'Ibague', 'Villavicencio', 'Cucuta', 'Monteria', 'Claro Colombia', 'Movistar Colombia', 'Tigo Colombia'], r: '哥伦比亚', p: 63 },
                '🇦🇺': { n: ['澳洲', '澳大利亚', 'Australia', 'AU', 'Sydney', 'Melbourne', 'Brisbane', 'Perth', 'Telstra', 'Optus', 'Vodafone AU', 'Adelaide', 'Gold Coast', 'Canberra', 'Hobart', 'Darwin', 'Newcastle', 'Wollongong', 'Geelong', 'Cairns', 'Townsville', 'Telstra Corporation', 'Optus Mobile', 'Vodafone Australia'], r: '澳洲', p: 70 },
                '🇳🇿': { n: ['新西兰', 'New Zealand', 'NZ', 'Auckland', 'Wellington', 'Christchurch', 'Spark', 'Vodafone NZ', '2degrees', 'Hamilton', 'Dunedin', 'Tauranga', 'Palmerston North', 'Napier', 'Hastings', 'New Plymouth', 'Rotorua', 'Whangarei', 'Invercargill', 'Spark New Zealand', 'Vodafone New Zealand', '2degrees Mobile'], r: '新西兰', p: 71 },
                '🇹🇷': { n: ['土耳其', 'Turkey', 'TR', 'Istanbul', 'Ankara', 'Izmir', 'Turkcell', 'Vodafone TR', 'Turk Telekom', 'Bursa', 'Antalya', 'Adana', 'Gaziantep', 'Konya', 'Mersin', 'Diyarbakir', 'Kayseri', 'Eskisehir', 'Urfa', 'Turkcell Iletisim', 'Vodafone Turkey', 'Turk Telekom'], r: '土耳其', p: 80 },
                '🇦🇪': { n: ['阿联酋', 'UAE', 'AE', 'Dubai', 'Abu Dhabi', 'Sharjah', 'Etisalat', 'du', 'Ajman', 'Ras Al Khaimah', 'Fujairah', 'Umm Al Quwain', 'Al Ain', 'Khor Fakkan', 'Dibba', 'Madinat Zayed', 'Liwa', 'Ruwais', 'Etisalat UAE', 'du Telecom'], r: '阿联酋', p: 81 },
                '🇮🇱': { n: ['以色列', 'Israel', 'IL', 'Tel Aviv', 'Jerusalem', 'Haifa', 'Cellcom', 'Partner', 'Pelephone', 'Netanya', 'Beersheba', 'Ashdod', 'Rishon LeZion', 'Petah Tikva', 'Ashkelon', 'Rehovot', 'Holon', 'Bat Yam', 'Ramat Gan', 'Cellcom Israel', 'Partner Communications', 'Pelephone Communications'], r: '以色列', p: 82 },
                '🇸🇦': { n: ['沙特', '沙特阿拉伯', 'Saudi Arabia', 'SA', 'Riyadh', 'Jeddah', 'Dammam', 'STC', 'Mobily', 'Zain SA', 'Mecca', 'Medina', 'Taif', 'Buraydah', 'Tabuk', 'Abha', 'Khamis Mushait', 'Hail', 'Hofuf', 'Yanbu', 'Saudi Telecom Company', 'Mobily Etihad Etisalat', 'Zain Saudi Arabia'], r: '沙特', p: 83 },
                '🇶🇦': { n: ['卡塔尔', 'Qatar', 'QA', 'Doha', 'Ooredoo', 'Vodafone QA', 'Al Rayyan', 'Al Wakrah', 'Al Khor', 'Dukhan', 'Mesaieed', 'Al Shahaniya', 'Al Shamal', 'Umm Salal', 'Al Daayen', 'Madīnat ash Shamāl', 'Ooredoo Qatar', 'Vodafone Qatar'], r: '卡塔尔', p: 84 },
                '🇮🇳': { n: ['印度', 'India', 'IN', 'Mumbai', 'Delhi', 'Bangalore', 'Airtel', 'Jio', 'Vodafone Idea', 'Chennai', 'Hyderabad', 'Kolkata', 'Pune', 'Ahmedabad', 'Jaipur', 'Lucknow', 'Kanpur', 'Nagpur', 'Indore', 'Bharti Airtel', 'Reliance Jio', 'Vodafone Idea Limited'], r: '印度', p: 90 },
                '🇿🇦': { n: ['南非', 'South Africa', 'ZA', 'Johannesburg', 'Cape Town', 'Durban', 'Vodacom', 'MTN', 'Cell C', 'Pretoria', 'Port Elizabeth', 'Bloemfontein', 'East London', 'Pietermaritzburg', 'Polokwane', 'Nelspruit', 'Kimberley', 'Rustenburg', 'Witbank', 'Vodacom South Africa', 'MTN South Africa', 'Cell C'], r: '南非', p: 91 },
                '🇪🇬': { n: ['埃及', 'Egypt', 'EG', 'Cairo', 'Alexandria', 'Giza', 'Orange EG', 'Vodafone EG', 'Etisalat EG', 'Luxor', 'Aswan', 'Port Said', 'Suez', 'Mansoura', 'Tanta', 'Mahalla', 'Assiut', 'Fayoum', 'Zagazig', 'Orange Egypt', 'Vodafone Egypt', 'Etisalat Egypt'], r: '埃及', p: 92 },
                '🇵🇰': { n: ['巴基斯坦', 'Pakistan', 'PK', 'Karachi', 'Lahore', 'Islamabad', 'Jazz', 'Telenor PK', 'Zong', 'Peshawar', 'Quetta', 'Faisalabad', 'Rawalpindi', 'Multan', 'Gujranwala', 'Sialkot', 'Hyderabad PK', 'Abbottabad', 'Islamabad Capital', 'Mobilink Jazz', 'Telenor Pakistan', 'Zong Pakistan'], r: '巴基斯坦', p: 93 },
                '🇧🇩': { n: ['孟加拉', 'Bangladesh', 'BD', 'Dhaka', 'Chittagong', 'Grameenphone', 'Robi', 'Banglalink', 'Khulna', 'Sylhet', 'Rajshahi', 'Barisal', 'Mymensingh', 'Rangpur', 'Comilla', 'Narayanganj', 'Gazipur', 'Savar', 'Grameenphone Ltd', 'Robi Axiata', 'Banglalink Digital'], r: '孟加拉', p: 94 },
                '🇳🇵': { n: ['尼泊尔', 'Nepal', 'NP', 'Kathmandu', 'Pokhara', 'Ncell', 'NTC', 'Biratnagar', 'Lalitpur', 'Bharatpur', 'Birgunj', 'Janakpur', 'Hetauda', 'Dharan', 'Butwal', 'Mahendranagar', 'Nepalgunj', 'Ncell Axiata', 'Nepal Telecom'], r: '尼泊尔', p: 95 },
                '🇰🇵': { n: ['朝鲜', 'North Korea', 'KP', 'Pyongyang', 'Koryolink', 'Hamhung', 'Chongjin', 'Nampo', 'Wonsan', 'Sinuiju', 'Kaesong', 'Hyesan', 'Kanggye', 'Haeju', 'Sariwon', 'Koryolink Mobile'], r: '朝鲜', p: 96 },
                '🇲🇲': { n: ['缅甸', 'Myanmar', 'MM', 'Yangon', 'Mandalay', 'Telenor MM', 'Ooredoo MM', 'MPT', 'Naypyidaw', 'Bago', 'Mawlamyine', 'Taunggyi', 'Pathein', 'Monywa', 'Sittwe', 'Meiktila', 'Myitkyina', 'Taungoo', 'Telenor Myanmar', 'Ooredoo Myanmar', 'Myanma Posts and Telecommunications'], r: '缅甸', p: 97 },
                '🇱🇰': { n: ['斯里兰卡', 'Sri Lanka', 'LK', 'Colombo', 'Kandy', 'Dialog', 'Mobitel', 'Hutch', 'Galle', 'Jaffna', 'Negombo', 'Kurunegala', 'Anuradhapura', 'Ratnapura', 'Batticaloa', 'Matara', 'Trincomalee', 'Badulla', 'Dialog Axiata', 'Mobitel Sri Lanka', 'Hutchison Telecommunications'], r: '斯里兰卡', p: 98 },
                '🇲🇳': { n: ['蒙古', 'Mongolia', 'MN', 'Ulaanbaatar', 'Unitel', 'Mobicom', 'G-Mobile', 'Erdenet', 'Darkhan', 'Choibalsan', 'Murun', 'Ulgii', 'Khovd', 'Bayankhongor', 'Arvaikheer', 'Sainshand', 'Dalanzadgad', 'Unitel LLC', 'Mobicom Corporation', 'G-Mobile LLC'], r: '蒙古', p: 99 },
                '🇰🇿': { n: ['哈萨克斯坦', 'Kazakhstan', 'KZ', 'Almaty', 'Astana', 'Kcell', 'Beeline KZ', 'Tele2 KZ', 'Shymkent', 'Karaganda', 'Aktobe', 'Pavlodar', 'Taraz', 'Semey', 'Atyrau', 'Kostanay', 'Oral', 'Petropavl', 'Kcell JSC', 'Beeline Kazakhstan', 'Tele2 Kazakhstan'], r: '哈萨克斯坦', p: 100 },
                '🇺🇿': { n: ['乌兹别克斯坦', 'Uzbekistan', 'UZ', 'Tashkent', 'Beeline UZ', 'Ucell', 'UMS', 'Samarkand', 'Bukhara', 'Namangan', 'Andijan', 'Nukus', 'Fergana', 'Qarshi', 'Jizzakh', 'Urgench', 'Termez', 'Beeline Uzbekistan', 'Ucell Coscom', 'UMS LLC'], r: '乌兹别克斯坦', p: 101 },
                '🇵🇹': { n: ['葡萄牙', 'Portugal', 'PT', 'Lisbon', 'Porto', 'Vodafone PT', 'NOS', 'MEO', 'Braga', 'Coimbra', 'Faro', 'Funchal', 'Ponta Delgada', 'Aveiro', 'Viseu', 'Vodafone Portugal', 'NOS Comunicações', 'MEO S.A.'], r: '葡萄牙', p: 65 },
                '🇬🇷': { n: ['希腊', 'Greece', 'GR', 'Athens', 'Thessaloniki', 'Cosmote', 'Vodafone GR', 'Nova', 'Patras', 'Heraklion', 'Larissa', 'Volos', 'Rhodes', 'Ioannina', 'Chania', 'Cosmote Mobile', 'Vodafone Greece', 'Nova-Wind'], r: '希腊', p: 66 },
                '🇳🇴': { n: ['挪威', 'Norway', 'NO', 'Oslo', 'Bergen', 'Telenor', 'Telia NO', 'Ice', 'Stavanger', 'Trondheim', 'Drammen', 'Fredrikstad', 'Kristiansand', 'Sandnes', 'Tromso', 'Sarpsborg', 'Tonsberg', 'Telenor Norway', 'Telia Norge', 'Ice Communication'], r: '挪威', p: 67 },
                '🇩🇰': { n: ['丹麦', 'Denmark', 'DK', 'Copenhagen', 'Aarhus', 'TDC', 'Telenor DK', 'Telia DK', 'Odense', 'Aalborg', 'Esbjerg', 'Randers', 'Kolding', 'Horsens', 'Vejle', 'Herning', 'Roskilde', 'Silkeborg', 'TDC Group', 'Telenor Denmark', 'Telia Denmark'], r: '丹麦', p: 68 },
                '🇰🇪': { n: ['肯尼亚', 'Kenya', 'KE', 'Nairobi', 'Mombasa', 'Safaricom', 'Airtel KE', 'Telkom KE', 'Kisumu', 'Nakuru', 'Eldoret', 'Nanyuki', 'Malindi', 'Thika', 'Kitale', 'Garissa', 'Safaricom PLC', 'Airtel Kenya', 'Telkom Kenya'], r: '肯尼亚', p: 110 },
                '🇳🇬': { n: ['尼日利亚', 'Nigeria', 'NG', 'Lagos', 'Abuja', 'MTN NG', 'Airtel NG', 'Glo', 'Port Harcourt', 'Ibadan', 'Kano', 'Benin City', 'Enugu', 'Kaduna', 'Uyo', 'Warri', 'Jos', 'MTN Nigeria', 'Airtel Nigeria', 'Globacom'], r: '尼日利亚', p: 111 },
                '🇲🇦': { n: ['摩洛哥', 'Morocco', 'MA', 'Casablanca', 'Rabat', 'Maroc Telecom', 'Orange MA', 'Inwi', 'Fes', 'Tangier', 'Agadir', 'Meknes', 'Oujda', 'Kenitra', 'Tetouan', 'Safi', 'El Jadida', 'Nador', 'Maroc Telecom', 'Orange Maroc', 'Inwi'], r: '摩洛哥', p: 112 },
                '🇩🇿': { n: ['阿尔及利亚', 'Algeria', 'DZ', 'Algiers', 'Oran', 'Mobilis', 'Djezzy', 'Ooredoo DZ', 'Constantine', 'Annaba', 'Blida', 'Batna', 'Setif', 'Sidi Bel Abbès', 'Biskra', 'Béjaïa', 'Tizi Ouzou', 'Mobilis DZ', 'Djezzy GSM', 'Ooredoo Algeria'], r: '阿尔及利亚', p: 113 }
            },

            // 用于美化节点名称的 Emoji 列表。
            // p: Premium (高级), f: Fast (高速), s: Stable (稳定), d: Default (默认)
            emoji: {
                p: ['💎', '👑', '⭐'],
                f: ['⚡', '🚀', '💨'],
                s: ['🛡️', '🔒', '💯'],
                d: ['✨', '🔥', '🌟']
            },

            // 🎨 节点命名美化配置
            naming: {
                // 命名风格: 'minimal' (简约), 'standard' (标准), 'detailed' (详细)
                style: 'minimal',

                // 是否显示序号
                showIndex: true,

                // 序号分隔符
                indexSeparator: '·',

                // 是否显示特性emoji
                showFeatureEmoji: true,

                // 地区名称映射（简化显示）
                regionShortNames: {
                    '香港': 'HK', '台湾': 'TW', '日本': 'JP', '韩国': 'KR',
                    '新加坡': 'SG', '美国': 'US', '英国': 'UK', '德国': 'DE',
                    '法国': 'FR', '荷兰': 'NL', '澳洲': 'AU', '加拿大': 'CA',
                    '俄罗斯': 'RU', '印度': 'IN', '巴西': 'BR', '马来西亚': 'MY',
                    '泰国': 'TH', '越南': 'VN', '菲律宾': 'PH', '印尼': 'ID',
                    '土耳其': 'TR', '阿联酋': 'AE', '瑞士': 'CH', '瑞典': 'SE',
                    '意大利': 'IT', '西班牙': 'ES', '波兰': 'PL', '奥地利': 'AT',
                    '比利时': 'BE', '捷克': 'CZ', '新西兰': 'NZ', '南非': 'ZA',
                    '澳门': 'MO', '中国': 'CN', '爱尔兰': 'IE', '芬兰': 'FI',
                    '挪威': 'NO', '丹麦': 'DK', '乌克兰': 'UA', '匈牙利': 'HU',
                    '罗马尼亚': 'RO', '阿根廷': 'AR', '智利': 'CL', '哥伦比亚': 'CO',
                    '墨西哥': 'MX', '葡萄牙': 'PT', '希腊': 'GR', '以色列': 'IL',
                    '沙特': 'SA', '卡塔尔': 'QA', '埃及': 'EG', '柬埔寨': 'KH',
                    '哈萨克斯坦': 'KZ', '巴基斯坦': 'PK', '孟加拉': 'BD',
                    '斯里兰卡': 'LK', '蒙古': 'MN', '缅甸': 'MM', '尼泊尔': 'NP',
                    '肯尼亚': 'KE', '尼日利亚': 'NG', '其他': 'XX'
                }
            }
        };

        // 🚀 性能优化：使用更高效的随机选择（避免重复创建）
        const getRandItem = (arr) => {
            if (!arr || arr.length === 0) return null;
            return arr[Math.floor(Math.random() * arr.length)];
        };

        // 🌐 智能 SNI 选择：根据节点地区匹配对应 CDN（带缓存）
        const sniCache = new Map();
        const getSmartSni = (regionName) => {
            // 使用缓存避免重复计算
            if (sniCache.has(regionName)) {
                const cached = sniCache.get(regionName);
                return getRandItem(cached);
            }

            let cdnList;
            // 1. 尝试从地区 CDN 映射中获取
            if (cfg.regionalCdnMapping[regionName] && cfg.regionalCdnMapping[regionName].length > 0) {
                cdnList = cfg.regionalCdnMapping[regionName];
            }
            // 2. Fallback: 使用通用 CDN
            else if (cfg.regionalCdnMapping['default'] && cfg.regionalCdnMapping['default'].length > 0) {
                cdnList = cfg.regionalCdnMapping['default'];
            }
            // 3. 最终 Fallback: 使用原始 SNI 列表
            else {
                cdnList = cfg.sni;
            }

            sniCache.set(regionName, cdnList);
            return getRandItem(cdnList);
        };

        // 原始随机 SNI 选择（用于不需要智能选择的场景）
        const getRandomSni = () => getRandItem(cfg.sni);
        const getRandomObfs = () => getRandItem(cfg.obfs);

        // 🌍 v3.6.1: 域名扩展名到地区映射 - 智能检测节点地区
        const detectRegionFromDomain = (server) => {
            if (!server || typeof server !== 'string') return null;
            const lowerServer = server.toLowerCase();

            // 域名扩展名映射表（按优先级排序）
            const domainRegionMap = {
                '.nl': { f: '🇳🇱', r: '荷兰', p: 19 },
                '.ch': { f: '🇨🇭', r: '瑞士', p: 24 },
                '.ru': { f: '🇷🇺', r: '俄罗斯', p: 25 },
                '.au': { f: '🇦🇺', r: '澳洲', p: 27 },
                '.de': { f: '🇩🇪', r: '德国', p: 16 },
                '.fr': { f: '🇫🇷', r: '法国', p: 17 },
                '.uk': { f: '🇬🇧', r: '英国', p: 15 },
                '.ca': { f: '🇨🇦', r: '加拿大', p: 28 },
                '.br': { f: '🇧🇷', r: '巴西', p: 41 },
                '.it': { f: '🇮🇹', r: '意大利', p: 52 },
                '.es': { f: '🇪🇸', r: '西班牙', p: 53 },
                '.se': { f: '🇸🇪', r: '瑞典', p: 61 },
                '.no': { f: '🇳🇴', r: '挪威', p: 67 },
                '.dk': { f: '🇩🇰', r: '丹麦', p: 68 },
                '.fi': { f: '🇫🇮', r: '芬兰', p: 60 },
                '.pl': { f: '🇵🇱', r: '波兰', p: 54 },
                '.cz': { f: '🇨🇿', r: '捷克', p: 57 },
                '.at': { f: '🇦🇹', r: '奥地利', p: 56 },
                '.be': { f: '🇧🇪', r: '比利时', p: 55 },
                '.gr': { f: '🇬🇷', r: '希腊', p: 66 },
                '.pt': { f: '🇵🇹', r: '葡萄牙', p: 63 },
                '.ro': { f: '🇷🇴', r: '罗马尼亚', p: 59 },
                '.tr': { f: '🇹🇷', r: '土耳其', p: 35 },
                '.ae': { f: '🇦🇪', r: '阿联酋', p: 33 },
                '.il': { f: '🇮🇱', r: '以色列', p: 64 },
                '.za': { f: '🇿🇦', r: '南非', p: 62 },
                '.nz': { f: '🇳🇿', r: '新西兰', p: 58 },
                '.ar': { f: '🇦🇷', r: '阿根廷', p: 38 },
                '.cl': { f: '🇨🇱', r: '智利', p: 42 },
                '.co': { f: '🇨🇴', r: '哥伦比亚', p: 39 },
                '.mx': { f: '🇲🇽', r: '墨西哥', p: 44 },
                '.pe': { f: '🇵🇪', r: '秘鲁', p: 45 },
                '.ec': { f: '🇪🇨', r: '厄瓜多尔', p: 48 },
                '.cr': { f: '🇨🇷', r: '哥斯达黎加', p: 46 },
                '.gt': { f: '🇬🇹', r: '危地马拉', p: 47 },
                '.bo': { f: '🇧🇴', r: '玻利维亚', p: 49 },
                '.ma': { f: '🇲🇦', r: '摩洛哥', p: 112 },
                '.ng': { f: '🇳🇬', r: '尼日利亚', p: 111 },
                '.ke': { f: '🇰🇪', r: '肯尼亚', p: 110 },
                '.th': { f: '🇹🇭', r: '泰国', p: 22 },
                '.pk': { f: '🇵🇰', r: '巴基斯坦', p: 36 }
            };

            // 检查域名扩展名
            for (const [ext, region] of Object.entries(domainRegionMap)) {
                if (lowerServer.endsWith(ext)) {
                    return region;
                }
            }

            return null;
        };

        // 🚫 v3.6.1: 过滤丑陋/技术性的主机名 - 防止暴露服务器信息
        const isUglyHostname = (name) => {
            if (!name || typeof name !== 'string') return false;
            const lowerName = name.toLowerCase();

            // 丑陋主机名模式列表
            const uglyPatterns = [
                /localhost/i,                    // localhost
                /\.local$/i,                     // *.local
                /^ip-\d+/i,                      // ip-172-31-34-157
                /droplet-\d+/i,                  // droplet-329
                /^lxc/i,                         // LXCNAME, lxc*
                /\.rev\./i,                      // *.rev.* (reverse DNS)
                /slashdevslashnetslashtun/i,    // Sub-Store tunnel domains
                /^\d{1,3}-\d{1,3}-\d{1,3}-\d{1,3}/i,  // 113-29-232-28
                /^[a-z0-9]{8,}$/i,              // Random hashes (dmitebv2, 5522356392hax)
                /^[a-f0-9]{8,}$/i,              // Hex hashes
                /\.aptransit\./i,               // *.aptransit.*
                /^fif.*ser\d+/i,                // fifctser578050009652
                /\.slashdev/i,                  // *.slashdev*
                /^vm-/i,                        // vm-* 
                /^vps-/i,                       // vps-*
                /^server\d+/i,                  // server01, server123
                /^node\d+/i,                    // node01, node123
                /^host\d+/i,                    // host01, host123
                /^[0-9a-f]{8}-[0-9a-f]{4}/i,   // UUID patterns
                /^instance-/i,                  // instance-*
                /^compute-/i,                   // compute-*
                /\.compute\./i,                 // *.compute.*
                /\.amazonaws\./i,               // AWS hostnames
                /\.googleusercontent\./i,       // Google Cloud hostnames
                /\.azure/i,                     // Azure hostnames
                /\.digitalocean/i,              // DigitalOcean hostnames
                /\.vultr/i,                     // Vultr hostnames
                /\.linode/i                     // Linode hostnames
            ];

            return uglyPatterns.some(pattern => pattern.test(lowerName));
        };



        // 🎭 v3.6.1: 智能指纹随机化 - 根据节点地区分配合适的TLS指纹
        const fingerprintCache = new Map();
        const getSmartFingerprint = (regionName, nodeId) => {
            const tlsBoost = cfg.boostOptions && cfg.boostOptions.tlsBoost;
            if (!tlsBoost || !tlsBoost.enableSmartFingerprint) {
                return tlsBoost?.fingerprintType || 'chrome';
            }
            const cacheKey = `${regionName}_${nodeId}`;
            if (fingerprintCache.has(cacheKey)) {
                return fingerprintCache.get(cacheKey);
            }
            const regionalFp = tlsBoost.regionalFingerprints || {};
            let fpPool = regionalFp[regionName]?.length > 0 ? regionalFp[regionName]
                : regionalFp['default']?.length > 0 ? regionalFp['default']
                    : ['chrome', 'safari', 'firefox', 'edge'];
            const selectedFp = getRandItem(fpPool);
            fingerprintCache.set(cacheKey, selectedFp);
            return selectedFp;
        };

        // 🛡️ 增强过滤检查（更完善的防御性检查）
        const checkAndFilter = (proxy) => {
            if (!cfg.filterMode) return false;

            // 防御性检查：确保 proxy 对象有效
            if (!proxy || typeof proxy !== 'object') return true;
            if (!proxy.type || typeof proxy.type !== 'string') return true;

            const protocolType = proxy.type.toLowerCase();
            if (!cfg.protocols.includes(protocolType)) return true;
            if (protocolType === 'trojan' && proxy.tls === false) return true;

            // 端口验证增强
            const port = parseInt(proxy.port, 10);
            if (!proxy.server || typeof proxy.server !== 'string') return true;
            if (isNaN(port) || port <= 0 || port > 65535) return true;

            const serverHost = proxy.server.toLowerCase().trim();
            // 增强的无效服务器检测
            if (serverHost.includes('example.com') ||
                serverHost.includes('test.com') ||
                serverHost === '127.0.0.1' ||
                serverHost === 'localhost' ||
                serverHost === '0.0.0.0' ||
                serverHost.startsWith('192.168.') ||
                serverHost.startsWith('10.') ||
                serverHost.startsWith('172.16.') ||
                serverHost.endsWith('.local')) return true;

            // 协议专属验证
            if ((protocolType === 'vmess' || protocolType === 'vless') && !proxy.uuid) return true;
            if (protocolType === 'trojan' && !proxy.password) return true;
            if (protocolType === 'wireguard' && (!proxy.privateKey && !proxy['private-key'])) return true;

            const nodeName = (proxy.name || '').toLowerCase();
            return BLOCK_REGEX.test(nodeName) || BLOCK_REGEX_EN.test(nodeName);
        };

        // 🛡️ v3.5.5: Reality 节点检测函数（支持多种格式）
        const isRealityNode = (proxy) => !!(
            // Clash Meta 格式
            proxy['reality-opts'] ||
            proxy['reality-ops'] ||
            proxy['reality-public-key'] ||
            proxy['public-key'] ||
            // Sing-box 格式
            proxy['pbk'] ||
            proxy['sid'] ||
            // Shadowrocket 格式（驼峰命名）
            proxy['publicKey'] ||
            proxy['shortId'] ||
            // 通用检测
            (proxy.tls && proxy['reality']) ||
            (proxy['server-name'] && proxy['public-key']) ||
            (proxy['peer'] && proxy['publicKey'])  // Shadowrocket Reality
        );

        // 🛡️ XTLS Flow 检测函数
        const hasXtlsFlow = (proxy) => !!(
            proxy.flow && (
                proxy.flow.includes('xtls') ||
                proxy.flow.includes('vision') ||
                proxy.flow.includes('splice') ||
                proxy.flow.includes('direct') ||
                proxy.flow.includes('origin')
            )
        );

        // 🛡️ ECH 支持检测函数
        const hasEchSupport = (proxy) => !!(
            proxy.ech ||
            proxy['ech-config'] ||
            proxy['tls-ech'] ||
            (proxy.tls && proxy['encrypted-client-hello'])
        );

        const applyTlsConfig = (proxy, regionName) => {
            if (!cfg.forceTls) return;

            // 🛡️ Reality 节点 7 层检测保护（完全不修改 Reality 节点）
            if (isRealityNode(proxy)) {
                proxy['_skip_reason'] = 'reality_node';
                return;
            }

            // XTLS Flow 节点跳过 TLS 修改
            if (hasXtlsFlow(proxy)) {
                proxy['_skip_reason'] = 'xtls_flow_node';
                return;
            }

            // 🛡️ v3.5.3: 端口白名单模式 - 只有白名单端口才启用TLS
            const port = parseInt(proxy.port) || 443;
            const inTlsWhitelist = TLS_WHITELIST_PORTS.has(port);
            const inNonTlsBlacklist = NON_TLS_PORTS.has(port);

            // ⚠️ 白名单策略：只有白名单端口才启用TLS
            // 黑名单端口和其他端口(16055/16056等)都不启用
            if (!inTlsWhitelist || inNonTlsBlacklist) {
                return; // 静默跳过，减少日志噪音
            }

            // 启用 TLS（仅白名单端口）
            proxy.tls = true;

            // 🔒 TLS 1.3 exclusive（Chrome 131 标准）
            const tlsBoost = cfg.enableBoost && cfg.boostOptions.tlsBoost;
            proxy['tls-min-version'] = tlsBoost?.tlsMinVersion || '1.3';
            proxy['tls-max-version'] = tlsBoost?.tlsMaxVersion || '1.3';

            // 🔒 skip-cert-verify: 智能判断
            // 强制开启的 TLS（原节点无 TLS）→ 允许不安全（便于测试）
            // 这里是 applyTlsConfig，说明是强制开启的场景
            proxy['skip-cert-verify'] = true;

            // 🌐 智能 SNI 配置（根据地区选择 CDN）
            // 仅在原节点无 SNI 或强制覆盖时设置
            if (cfg.forceSniOverride || !proxy.sni) {
                proxy.sni = regionName ? getSmartSni(regionName) : getRandomSni();
            }
        };

        // 🛡️ 智能 TLS 配置：保护原有设置，仅增强缺失项
        const applySmartTlsEnhancement = (proxy, regionName) => {
            // 如果节点已有 TLS 设置，只增强缺失的配置项
            if (!proxy.tls) return;

            const tlsBoost = cfg.enableBoost && cfg.boostOptions.tlsBoost;
            if (!tlsBoost) return;

            // 🛡️ Reality/XTLS 节点：只添加曲线配置，跳过其他修改
            const isReality = isRealityNode(proxy);
            const hasXtls = hasXtlsFlow(proxy);

            // 🔧 曲线配置：Chrome 131 椭圆曲线偏好（所有 TLS 节点都添加，包括 Reality）
            // Chrome 131 使用的曲线顺序：X25519 > secp256r1 > secp384r1
            if (tlsBoost.curves) {
                // Clash Meta / Mihomo 格式 (使用冒号分隔)
                proxy['ecdh-curves'] = tlsBoost.curves.join(':');

                // 🎵 Sing-box 格式：curve_preferences 数组
                // 参考: https://sing-box.sagernet.org/configuration/shared/tls/
                // 使用小写格式：x25519, secp256r1, secp384r1
                proxy['curve_preferences'] = ['x25519', 'secp256r1', 'secp384r1'];

                // 🎵 Sing-box uTLS 指纹配置（Reality 节点使用 chrome 指纹）
                // 参考: https://sing-box.sagernet.org/configuration/shared/tls/#utls
                proxy['_utls'] = {
                    enabled: true,
                    fingerprint: 'chrome'  // Chrome 131 指纹
                };
            }

            // 🛡️ Reality/XTLS 节点：曲线配置已添加，跳过其他修改
            if (isReality || hasXtls) return;

            // TLS 版本：仅在未设置时添加
            if (!proxy['tls-min-version']) {
                proxy['tls-min-version'] = tlsBoost.tlsMinVersion || '1.3';
            }
            if (!proxy['tls-max-version']) {
                proxy['tls-max-version'] = tlsBoost.tlsMaxVersion || '1.3';
            }

            // 🔒 skip-cert-verify: 智能判断
            // 1. 如果节点有证书配置（ca/ca-str），则验证证书
            // 2. 如果节点有 SNI 且是知名域名，则验证证书
            // 3. 其他情况（自签证书、无证书配置），允许不安全
            const hasCertConfig = proxy.ca || proxy['ca-str'] || proxy['ca_str'];
            const hasKnownSni = proxy.sni && /\.(com|net|org|io|co|gov|edu)$/i.test(proxy.sni);

            if (hasCertConfig) {
                // 有证书配置，验证证书
                proxy['skip-cert-verify'] = false;
            } else if (hasKnownSni && !proxy['skip-cert-verify']) {
                // 有知名域名 SNI 且原节点未明确设置，保持原设置或默认验证
                // 但如果原节点明确设置了 false，说明机场要求验证，尊重原设置
                proxy['skip-cert-verify'] = proxy['skip-cert-verify'] ?? false;
            } else {
                // 无证书配置、无知名 SNI，或原节点已设置为 true
                // 默认允许不安全（机场常用自签证书）
                proxy['skip-cert-verify'] = true;
            }

            // ALPN：仅在未设置时添加（数组格式，Clash Meta/Shadowrocket 通用）
            if (tlsBoost.enableAlpn && !proxy.alpn) {
                proxy.alpn = ['h2', 'http/1.1'];
            }
            // 确保 alpn 是数组格式
            if (proxy.alpn && !Array.isArray(proxy.alpn)) {
                proxy.alpn = [proxy.alpn];
            }

            // 客户端指纹
            if (tlsBoost.enableClientFingerprint && !proxy['client-fingerprint']) {
                proxy['client-fingerprint'] = tlsBoost.fingerprintType || 'chrome';
            }

            // SNI：仅在未设置时添加
            if (!proxy.sni && !proxy.flow) {
                proxy.sni = regionName ? getSmartSni(regionName) : getRandomSni();
            }

            // 🆕 v3.5.6: TLS Fragment 分片（绕过DPI检测）- 修复版
            // 使用Sub-Store官方支持的参数格式
            // 参考: substore/Sub-Store-master/backend/src/core/proxy-utils/producers/sing-box.js
            // tlsParser函数中：
            //   if (proxy['_fragment']) parsedProxy.tls.fragment = !!proxy['_fragment'];
            //   if (proxy['_fragment_fallback_delay']) parsedProxy.tls.fragment_fallback_delay = proxy['_fragment_fallback_delay'];
            //   if (proxy['_record_fragment']) parsedProxy.tls.record_fragment = !!proxy['_record_fragment'];
            if (tlsBoost.enableTlsFragment) {
                // 🔧 v3.5.6修复：确保TLS已启用，分片才有意义
                if (proxy.tls === true) {
                    const fragOpts = tlsBoost.tlsFragmentOptions || {};
                    const fragInterval = fragOpts.interval || '10-20';

                    // ✅ Sub-Store sing-box producer 官方支持的参数（布尔值）
                    proxy['_fragment'] = true;  // 启用TLS分片 -> tls.fragment = true
                    proxy['_fragment_fallback_delay'] = fragInterval;  // 分片间隔 -> tls.fragment_fallback_delay
                    proxy['_record_fragment'] = true;  // 记录分片 -> tls.record_fragment = true

                    // ✅ Clash Meta / Mihomo 格式（备用）
                    proxy['client-fingerprint'] = proxy['client-fingerprint'] || 'chrome';
                    // TLS分片已启用（仅sing-box生效）
                }
            }

            // 🆕 v3.5.7: HTTP/2 增强 - Clash Meta/Shadowrocket 通用格式
            if (tlsBoost.enableHttp2 && proxy.tls) {
                // ✅ ALPN配置（数组格式，确保h2在前）
                if (!proxy.alpn) {
                    proxy.alpn = ['h2', 'http/1.1'];
                } else if (Array.isArray(proxy.alpn) && !proxy.alpn.includes('h2')) {
                    proxy.alpn.unshift('h2');
                }
                // 确保 alpn 是数组格式
                if (proxy.alpn && !Array.isArray(proxy.alpn)) {
                    proxy.alpn = [proxy.alpn];
                }

                // ✅ Clash Meta HTTP/2 传输配置（仅当 network=h2 时）
                if (proxy.network === 'h2') {
                    proxy['h2-opts'] = proxy['h2-opts'] || {
                        host: [proxy.sni || proxy.server],
                        path: '/'
                    };
                }
            }
        };

        const applyWsObfsConfig = (proxy) => {
            if (!cfg.forceWsObfs) return;
            proxy.network = 'ws';
            if (cfg.forceObfsOverride || _.get(proxy, 'ws-opts.headers.Host')) {
                _.set(proxy, 'ws-opts.headers.Host', getRandomObfs());
            }
        };

        const optimizeProxy = (proxy) => {
            const protocolType = proxy.type.toLowerCase();
            const modifiedProxy = { ...proxy };

            // ============================================================
            // 通用 Boost 选项（适用于多数协议）
            // ============================================================

            if (cfg.enableBoost) {
                // ECN (显式拥塞通知) - 提升拥堵网络性能
                modifiedProxy['ecn'] = true;  // 启用 ECN

                // IPv6 偏好设置
                if (cfg.forceIPv6) modifiedProxy['ip-version'] = 'prefer-v6';

                // TCP Fast Open (TFO) - 减少首次连接延迟
                if (cfg.boostOptions.enableTcpFastOpen) {
                    modifiedProxy['tfo'] = true;         // Surge 使用 tfo
                    modifiedProxy['fast-open'] = true;   // Shadowrocket 使用 fast-open
                }

                // 启用 UDP 转发 - 支持全流量代理（游戏/DNS等）
                if (cfg.boostOptions.enableUdp) {
                    modifiedProxy['udp'] = true;
                    // 🚀 Shadowrocket: 使用 packet-addr 模式加速 UDP
                    modifiedProxy['udp-relay'] = true;
                }
            } else {
                // 如果禁用 boost，仍保留原有的基本设置
                modifiedProxy['ecn'] = true;  // 默认启用 ECN
                if (cfg.forceIPv6) modifiedProxy['ip-version'] = 'prefer-v6';
                modifiedProxy['tfo'] = true;        // Surge
                modifiedProxy['fast-open'] = true;  // Shadowrocket
            }

            // ============================================================
            // 协议专属优化
            // ============================================================

            switch (protocolType) {
                case 'vless':
                    // VLESS 优化配置
                    // 🛡️ Reality 节点保护（使用提取的检测函数）
                    if (isRealityNode(modifiedProxy)) {
                        // 🔧 Reality节点：只添加曲线配置和Chrome指纹，不修改其他设置
                        const tlsBoost = cfg.enableBoost && cfg.boostOptions.tlsBoost;
                        if (tlsBoost) {
                            // 🎭 智能指纹随机化 - utls自动包含曲线配置
                            const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                            const nodeId = modifiedProxy.server + ':' + modifiedProxy.port;
                            const smartFp = getSmartFingerprint(regionInfo.r, nodeId);
                            modifiedProxy['client-fingerprint'] = smartFp;
                            // Clash Meta格式
                            if (tlsBoost.curves) modifiedProxy['ecdh-curves'] = tlsBoost.curves.join(':');
                        }
                        modifiedProxy['_skip_reason'] = 'reality_vless';
                        break;
                    }

                    // 🆕 v3.5.2: 端口白名单模式
                    const vlessPort = parseInt(modifiedProxy.port) || 443;
                    const vlessInTlsWhitelist = TLS_WHITELIST_PORTS.has(vlessPort);
                    const vlessInNonTlsBlacklist = NON_TLS_PORTS.has(vlessPort);

                    // 🛡️ 白名单策略：只有白名单端口才强制启用TLS
                    if (cfg.forceTls && !modifiedProxy.tls && vlessInTlsWhitelist) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                        applyTlsConfig(modifiedProxy, regionInfo.r);
                    } else if (vlessInNonTlsBlacklist && modifiedProxy.tls) {
                        // 黑名单端口，禁用TLS
                        modifiedProxy.tls = false;
                        delete modifiedProxy['skip-cert-verify'];
                        delete modifiedProxy['tls-min-version'];
                        delete modifiedProxy['tls-max-version'];
                        delete modifiedProxy['client-fingerprint'];
                        delete modifiedProxy.alpn;
                        delete modifiedProxy.sni;
                    }
                    // ⚠️ 其他端口：完全不修改TLS设置

                    // TLS 增强 - 仅在节点原本就有TLS时
                    if (cfg.enableBoost && modifiedProxy.tls && !vlessInNonTlsBlacklist) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                        applySmartTlsEnhancement(modifiedProxy, regionInfo.r);
                    }

                    // XTLS Flow 优化（仅升级已知不安全的旧版）
                    if (modifiedProxy.flow === 'xtls-rprx-direct') {
                        modifiedProxy.flow = 'xtls-rprx-vision';
                    }

                    // 🔧 v3.5.7修复：UDP数据包编码优化
                    // Clash Meta: 使用 packet-encoding 参数
                    // sing-box: 使用 xudp 字段（producer会转换为 packet_encoding）
                    if (cfg.enableBoost && cfg.boostOptions.transportBoost.enableXudp) {
                        // Clash Meta 格式
                        modifiedProxy['packet-encoding'] = 'packetaddr';
                        // sing-box 格式（通过 xudp 字段触发）
                        modifiedProxy.xudp = true;
                    }

                    applyWsObfsConfig(modifiedProxy);

                    // gRPC 传输优化
                    if (cfg.enableBoost && cfg.boostOptions.transportBoost.enableGrpcOptimization &&
                        modifiedProxy.network === 'grpc' && !modifiedProxy['grpc-opts']) {
                        modifiedProxy['grpc-opts'] = { 'grpc-service-name': 'GunService' };
                    }

                    // 🚀 多路复用（不与 XTLS flow 同时使用）
                    if (cfg.enableBoost && cfg.boostOptions.enableMux && !modifiedProxy.flow && !modifiedProxy.smux) {
                        modifiedProxy.smux = {
                            enabled: true, protocol: 'smux',
                            'max-connections': 4, 'min-streams': 4, 'max-streams': 0,
                            padding: true, stateless: false
                        };
                        modifiedProxy.mux = true;
                    }

                    // 🆕 v3.5.4: 最终TLS保护 - Reality节点或非白名单端口
                    // Reality节点不应该被修改TLS设置
                    if (isRealityNode(modifiedProxy)) {
                        // Reality节点保持原设置
                    } else if (!vlessInTlsWhitelist && !proxy.tls) {
                        modifiedProxy.tls = false;
                    }
                    break;

                case 'trojan':
                    // Trojan 优化配置
                    // 🆕 v3.5.2: 端口白名单模式
                    const trojanPort = parseInt(modifiedProxy.port) || 443;
                    const trojanInTlsWhitelist = TLS_WHITELIST_PORTS.has(trojanPort);
                    const trojanInNonTlsBlacklist = NON_TLS_PORTS.has(trojanPort);

                    // 🛡️ Trojan必须使用TLS，但采用白名单策略
                    if (modifiedProxy.tls === undefined) {
                        if (trojanInNonTlsBlacklist) {
                            // 黑名单端口，Trojan需要TLS，可能无法工作
                        } else if (trojanInTlsWhitelist) {
                            // 白名单端口，启用TLS
                            modifiedProxy.tls = true;
                        }
                        // 其他端口：保持原设置（不强制启用TLS）
                    }

                    // TLS 增强 - 仅在节点原本就有TLS时
                    if (cfg.enableBoost && modifiedProxy.tls && !trojanInNonTlsBlacklist) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                        applySmartTlsEnhancement(modifiedProxy, regionInfo.r);
                    }

                    // XTLS Flow（仅升级非 vision 版本）
                    if (modifiedProxy.flow && !modifiedProxy.flow.includes('vision')) {
                        modifiedProxy.flow = 'xtls-rprx-vision';
                    }

                    applyWsObfsConfig(modifiedProxy);

                    if (cfg.enableBoost && cfg.boostOptions.transportBoost.enableGrpcOptimization &&
                        modifiedProxy.network === 'grpc' && !modifiedProxy['grpc-opts']) {
                        modifiedProxy['grpc-opts'] = { 'grpc-service-name': 'TrojanService' };
                    }

                    // 多路复用（不与 XTLS flow 同时使用）
                    if (cfg.enableBoost && cfg.boostOptions.enableMux && !modifiedProxy.flow && !modifiedProxy.smux) {
                        modifiedProxy.smux = {
                            enabled: true, protocol: 'smux',
                            'max-connections': 4, 'min-streams': 4, 'max-streams': 0,
                            padding: true, stateless: false
                        };
                        modifiedProxy.mux = true;
                    }
                    break;

                case 'vmess':
                    // VMess 优化配置
                    // 🆕 v3.5.2: 端口白名单模式 - 只有白名单端口才强制启用TLS
                    const vmessPort = parseInt(modifiedProxy.port) || 443;
                    // ✅ 白名单模式：只有这些端口才强制启用TLS
                    const vmessInTlsWhitelist = TLS_WHITELIST_PORTS.has(vmessPort);
                    // 🚫 黑名单：这些端口明确不支持TLS
                    const vmessInNonTlsBlacklist = NON_TLS_PORTS.has(vmessPort);

                    // 🛡️ 白名单策略（最保守）：
                    // 1. 白名单端口(443/8443等) + forceTls + 原节点无TLS → 启用TLS
                    // 2. 黑名单端口(80/8080等) + 原节点有TLS → 禁用TLS
                    // 3. 其他所有端口(12800/16056/19203等) → 完全保持原设置
                    if (cfg.forceTls && !modifiedProxy.tls && vmessInTlsWhitelist) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                        applyTlsConfig(modifiedProxy, regionInfo.r);
                    } else if (vmessInNonTlsBlacklist && modifiedProxy.tls) {
                        // 黑名单端口，禁用TLS
                        modifiedProxy.tls = false;
                        delete modifiedProxy['skip-cert-verify'];
                        delete modifiedProxy['tls-min-version'];
                        delete modifiedProxy['tls-max-version'];
                        delete modifiedProxy['client-fingerprint'];
                        delete modifiedProxy.alpn;
                        delete modifiedProxy.sni;
                    }
                    // ⚠️ 其他端口(12800/16056/19203等)：完全不修改TLS设置

                    // TLS 增强选项 - 仅在节点原本就有TLS时增强，不强制添加
                    if (cfg.enableBoost && modifiedProxy.tls && !vmessInNonTlsBlacklist) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                        applySmartTlsEnhancement(modifiedProxy, regionInfo.r);
                    }

                    // AEAD 加密模式（alterId = 0）
                    modifiedProxy['alter-id'] = cfg.enableBoost && cfg.boostOptions.protocolSpecific.vmess.forceAead
                        ? 0
                        : (modifiedProxy['alter-id'] ?? 0);

                    // 智能加密方法选择（ECH 感知 + 替换 auto）
                    if (cfg.enableBoost) {
                        const hasECH = hasEchSupport(modifiedProxy);
                        const vmessConfig = cfg.boostOptions.protocolSpecific.vmess;
                        const preferredCipher = hasECH ? 'chacha20-poly1305' : (vmessConfig.defaultCipher || 'aes-128-gcm');

                        // 🔧 修复 security: auto - 替换为具体加密方法
                        // auto 会导致某些客户端选择不安全的加密方法
                        if (!modifiedProxy.cipher || modifiedProxy.cipher === 'auto' || modifiedProxy.cipher === 'none') {
                            modifiedProxy.cipher = preferredCipher;
                            modifiedProxy['_cipher_reason'] = 'auto_replaced_with_' + preferredCipher;
                        }
                        if (modifiedProxy.security === 'auto' || modifiedProxy.security === 'none') {
                            modifiedProxy.security = preferredCipher;
                            modifiedProxy['_cipher_reason'] = 'security_auto_replaced';
                        }

                        // 非 ECH 场景：替换 ChaCha20 为 AES-GCM（更好的硬件加速）
                        if (!hasECH) {
                            if (modifiedProxy.cipher?.includes('chacha20')) {
                                modifiedProxy.cipher = vmessConfig.defaultCipher || 'aes-128-gcm';
                                modifiedProxy['_cipher_reason'] = 'chacha20_replaced_no_ech';
                            }
                            if (modifiedProxy.security?.includes('chacha20')) {
                                modifiedProxy.security = vmessConfig.defaultCipher || 'aes-128-gcm';
                                modifiedProxy['_cipher_reason'] = 'chacha20_replaced_no_ech';
                            }
                        }

                        // 🔧 v3.5.7修复：UDP数据包编码优化
                        // Clash Meta: 使用 packet-encoding 参数
                        // sing-box: 使用 xudp 字段
                        if (cfg.boostOptions.transportBoost.enableXudp) {
                            modifiedProxy['packet-encoding'] = 'packetaddr';
                            modifiedProxy.xudp = true;
                        }
                    }

                    applyWsObfsConfig(modifiedProxy);

                    if (cfg.enableBoost && cfg.boostOptions.transportBoost.enableGrpcOptimization &&
                        modifiedProxy.network === 'grpc' && !modifiedProxy['grpc-opts']) {
                        modifiedProxy['grpc-opts'] = { 'grpc-service-name': 'GunService' };
                    }

                    // 多路复用
                    if (cfg.enableBoost && cfg.boostOptions.enableMux && !modifiedProxy.smux) {
                        modifiedProxy.smux = {
                            enabled: true, protocol: 'smux',
                            'max-connections': 4, 'min-streams': 4, 'max-streams': 0,
                            padding: true, stateless: false
                        };
                        modifiedProxy.mux = true;
                    }

                    // 🆕 v3.5.4: 最终TLS保护 - 确保非白名单端口不会被意外启用TLS
                    // 如果原节点没有TLS且端口不在白名单中，强制确保TLS为false
                    if (!vmessInTlsWhitelist && !proxy.tls) {
                        modifiedProxy.tls = false;
                    }
                    break;

                case 'ss':
                case 'shadowsocks':
                    // Shadowsocks 优化配置

                    if (cfg.enableBoost) {
                        // 🔒 AEAD 加密方法（仅使用 AES-GCM）
                        if (!modifiedProxy.cipher) {
                            modifiedProxy.cipher = cfg.boostOptions.protocolSpecific.shadowsocks.defaultCipher || 'aes-128-gcm';
                        }
                        // 如果已指定但是 chacha20，切换到 AES-GCM
                        else if (modifiedProxy.cipher && modifiedProxy.cipher.includes('chacha20')) {
                            modifiedProxy.cipher = cfg.boostOptions.protocolSpecific.shadowsocks.defaultCipher || 'aes-128-gcm';
                        }

                        // UDP over TCP - 提升 UDP 可靠性
                        if (cfg.boostOptions.protocolSpecific.shadowsocks.enableUdpOverTcp &&
                            !modifiedProxy['udp-over-tcp'] && !modifiedProxy.uot) {
                            modifiedProxy['udp-over-tcp'] = true;
                        }
                    }

                    // 插件配置保留（如果已配置）
                    // 不自动添加插件，避免破坏现有配置

                    // 🚀 多路复用（smux）- Shadowsocks 支持多路复用
                    // 注意：需要服务器端支持 simple-obfs 或 v2ray-plugin 的 mux 功能
                    if (cfg.enableBoost && cfg.boostOptions.enableMux) {
                        // 仅在使用 v2ray-plugin 时启用多路复用
                        if (modifiedProxy.plugin === 'v2ray-plugin' || modifiedProxy.plugin === 'obfs') {
                            if (!modifiedProxy.smux) {
                                modifiedProxy.smux = {
                                    enabled: true,
                                    protocol: 'smux',
                                    'max-connections': 4,
                                    'min-streams': 4,
                                    'max-streams': 0,
                                    padding: true,
                                    stateless: false
                                };
                            }
                            modifiedProxy.mux = true;
                        }
                    }

                    break;

                case 'hysteria2':
                    // Hysteria2 优化配置（QUIC 原生协议 - 完全保护 UDP/QUIC）
                    // 🛡️ Hysteria2 节点保护：最小化修改，保留原有配置

                    // TLS 基本配置（Hysteria2 必须启用 TLS）
                    if (modifiedProxy.tls === undefined) {
                        modifiedProxy.tls = true;
                    }

                    if (cfg.enableBoost) {
                        // 🔒 TLS 1.3 配置（Hysteria2 需要 TLS 1.3）
                        // 仅在未设置时添加
                        if (!modifiedProxy['tls-min-version']) {
                            modifiedProxy['tls-min-version'] = '1.3';
                        }
                        if (!modifiedProxy['tls-max-version']) {
                            modifiedProxy['tls-max-version'] = '1.3';
                        }

                        // 🔒 skip-cert-verify: 智能判断
                        // Hysteria2 机场常用自签证书，但也要尊重有证书配置的节点
                        const hy2HasCert = modifiedProxy.ca || modifiedProxy['ca-str'];
                        if (hy2HasCert) {
                            // 有证书配置，验证证书
                            modifiedProxy['skip-cert-verify'] = false;
                        } else {
                            // 无证书配置，允许不安全（机场常用自签证书）
                            modifiedProxy['skip-cert-verify'] = true;
                        }

                        // 🚀 TLS 指纹伪装 - 仅在未设置时添加
                        if (cfg.boostOptions.tlsBoost.enableClientFingerprint && !modifiedProxy['client-fingerprint']) {
                            modifiedProxy['client-fingerprint'] = cfg.boostOptions.tlsBoost.fingerprintType || 'chrome';
                        }

                        // 🚀 ALPN 协议协商 - Hysteria2 专用 HTTP/3
                        // 仅在未设置时添加
                        if (cfg.boostOptions.tlsBoost.enableAlpn && !modifiedProxy.alpn) {
                            modifiedProxy.alpn = ['h3'];  // HTTP/3 over QUIC
                        }

                        // 带宽设置（只有当配置值不为空时才设置，否则让节点自行协商）
                        if (!modifiedProxy.up && cfg.boostOptions.protocolSpecific.hysteria2.defaultUpBandwidth) {
                            modifiedProxy.up = cfg.boostOptions.protocolSpecific.hysteria2.defaultUpBandwidth;
                        }
                        if (!modifiedProxy.down && cfg.boostOptions.protocolSpecific.hysteria2.defaultDownBandwidth) {
                            modifiedProxy.down = cfg.boostOptions.protocolSpecific.hysteria2.defaultDownBandwidth;
                        }

                        // MTU 发现（优化包大小）
                        if (cfg.boostOptions.protocolSpecific.hysteria2.enableMtuDiscovery &&
                            modifiedProxy['disable-mtu-discovery'] === undefined) {
                            modifiedProxy['disable-mtu-discovery'] = false;
                        }

                        // 混淆配置（如果已配置密码，启用 salamander）
                        if (modifiedProxy['obfs-password'] && !modifiedProxy.obfs) {
                            modifiedProxy.obfs = 'salamander';
                        }

                        // ⚠️ v3.5.7: Hysteria2 心跳说明
                        // Clash Meta 的 Hysteria2 不支持 heartbeat-interval 参数
                        // 这是 TUIC 协议的参数，Hysteria2 使用 QUIC 内置的心跳机制
                        // 如果需要心跳，请使用 TUIC 协议
                    }

                    // 🛡️ QUIC 原生协议保护：确保不被 QUIC 屏蔽影响
                    // Hysteria2 依赖 UDP/QUIC，绝不能屏蔽
                    if (modifiedProxy['block-quic']) {
                        delete modifiedProxy['block-quic'];  // 移除 QUIC 屏蔽
                    }
                    if (modifiedProxy['udp'] === false) {
                        modifiedProxy['udp'] = true;  // 强制启用 UDP
                    }

                    // 端口跳跃配置（如果已配置）
                    // 不自动添加，避免破坏配置

                    break;

                case 'tuic':
                    // TUIC 优化配置（QUIC 原生协议 - 完全保护 UDP/QUIC）
                    // 🛡️ TUIC 节点保护：最小化修改，保留原有配置

                    if (cfg.enableBoost) {
                        // 🔒 TLS 1.3 配置（TUIC 需要 TLS 1.3）
                        // 仅在未设置时添加
                        if (!modifiedProxy['tls-min-version']) {
                            modifiedProxy['tls-min-version'] = '1.3';
                        }
                        if (!modifiedProxy['tls-max-version']) {
                            modifiedProxy['tls-max-version'] = '1.3';
                        }

                        // 🔒 skip-cert-verify: 智能判断
                        // TUIC 机场常用自签证书，但也要尊重有证书配置的节点
                        const tuicHasCert = modifiedProxy.ca || modifiedProxy['ca-str'];
                        if (tuicHasCert) {
                            // 有证书配置，验证证书
                            modifiedProxy['skip-cert-verify'] = false;
                        } else {
                            // 无证书配置，允许不安全（机场常用自签证书）
                            modifiedProxy['skip-cert-verify'] = true;
                        }

                        // 🚀 TLS 指纹伪装 - Chrome 131
                        if (cfg.boostOptions.tlsBoost.enableClientFingerprint && !modifiedProxy['client-fingerprint']) {
                            modifiedProxy['client-fingerprint'] = cfg.boostOptions.tlsBoost.fingerprintType || 'chrome';
                        }

                        // 🚀 ALPN 协议协商 - TUIC 专用 HTTP/3
                        if (cfg.boostOptions.tlsBoost.enableAlpn && !modifiedProxy.alpn) {
                            modifiedProxy.alpn = ['h3'];  // HTTP/3 over QUIC
                        }

                        // 拥塞控制算法 - BBR 优化高延迟网络
                        if (!modifiedProxy['congestion-controller']) {
                            modifiedProxy['congestion-controller'] = cfg.boostOptions.protocolSpecific.tuic.congestionController;
                        }

                        // UDP 中继模式
                        if (modifiedProxy['udp-relay-mode'] === undefined) {
                            modifiedProxy['udp-relay-mode'] = cfg.boostOptions.protocolSpecific.tuic.udpRelayMode;
                        }

                        // 零往返时间 (0-RTT) - 使用 enableZeroRtt 配置
                        if (modifiedProxy['reduce-rtt'] === undefined && cfg.boostOptions.protocolSpecific.tuic.enableZeroRtt) {
                            modifiedProxy['reduce-rtt'] = true;
                        }
                    }

                    // 🛡️ QUIC 原生协议保护：确保不被 QUIC 屏蔽影响
                    // TUIC 依赖 UDP/QUIC，绝不能屏蔽
                    if (modifiedProxy['block-quic']) {
                        delete modifiedProxy['block-quic'];  // 移除 QUIC 屏蔽
                    }
                    if (modifiedProxy['udp'] === false) {
                        modifiedProxy['udp'] = true;  // 强制启用 UDP
                    }

                    break;

                case 'wireguard':
                    // WireGuard 优化配置（UDP 原生协议 - 保护 UDP）

                    if (cfg.enableBoost) {
                        // MTU 优化 - 减少碎片，提升性能
                        if (!modifiedProxy.mtu) {
                            modifiedProxy.mtu = cfg.boostOptions.protocolSpecific.wireguard.defaultMtu || 1420;
                        }

                        // 保留位（兼容性）
                        if (!modifiedProxy.reserved) {
                            modifiedProxy.reserved = [0, 0, 0];
                        }

                        // 持续连接（Keep Alive）
                        if (!modifiedProxy['persistent-keepalive'] && !modifiedProxy.keepalive) {
                            modifiedProxy['persistent-keepalive'] = 25;  // 25秒心跳
                        }
                    }

                    // 🛡️ UDP 原生协议保护：WireGuard 依赖 UDP
                    if (modifiedProxy['udp'] === false) {
                        modifiedProxy['udp'] = true;  // 强制启用 UDP
                    }

                    // IP 配置保留现有设置
                    // 不强制修改 IP 配置

                    break;

                case 'snell':
                    // Snell 优化配置

                    // 强制使用 Snell v5（最新版本）
                    if (!modifiedProxy.version || modifiedProxy.version < 5) {
                        modifiedProxy.version = 5;
                    }

                    // TCP Fast Open - 减少握手延迟
                    if (cfg.enableBoost && cfg.boostOptions.enableTcpFastOpen) {
                        modifiedProxy['tcp-fast-open'] = true;
                    }

                    // 混淆配置 - HTTP 模式
                    _.set(modifiedProxy, 'obfs-opts.mode', 'http');
                    if (cfg.forceObfsOverride || !_.get(modifiedProxy, 'obfs-opts.host')) {
                        _.set(modifiedProxy, 'obfs-opts.host', getRandomObfs());
                    }

                    // 重用连接（提升性能）
                    if (modifiedProxy['reuse'] === undefined) {
                        modifiedProxy['reuse'] = true;
                    }

                    break;

                case 'https':
                    // HTTPS 代理优化

                    // TLS 配置
                    if (!modifiedProxy.tls) {
                        modifiedProxy.tls = true;
                    }

                    // TLS 1.3 配置（如果启用 Boost）
                    if (cfg.enableBoost && modifiedProxy.tls) {
                        if (!modifiedProxy['tls-min-version']) {
                            modifiedProxy['tls-min-version'] = '1.3';
                        }
                        if (!modifiedProxy['tls-max-version']) {
                            modifiedProxy['tls-max-version'] = '1.3';
                        }

                        // skip-cert-verify（根据配置）
                        if (modifiedProxy['skip-cert-verify'] === undefined) {
                            modifiedProxy['skip-cert-verify'] = cfg.boostOptions.tlsBoost.skipCertVerify !== undefined
                                ? cfg.boostOptions.tlsBoost.skipCertVerify
                                : false;
                        }
                    }

                    // 智能 SNI 配置
                    if (!modifiedProxy.sni || cfg.forceSniOverride) {
                        const regionInfo = getRegionInfo(modifiedProxy._originalName || modifiedProxy.name || '');
                        modifiedProxy.sni = regionInfo.r ? getSmartSni(regionInfo.r) : modifiedProxy.server;
                    }

                    break;
            }

            // ============================================================
            // 🚫 QUIC 屏蔽增强（仅对非 QUIC 原生协议）
            // ============================================================
            // 使用预编译的 Set 进行 O(1) 查找
            const isQuicNative = QUIC_NATIVE_PROTOCOLS.has(protocolType);

            if (cfg.blockQuic && !isQuicNative) {
                modifiedProxy['block-quic'] = true;

                // 从 ALPN 中移除 HTTP/3（防止 QUIC 协商）
                if (modifiedProxy.alpn && Array.isArray(modifiedProxy.alpn)) {
                    modifiedProxy.alpn = modifiedProxy.alpn.filter(proto => proto !== 'h3');
                    if (modifiedProxy.alpn.length === 0) {
                        modifiedProxy.alpn = ['h2', 'http/1.1'];
                    }
                }
            }

            // ShadowTLS 扩展（适用于 VLESS/Trojan/VMess）
            if (cfg.shadowTlsEnabled && ['vless', 'trojan', 'vmess'].includes(protocolType) && !modifiedProxy['shadow-tls']) {
                modifiedProxy['shadow-tls'] = {
                    version: cfg.shadowTlsVersion,
                    servername: getRandomSni()
                };
            }

            // 🚫 QUIC 屏蔽功能（细化控制）
            if (cfg.blockQuic && cfg.quicBlockOptions.enableForAllNodes && !isQuicNative) {
                const blockMethod = cfg.quicBlockOptions.blockMethod;

                // 方法 1: 阻止 UDP（强制使用 TCP）
                if (blockMethod === 'block-udp' || blockMethod === 'both') {
                    modifiedProxy['udp'] = false;
                    if (protocolType === 'ss' || protocolType === 'shadowsocks') {
                        modifiedProxy['udp-over-tcp'] = true;
                    }
                }

                // 方法 2: 强制使用 TCP
                if (blockMethod === 'force-tcp' || blockMethod === 'both') {
                    if (cfg.quicBlockOptions.disableHttp3 && modifiedProxy.alpn) {
                        modifiedProxy.alpn = modifiedProxy.alpn.filter(proto => proto !== 'h3');
                        if (modifiedProxy.alpn.length === 0) modifiedProxy.alpn = ['h2', 'http/1.1'];
                    }
                    // 将 quic network 改为 tcp
                    if (['vless', 'vmess', 'trojan'].includes(protocolType) && modifiedProxy.network === 'quic') {
                        modifiedProxy.network = 'tcp';
                    }
                }
            }

            return modifiedProxy;
        };

        // 🚀 性能优化：使用 lodash memoize 缓存地区识别结果
        // 🚀 性能优化：使用预编译的 REGION_PATTERNS（O(1) 正则匹配，无运行时编译）
        const getRegionInfo = _.memoize((nodeName) => {
            if (!nodeName) return { f: '🌐', r: '其他', p: 999 };

            // 使用顶部预编译的 REGION_PATTERNS
            for (const [flag, info] of Object.entries(REGION_PATTERNS)) {
                if (info.r.test(nodeName)) {
                    let r = info.n;
                    // 中国省份识别优化
                    if (flag === '🇨🇳') {
                        const lowerName = nodeName.toLowerCase();
                        for (const [province, keywords] of Object.entries(PROVINCES)) {
                            if (keywords.some(k => lowerName.includes(k))) {
                                r = province;
                                break;
                            }
                        }
                    }
                    return { f: flag, r, p: info.p };
                }
            }
            return { f: '🌐', r: '其他', p: 999 };
        });

        // 🚀 性能优化：使用 lodash memoize 缓存特性识别结果
        const getFeatureType = _.memoize((nodeName) => {
            if (!nodeName) return 'd';
            const lowerName = nodeName.toLowerCase();
            for (const type of ['p', 'f', 's']) {
                if (FEATURE_REGEX[type].test(lowerName)) return type;
            }
            return 'd';
        });

        // 使用预编译的 Set（O(1) 查找）
        const removePortHoppingParams = (proxy) => {
            const cleanedProxy = { ...proxy };
            for (const param of PORT_HOPPING_PARAMS) {
                delete cleanedProxy[param];
            }
            return cleanedProxy;
        };

        // 使用预编译的 Set 进行特殊关键词检查
        const hasSpecialKeywordCheck = (name) => {
            if (!name) return false;
            const lowerName = name.toLowerCase();
            for (const keyword of SPECIAL_KEYWORDS_LOWER) {
                if (lowerName.includes(keyword)) return true;
            }
            return false;
        };

        // 🚀 性能优化：使用顶部预编译的 REGION_PATTERNS（避免运行时编译 60+ 个正则）
        // 注意：cfg.regions 仅用于用户自定义优先级，地区匹配使用预编译的 REGION_PATTERNS

        // 🚀 批量过滤无效节点
        const validProxies = proxies.filter(proxy => !checkAndFilter(proxy));
        if (validProxies.length === 0) return [];

        // 🚀 v3.6.0: 高效去重 - 使用Map缓存和批量处理
        const AUTH_KEY_EXTRACTORS = Object.freeze({
            vmess: p => p.uuid || '',
            vless: p => p.uuid || '',
            trojan: p => p.password || '',
            ss: p => `${p.cipher || ''}:${p.password || ''}`,
            shadowsocks: p => `${p.cipher || ''}:${p.password || ''}`,
            hysteria2: p => p.password || p.auth || p['auth-str'] || '',
            hysteria: p => p.password || p.auth || p['auth-str'] || '',
            tuic: p => `${p.uuid || ''}:${p.password || ''}`,
            wireguard: p => p.privateKey || p['private-key'] || '',
        });

        const getProxyUniqueKey = (proxy) => {
            const type = (proxy.type || '').toLowerCase();
            const extractor = AUTH_KEY_EXTRACTORS[type];
            const authKey = extractor ? extractor(proxy) : (proxy.password || proxy.uuid || '');
            return `${type}|${(proxy.server || '').toLowerCase()}|${proxy.port || 0}|${authKey}`;
        };

        // 高效去重：单次遍历
        const seenKeys = new Set();
        const dedupedProxies = [];
        let duplicateCount = 0;

        for (let i = 0, len = validProxies.length; i < len; i++) {
            const proxy = validProxies[i];
            const key = getProxyUniqueKey(proxy);
            if (!seenKeys.has(key)) {
                seenKeys.add(key);
                dedupedProxies.push(proxy);
            } else {
                duplicateCount++;
            }
        }

        if (duplicateCount > 0) {
            console.log(`[v3.6.0] 🔄 去重: -${duplicateCount} 重复, 剩余 ${dedupedProxies.length} 节点`);
        }

        const processedProxies = [];
        const regionCounters = new Map();

        const incrementCounter = (map, key) => {
            const val = (map.get(key) || 0) + 1;
            map.set(key, val);
            return val;
        };

        // 🎨 关键标签提取器 - 从原始名称中提取有价值的标签
        const extractKeyTags = (name) => {
            if (!name) return [];
            const tags = [];
            const upperName = name.toUpperCase();

            // 线路类型标签 (优先级最高)
            const lineTypes = {
                'IPLC': 'IPLC', 'IEPL': 'IEPL', 'CN2': 'CN2', 'GIA': 'GIA',
                'BGP': 'BGP', 'CMI': 'CMI', 'CU': 'CU', 'CT': 'CT', 'CM': 'CM',
                'CUVIP': 'CU', 'AS9929': '9929', 'AS4837': '4837',
                '专线': '专线', '精品': '精品', '原生': '原生'
            };
            for (const [key, tag] of Object.entries(lineTypes)) {
                if (upperName.includes(key.toUpperCase())) {
                    tags.push(tag);
                    break; // 只取一个线路类型
                }
            }

            // 用途标签
            const usageTags = {
                '流媒体': '📺', 'NETFLIX': '📺', 'NF': '📺', 'DISNEY': '📺',
                'EMBY': '📺', 'STREAMING': '📺', '解锁': '🔓',
                'CHATGPT': '🤖', 'GPT': '🤖', 'OPENAI': '🤖', 'AI': '🤖',
                'GAME': '🎮', '游戏': '🎮', 'GAMING': '🎮',
                'DOWNLOAD': '📥', '下载': '📥'
            };
            for (const [key, tag] of Object.entries(usageTags)) {
                if (upperName.includes(key)) {
                    if (!tags.includes(tag)) tags.push(tag);
                    break; // 只取一个用途标签
                }
            }

            // 倍率标签
            const rateMatch = name.match(/(\d+(?:\.\d+)?)\s*[xX×倍]/);
            if (rateMatch) {
                const rate = parseFloat(rateMatch[1]);
                if (rate !== 1) tags.push(`${rate}x`);
            }

            return tags.slice(0, 2); // 最多保留2个标签
        };

        // 🎨 美化节点名称生成函数
        const beautifyNodeName = (regionInfo, featType, count, originalName, hasSpecialKeyword) => {
            const { f: regionFlag, r: regionName } = regionInfo;
            const namingCfg = cfg.naming;

            // 获取地区简称
            const regionShort = namingCfg.regionShortNames[regionName] || regionName;

            // 格式化序号
            const paddedCount = count < 10 ? `0${count}` : `${count}`;

            // 提取关键标签（仅当有特殊关键词时）
            const keyTags = hasSpecialKeyword ? extractKeyTags(originalName) : [];
            const tagStr = keyTags.length > 0 ? ` ${keyTags.join('·')}` : '';

            // 获取特性emoji
            const featureEmoji = namingCfg.showFeatureEmoji
                ? (regionName === '台湾' ? '' : getRandItem(cfg.emoji[featType]))
                : '';

            // 根据命名风格生成名称
            switch (namingCfg.style) {
                case 'minimal':
                    // 简约风格: 🇭🇰 HK·01 或 🇭🇰 HK·01 IPLC·📺
                    if (featureEmoji) {
                        return `${regionFlag} ${regionShort}·${paddedCount}${tagStr} ${featureEmoji}`.trim();
                    }
                    return `${regionFlag} ${regionShort}·${paddedCount}${tagStr}`.trim();

                case 'standard':
                    // 标准风格: 🇭🇰 香港 01 或 🇭🇰 香港 01 IPLC
                    if (featureEmoji) {
                        return `${regionFlag} ${regionName} ${paddedCount}${tagStr} ${featureEmoji}`.trim();
                    }
                    return `${regionFlag} ${regionName} ${paddedCount}${tagStr}`.trim();

                case 'detailed':
                    // 详细风格: 🇭🇰 香港 | #01 IPLC 💎
                    if (featureEmoji) {
                        return `${regionFlag} ${regionName} | #${paddedCount}${tagStr} ${featureEmoji}`.trim();
                    }
                    return `${regionFlag} ${regionName} | #${paddedCount}${tagStr}`.trim();

                default:
                    return `${regionFlag} ${regionShort}·${paddedCount}${tagStr}`.trim();
            }
        };

        // 🚀 v3.6.0: 优化主处理循环 - 减少函数调用和字符串操作
        const len = dedupedProxies.length;
        for (let index = 0; index < len; index++) {
            const proxy = dedupedProxies[index];
            try {
                const processedProxy = optimizeProxy(proxy);
                const originalName = processedProxy.name ||
                    `${(processedProxy.type || 'UNKNOWN').toUpperCase()} ${processedProxy.server}:${processedProxy.port}`;
                processedProxy._originalName = originalName;

                // 缓存地区信息（memoize已处理）
                const regionInfo = getRegionInfo(originalName);
                const regionName = regionInfo.r;

                // 获取特性类型
                const featType = getFeatureType(originalName);

                // 计数器优化
                const count = incrementCounter(regionCounters, regionName);

                // 检查是否有特殊关键词
                const hasSpecialKeyword = hasSpecialKeywordCheck(originalName);

                // 🎨 使用美化函数生成名称
                processedProxy.name = beautifyNodeName(regionInfo, featType, count, originalName, hasSpecialKeyword);

                processedProxy._priority = regionInfo.p;
                processedProxy._index = index;
                processedProxies.push(processedProxy);
            } catch (e) {
                // 静默处理错误，保留原始节点
                proxy._error = true;
                processedProxies.push(proxy);
            }
        }

        const generateChainProxies = (exitNodes, entryGroup, chainType, priorityOffset, indexOffset, emoji) => {
            const padLength = exitNodes.length.toString().length;
            // 链类型简化映射
            const chainTypeShort = {
                '中继 Blaze': '🔗',
                '落地 Surge': '🎯'
            };
            const shortType = chainTypeShort[chainType] || '🔗';

            return exitNodes.map((exitNode, index) => {
                const chainProxy = removePortHoppingParams(exitNode);
                const regionInfo = getRegionInfo(chainProxy._originalName);
                const regionShort = cfg.naming.regionShortNames[regionInfo.r] || regionInfo.r;
                const paddedCount = (index + 1).toString().padStart(padLength, '0');

                // 提取关键标签
                const keyTags = extractKeyTags(chainProxy._originalName || '');
                const tagStr = keyTags.length > 0 ? ` ${keyTags.join('·')}` : '';

                chainProxy['underlying-proxy'] = entryGroup;
                // 美化链名称: 🔗 🇭🇰 HK·01 IPLC
                chainProxy.name = `${shortType} ${regionInfo.f} ${regionShort}·${paddedCount}${tagStr}`;
                chainProxy._priority = regionInfo.p + priorityOffset;
                chainProxy._index = indexOffset + index;
                return chainProxy;
            });
        };

        let relayChainProxies = [], landingChainProxies = [];
        if (cfg.generateRelayChains || cfg.generateLandingChains) {
            const exitNodeCandidates = processedProxies.filter(p => !p['underlying-proxy'] && p.type !== 'wireguard');

            if (cfg.generateRelayChains) {
                relayChainProxies = generateChainProxies(exitNodeCandidates, cfg.relayEntryGroupName, '中继 Blaze', -0.5, 10000, '🔗');
            }
            if (cfg.generateLandingChains) {
                landingChainProxies = generateChainProxies(exitNodeCandidates, cfg.landingEntryGroupName, '落地 Surge', -0.4, 20000, '🔗');
            }
        }

        let finalNodes;
        switch (cfg.outputMode) {
            case 'proxies_only': finalNodes = processedProxies; break;
            case 'relay_only': finalNodes = relayChainProxies; break;
            case 'landing_only': finalNodes = landingChainProxies; break;
            case 'airport_only': {
                // ✈️ 机场预设模式：美化命名但保持原始配置
                // 格式: ✈️ 🇭🇰 HK·01 IPLC·📺
                const airportCounters = new Map();
                finalNodes = proxies.map((proxy, idx) => {
                    if (!proxy || typeof proxy !== 'object') return proxy;

                    const originalName = proxy.name || `Node ${idx + 1}`;
                    const regionInfo = getRegionInfo(originalName);
                    const regionShort = cfg.naming.regionShortNames[regionInfo.r] || regionInfo.r;

                    // 计数
                    const count = (airportCounters.get(regionInfo.r) || 0) + 1;
                    airportCounters.set(regionInfo.r, count);
                    const paddedCount = count < 10 ? `0${count}` : `${count}`;

                    // 提取关键标签
                    const keyTags = extractKeyTags(originalName);
                    const tagStr = keyTags.length > 0 ? ` ${keyTags.join('·')}` : '';

                    // 美化名称: ✈️ 🇭🇰 HK·01 IPLC·📺
                    const beautifiedName = `✈️ ${regionInfo.f} ${regionShort}·${paddedCount}${tagStr}`;

                    return {
                        ...proxy,
                        name: beautifiedName,
                        _priority: regionInfo.p,
                        _index: idx
                    };
                });
                break;
            }
            default: finalNodes = processedProxies; break;
        }

        // 🚀 性能优化：使用更高效的排序算法
        if (cfg.sortEnabled) {
            finalNodes.sort((a, b) => {
                const pA = a._priority ?? 999;
                const pB = b._priority ?? 999;

                // 优先级排序
                if (pA !== pB) {
                    return cfg.reverseSort ? (pA - pB) : (pB - pA);
                }

                // 优先级相同时，按原始索引排序（保持稳定性）
                return (a._index ?? 0) - (b._index ?? 0);
            });
        }

        // 🆕 v3.5.5: 最终TLS安全检查 - 确保非白名单端口不会有意外的TLS
        for (let i = 0, len = finalNodes.length; i < len; i++) {
            const node = finalNodes[i];
            const nodeType = (node.type || '').toLowerCase();
            const nodePort = parseInt(node.port) || 443;

            // 只处理VMess和VLESS（非Reality）
            if ((nodeType === 'vmess' || nodeType === 'vless') && !isRealityNode(node)) {
                // 如果端口不在白名单中，且节点原本没有TLS标记，确保TLS为false
                if (!TLS_WHITELIST_PORTS.has(nodePort)) {
                    // 检查是否有Reality相关字段（双重保护）
                    const hasRealityFields = node.publicKey || node.shortId || node['public-key'] || node.pbk;
                    if (!hasRealityFields && node.tls === true) {
                        // 如果没有原始TLS标记但现在有TLS，可能是被错误设置的
                        // 检查是否有SNI或证书配置来判断是否是有意的TLS
                        const hasIntentionalTls = node.sni || node.servername || node['server-name'] ||
                            node.alpn || node['skip-cert-verify'] !== undefined;
                        if (!hasIntentionalTls) {
                            // 移除意外的TLS配置
                            node.tls = false;
                            delete node['skip-cert-verify'];
                            delete node['tls-min-version'];
                            delete node['tls-max-version'];
                        }
                    }
                }
            }
        }

        // 🚀 性能优化：使用预编译的属性列表批量清理临时属性
        for (let i = 0, len = finalNodes.length; i < len; i++) {
            const node = finalNodes[i];
            for (const prop of CLEANUP_PROPS) {
                delete node[prop];
            }
        }

        console.log('[node_rules_entrance] 处理完成，输出节点数:', finalNodes.length);
        return finalNodes;

    } catch (error) {
        console.log(`[v3.6.0] ❌ 错误: ${error.message}`);
        // 出错时返回原始节点，避免订阅完全失败
        return proxies;
    }
}
