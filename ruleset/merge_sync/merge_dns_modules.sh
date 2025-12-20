#!/bin/bash
# DNS模块合并脚本 - 从GetSomeFries上游自动下载合并
# 上游: VirgilClyne/GetSomeFries (General + DNS + ASN.China + HTTPDNS.Block)

set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/module/surge(main)/amplify_nexus"
OUTPUT_FILE="$OUTPUT_DIR/🌐 DNS & Host Enhanced.sgmodule"
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

# 上游URL
URLS=(
    "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/General.sgmodule"
    "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/DNS.sgmodule"
    "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/ASN.China.sgmodule"
    "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/HTTPDNS.Block.sgmodule"
)
NAMES=("General" "DNS" "ASN.China" "HTTPDNS.Block")

echo "[INFO] 下载上游GetSomeFries模块..."
for i in "${!URLS[@]}"; do
    if curl -sL "${URLS[$i]}" -o "$TEMP_DIR/${NAMES[$i]}.sgmodule" 2>/dev/null; then
        echo "[✓] ${NAMES[$i]} 下载成功"
    else
        echo "[✗] ${NAMES[$i]} 下载失败: ${URLS[$i]}"
        exit 1
    fi
done

echo "[INFO] 合并模块..."

# 读取现有模块的自定义内容（保留本地增强配置）
EXISTING_CUSTOM=""
if [ -f "$OUTPUT_FILE" ]; then
    # 提取本地自定义的Host和URL Rewrite（非上游内容）
    EXISTING_CUSTOM=$(sed -n '/# ═.*LOCAL CUSTOM/,/# ═.*END LOCAL/p' "$OUTPUT_FILE" 2>/dev/null || true)
fi

# 生成合并后的模块
cat > "$OUTPUT_FILE" << 'HEADER'
#!name=🌐 DNS & Host & URL Rewrite Enhanced
#!desc=🔒 全量DoH加密DNS + Host分流增强 + URL重写 + GetSomeFries增强 | 自动合并上游 General+DNS+ASN+HTTPDNS | 🔧 AUTO-MERGED
#!author=VirgilClyne & nyamiiko (Auto-Merged)
#!homepage=https://github.com/nowaytouse/script_hub
#!icon=https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/Server.png
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
HEADER
echo "#!version=$(date +%Y.%m.%d)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# 合并 [General] 段
echo "[General]" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
echo "# FROM: GetSomeFries General + DNS + HTTPDNS.Block" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"

# 从General提取
grep -E "^(skip-proxy|always-real-ip|force-http-engine-hosts)" "$TEMP_DIR/General.sgmodule" 2>/dev/null | head -20 >> "$OUTPUT_FILE" || true

# 从DNS提取
grep -E "^(use-local-host-item-for-proxy|encrypted-dns-follow-outbound-mode)" "$TEMP_DIR/DNS.sgmodule" 2>/dev/null >> "$OUTPUT_FILE" || true

# 从HTTPDNS.Block提取force-http-engine-hosts
HTTPDNS_FORCE=$(grep "^force-http-engine-hosts" "$TEMP_DIR/HTTPDNS.Block.sgmodule" 2>/dev/null | sed 's/force-http-engine-hosts = %APPEND% //' || true)
if [ -n "$HTTPDNS_FORCE" ]; then
    echo "# HTTPDNS Block force-http-engine-hosts" >> "$OUTPUT_FILE"
    echo "force-http-engine-hosts = %APPEND% $HTTPDNS_FORCE" >> "$OUTPUT_FILE"
fi

# 本地增强配置
cat >> "$OUTPUT_FILE" << 'LOCAL_GENERAL'

# ═══════════════════════════════════════════════════════════════
# LOCAL ENHANCED: Encrypted DNS + TUN + Raw TCP
# ═══════════════════════════════════════════════════════════════
encrypted-dns-server = h3://dns.alidns.com/dns-query, h3://cloudflare-dns.com/dns-query, https://dns11.quad9.net/dns-query, h3://dns.adguard-dns.com/dns-query, h3://dns.google/dns-query, https://doh.pub/dns-query, https://doh.360.cn/dns-query
tun-excluded-routes = 0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10, 127.0.0.0/8, 169.254.0.0/16, 172.16.0.0/12, 192.168.0.0/16, 224.0.0.0/4, 240.0.0.0/4, 255.255.255.255/32
tun-included-routes = 192.168.1.12/32, 192.168.1.100/32
always-raw-tcp-hosts = *.alipay.com, *.unionpay.com, *.95516.com, *.apple.com, *.jd.com, icbc.com.cn, ccb.com, cmbchina.com, 12306.cn
always-raw-tcp-keywords = alipay, pay, unionpay, 95516, apple, weixin, wechat, taobao, jd, bank, auth, login

LOCAL_GENERAL

# 合并 [Rule] 段 - ASN China + HTTPDNS Block
echo "" >> "$OUTPUT_FILE"
echo "[Rule]" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
echo "# FROM: GetSomeFries HTTPDNS.Block" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
sed -n '/^\[Rule\]/,/^\[/p' "$TEMP_DIR/HTTPDNS.Block.sgmodule" | grep -v '^\[' | grep -v '^$' | head -100 >> "$OUTPUT_FILE"

echo "" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
echo "# FROM: GetSomeFries ASN.China (中国大陆ASN直连)" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
sed -n '/^\[Rule\]/,/^\[/p' "$TEMP_DIR/ASN.China.sgmodule" | grep -v '^\[' | grep -v '^$' >> "$OUTPUT_FILE"

# 合并 [URL Rewrite] 段
echo "" >> "$OUTPUT_FILE"
echo "[URL Rewrite]" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
echo "# LOCAL: URL Rewrite Rules" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
cat >> "$OUTPUT_FILE" << 'URL_REWRITE'
# Google Redirect
^https?://(www\.)?(g|google)\.cn https://www.google.com 307
^https?://(www\.)?google\.com\.hk https://www.google.com 307
^https?://(ditu|maps)\.google\.cn https://maps.google.com 307

# HTTPS Redirect
^https?:\/\/(www\.)?taobao\.com\/ https://taobao.com/ 302
^https?:\/\/(www\.)?jd\.com\/ https://www.jd.com/ 302
^https?:\/\/(www\.)?mi\.com\/ https://www.mi.com/ 302
^https?:\/\/you\.163\.com\/ https://you.163.com/ 302
^https?:\/\/(www\.)?suning\.com\/ https://suning.com/ 302
^https?:\/\/(www\.)?yhd\.com\/ https://yhd.com/ 302

URL_REWRITE

# 合并 [Host] 段
echo "" >> "$OUTPUT_FILE"
echo "[Host]" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
echo "# FROM: GetSomeFries DNS" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
sed -n '/^\[Host\]/,/^\[/p' "$TEMP_DIR/DNS.sgmodule" | grep -v '^\[' | grep -v '^$' >> "$OUTPUT_FILE"

# 本地增强Host配置
cat >> "$OUTPUT_FILE" << 'LOCAL_HOST'

# ═══════════════════════════════════════════════════════════════
# LOCAL ENHANCED: Router Admin + Connectivity Check
# ═══════════════════════════════════════════════════════════════
*.local = server:system
*.lan = server:system
*.id.ui.direct = server:force-syslib
amplifi.lan = server:force-syslib
router.synology.com = server:force-syslib
router.asus.com = server:force-syslib
routerlogin.net = server:force-syslib
www.miwifi.com = server:force-syslib
miwifi.com = server:force-syslib
tplogin.cn = server:force-syslib
captive.apple.com = server:system
connectivitycheck.gstatic.com = server:system
detectportal.firefox.com = server:system
*.cn = server:system

# ═══════════════════════════════════════════════════════════════
# LOCAL ENHANCED: China Services DoH Optimization
# ═══════════════════════════════════════════════════════════════
*.alibaba.cn = server:h3://dns.alidns.com/dns-query
*.taobao.com = server:h3://dns.alidns.com/dns-query
*.tmall.com = server:h3://dns.alidns.com/dns-query
*.alipay.com = server:h3://dns.alidns.com/dns-query
*.tencent.com = server:https://doh.pub/dns-query
*.qq.com = server:https://doh.pub/dns-query
*.weixin.com = server:https://doh.pub/dns-query
*.baidu.com = server:h3://dns.alidns.com/dns-query
*.bilibili.com = server:h3://dns.alidns.com/dns-query
*.douyin.com = server:h3://dns.alidns.com/dns-query

LOCAL_HOST

# 合并 [MITM] 段
echo "" >> "$OUTPUT_FILE"
echo "[MITM]" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
echo "# FROM: GetSomeFries General" >> "$OUTPUT_FILE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_FILE"
sed -n '/^\[MITM\]/,/^\[/p' "$TEMP_DIR/General.sgmodule" | grep -v '^\[' | grep -v '^$' >> "$OUTPUT_FILE"

# 统计
RULE_COUNT=$(grep -c "^IP-ASN\|^DOMAIN\|^IP-CIDR" "$OUTPUT_FILE" 2>/dev/null || echo "0")
HOST_COUNT=$(grep -c "^[^#\[].*=" "$OUTPUT_FILE" 2>/dev/null | head -1 || echo "0")

echo "[✓] DNS模块合并完成: $RULE_COUNT 规则"
