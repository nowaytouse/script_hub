#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# BiliBili Module Merge Script - 哔哩哔哩模块合并脚本
# 上游源: BiliUniverse (Enhanced/Global/Redirect) + Maasea (Helper)
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_bilibili_merge"
OUTPUT_MODULE="$PROJECT_ROOT/module/surge(main)/amplify_nexus/📺 BiliBili增强合集.sgmodule"

# 上游URL
ENHANCED_URL="https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"
GLOBAL_URL="https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"
REDIRECT_URL="https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"
HELPER_URL="https://raw.githubusercontent.com/Maasea/sgmodule/master/Bilibili.Helper.sgmodule"

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }

rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

log_info "下载上游BiliBili模块..."

# 下载模块
for mod in Enhanced Global Redirect Helper; do
    url_var="${mod^^}_URL"
    url="${!url_var}"
    output="$TEMP_DIR/${mod,,}.module"
    if curl -sL "$url" -o "$output" 2>/dev/null && [ -s "$output" ]; then
        log_success "$mod 下载成功"
    else
        log_error "$mod 下载失败: $url"
        exit 1
    fi
done

log_info "合并模块..."

# 初始化临时文件
for f in general script mitm arguments arguments_desc rule header_rewrite; do
    touch "$TEMP_DIR/$f.tmp"
done

# 提取各部分
for mod in enhanced global redirect helper; do
    # #!arguments
    grep '^#!arguments *= *' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments *= *//' >> "$TEMP_DIR/arguments.tmp" || true
    
    # #!arguments-desc
    mod_name=$(grep '^#!name' "$TEMP_DIR/$mod.module" | head -1 | sed 's/^#!name *= *//')
    desc=$(grep '^#!arguments-desc *= *' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments-desc *= *//' || true)
    [ -n "$desc" ] && echo "[$mod_name]\\n$desc" >> "$TEMP_DIR/arguments_desc.tmp"
    
    # [Script]
    awk '/^\[Script\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/script.tmp" 2>/dev/null || true
    
    # [General]
    awk '/^\[General\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/general.tmp" 2>/dev/null || true
    
    # [Rule]
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/rule.tmp" 2>/dev/null || true
    
    # [Header Rewrite]
    awk '/^\[Header Rewrite\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/header_rewrite.tmp" 2>/dev/null || true
    
    # [MITM] hostname
    awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true
done

# 去重
for f in script general rule header_rewrite; do
    sort -u "$TEMP_DIR/$f.tmp" -o "$TEMP_DIR/$f.tmp"
done

# 合并MITM hostname
mitm_hosts=$(cat "$TEMP_DIR/mitm.tmp" | sed 's/hostname = %APPEND% //' | tr ',' '\n' | sed 's/^ *//' | sed 's/ *$//' | grep -v '^$' | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')

# 合并 arguments
merged_args=$(cat "$TEMP_DIR/arguments.tmp" | tr ',' '\n' | sed 's/^ *//' | sed 's/ *$//' | grep -v '^$' | sort -u | tr '\n' ',' | sed 's/,$//')

# 合并 arguments-desc
merged_args_desc=$(cat "$TEMP_DIR/arguments_desc.tmp" | tr -d '\n' | sed 's/\[📺/\\n\\n[📺/g' | sed 's/\[Bilibili/\\n\\n[Bilibili/g')

# 生成合并模块
cat > "$OUTPUT_MODULE" << 'HEADER'
#!name=📺 BiliBili增强合集
#!desc=合并BiliUniverse四大模块 (自动追随上游更新)\n⚙️ Enhanced: UI自定义\n🌐 Global: 全区搜索\n🔀 Redirect: CDN重定向\n🛠️ Helper: 去广告+禁P2P
#!author=VirgilClyne, Maasea
#!icon=https://github.com/BiliUniverse/Enhanced/raw/main/src/assets/icon_rounded.png
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!tag=BiliBili, 增强, 合并
#!openUrl=http://boxjs.com/#/app/BiliBili.Enhanced
HEADER

echo "#!date=$(date +%Y-%m-%d\ %H:%M:%S)" >> "$OUTPUT_MODULE"

# 添加参数
[ -n "$merged_args" ] && echo "#!arguments = $merged_args" >> "$OUTPUT_MODULE"
[ -s "$TEMP_DIR/arguments_desc.tmp" ] && echo "#!arguments-desc = $merged_args_desc" >> "$OUTPUT_MODULE"

echo "" >> "$OUTPUT_MODULE"

# [General]
if [ -s "$TEMP_DIR/general.tmp" ]; then
    echo "[General]" >> "$OUTPUT_MODULE"
    cat "$TEMP_DIR/general.tmp" >> "$OUTPUT_MODULE"
    echo "" >> "$OUTPUT_MODULE"
fi

# [Rule]
if [ -s "$TEMP_DIR/rule.tmp" ]; then
    echo "[Rule]" >> "$OUTPUT_MODULE"
    cat "$TEMP_DIR/rule.tmp" >> "$OUTPUT_MODULE"
    echo "" >> "$OUTPUT_MODULE"
fi

# [Header Rewrite]
if [ -s "$TEMP_DIR/header_rewrite.tmp" ]; then
    echo "[Header Rewrite]" >> "$OUTPUT_MODULE"
    cat "$TEMP_DIR/header_rewrite.tmp" >> "$OUTPUT_MODULE"
    echo "" >> "$OUTPUT_MODULE"
fi

# [Script]
echo "[Script]" >> "$OUTPUT_MODULE"
echo "# BiliUniverse + Maasea Scripts" >> "$OUTPUT_MODULE"
cat "$TEMP_DIR/script.tmp" >> "$OUTPUT_MODULE"

# [MITM]
cat >> "$OUTPUT_MODULE" << EOF

[MITM]
hostname = %APPEND% $mitm_hosts
h2 = true
EOF

rm -rf "$TEMP_DIR"

script_count=$(grep -c '^[a-zA-Z📺].*= *type=' "$OUTPUT_MODULE" 2>/dev/null || echo "0")
log_success "BiliBili模块合并完成: $script_count 脚本"
