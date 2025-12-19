#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# BiliBili Module Merge Script - 哔哩哔哩模块合并脚本
# 
# 上游源: VirgilClyne/BiliUniverse
# - Enhanced: UI自定义
# - Global: 全区搜索
# - Redirect: CDN重定向
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_bilibili_merge"
OUTPUT_MODULE="$PROJECT_ROOT/module/surge(main)/amplify_nexus/📺 BiliBili增强合集.sgmodule"

# 上游URL
ENHANCED_URL="https://raw.githubusercontent.com/BiliUniverse/Enhanced/main/modules/BiliBili.Enhanced.sgmodule"
GLOBAL_URL="https://raw.githubusercontent.com/BiliUniverse/Global/main/modules/BiliBili.Global.sgmodule"
REDIRECT_URL="https://raw.githubusercontent.com/BiliUniverse/Redirect/main/modules/BiliBili.Redirect.sgmodule"

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }

rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

log_info "下载上游BiliBili模块..."

# 下载模块
curl -sL "$ENHANCED_URL" -o "$TEMP_DIR/enhanced.module" 2>/dev/null || log_error "Enhanced下载失败"
curl -sL "$GLOBAL_URL" -o "$TEMP_DIR/global.module" 2>/dev/null || log_error "Global下载失败"
curl -sL "$REDIRECT_URL" -o "$TEMP_DIR/redirect.module" 2>/dev/null || log_error "Redirect下载失败"

# 如果下载失败，使用本地文件
[ ! -s "$TEMP_DIR/enhanced.module" ] && cp "$PROJECT_ROOT/module/surge(main)/amplify_nexus/BiliBili.Enhanced.sgmodule" "$TEMP_DIR/enhanced.module"
[ ! -s "$TEMP_DIR/global.module" ] && cp "$PROJECT_ROOT/module/surge(main)/amplify_nexus/BiliBili.Global.sgmodule" "$TEMP_DIR/global.module"
[ ! -s "$TEMP_DIR/redirect.module" ] && cp "$PROJECT_ROOT/module/surge(main)/amplify_nexus/BiliBili.Redirect.sgmodule" "$TEMP_DIR/redirect.module"

log_info "合并模块..."

# 提取各部分
extract_section() {
    local file="$1" section="$2" output="$3"
    awk -v sec="$section" '
        /^\[/ { in_section = ($0 ~ "^\\[" sec "\\]") }
        in_section && !/^\[/ && !/^#!/ && NF { print }
    ' "$file" >> "$output" 2>/dev/null || true
}

for f in general script mitm; do touch "$TEMP_DIR/$f.tmp"; done

# 提取各模块
for mod in enhanced global redirect; do
    extract_section "$TEMP_DIR/$mod.module" "General" "$TEMP_DIR/general.tmp"
    extract_section "$TEMP_DIR/$mod.module" "Script" "$TEMP_DIR/script.tmp"
    awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true
done

# 去重
sort -u "$TEMP_DIR/script.tmp" -o "$TEMP_DIR/script.tmp"

# 合并MITM
mitm_hosts=$(cat "$TEMP_DIR/mitm.tmp" | sed 's/hostname = %APPEND% //' | tr ',' '\n' | sed 's/^ *//' | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')

# 生成合并模块
cat > "$OUTPUT_MODULE" << 'EOF'
#!name=📺 BiliBili增强合集
#!desc=合并BiliUniverse三大模块 (自动追随上游更新)\n\n⚙️ Enhanced: UI自定义\n🌐 Global: 全区搜索\n🔀 Redirect: CDN重定向
#!author=VirgilClyne
#!icon=https://github.com/BiliUniverse/Enhanced/raw/main/src/assets/icon_rounded.png
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!tag=BiliBili, 增强, 合并
EOF
echo "#!date=$(date +%Y-%m-%d\ %H:%M:%S)" >> "$OUTPUT_MODULE"

# 添加参数 (合并三个模块的参数)
cat >> "$OUTPUT_MODULE" << 'EOF'

# ═══════════════════════════════════════════════════════════════
# Enhanced 参数
# ═══════════════════════════════════════════════════════════════
#!arguments = Home.Tab:"直播tab,推荐tab,hottopic,bangumi,anime,film,koreavtw",Home.Tab_default:"推荐tab",Home.Top_left:"mine",Home.Top:"消息Top",Bottom:"home,dynamic,ogv,会员购Bottom,我的Bottom",LogLevel:"WARN"

# ═══════════════════════════════════════════════════════════════
# Global 参数
# ═══════════════════════════════════════════════════════════════
#!arguments = ForceHost:"1",Locales:"CHN,HKG,TWN",Proxies.CHN:"DIRECT",Proxies.HKG:"🇭🇰香港",Proxies.MAC:"🇲🇴澳门",Proxies.TWN:"🇹🇼台湾"

# ═══════════════════════════════════════════════════════════════
# Redirect 参数
# ═══════════════════════════════════════════════════════════════
#!arguments = Host.Akamaized:"upos-sz-mirrorali.bilivideo.com",Host.BStar:"upos-sz-mirrorali.bilivideo.com",Host.PCDN:"upos-sz-mirrorali.bilivideo.com",Host.MCDN:"proxy-tf-all-ws.bilivideo.com"

EOF

# 添加General
if [ -s "$TEMP_DIR/general.tmp" ]; then
    echo "[General]" >> "$OUTPUT_MODULE"
    cat "$TEMP_DIR/general.tmp" >> "$OUTPUT_MODULE"
    echo "" >> "$OUTPUT_MODULE"
fi

# 添加Script
echo "[Script]" >> "$OUTPUT_MODULE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_MODULE"
echo "# BiliUniverse Scripts (Enhanced + Global + Redirect)" >> "$OUTPUT_MODULE"
echo "# ═══════════════════════════════════════════════════════════════" >> "$OUTPUT_MODULE"
cat "$TEMP_DIR/script.tmp" >> "$OUTPUT_MODULE"

# 添加MITM
cat >> "$OUTPUT_MODULE" << EOF

[MITM]
hostname = %APPEND% $mitm_hosts
h2 = true
EOF

rm -rf "$TEMP_DIR"

script_count=$(grep -c "^📺" "$OUTPUT_MODULE" 2>/dev/null || echo "0")
log_success "BiliBili模块合并完成: $script_count 脚本"
