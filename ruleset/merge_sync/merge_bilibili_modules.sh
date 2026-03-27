#!/usr/bin/env bash
# BiliBili Module Merge Script - 哔哩哔哩模块合并脚本
# 上游源: BiliUniverse (Enhanced/Global/Redirect/ADBlock) + Maasea (Helper)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_bilibili_merge"
OUTPUT_MODULE="$PROJECT_ROOT/module/surge(main)/amplify_nexus/📺 BiliBili增强合集.sgmodule"

ENHANCED_URL="https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"
GLOBAL_URL="https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"
REDIRECT_URL="https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"
ADBLOCK_URL="https://github.com/BiliUniverse/ADBlock/releases/latest/download/BiliBili.ADBlock.sgmodule"
HELPER_URL="https://raw.githubusercontent.com/Maasea/sgmodule/master/Bilibili.Helper.sgmodule"

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }

rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

log_info "下载上游BiliBili模块..."

for mod in Enhanced Global Redirect ADBlock Helper; do
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

for f in general script mitm rule header_rewrite url_rewrite map_local; do
    touch "$TEMP_DIR/$f.tmp"
done

# 直接拼接 arguments (不拆分，保持原格式)
args_list=""
args_desc_list=""

for mod in enhanced global redirect adblock helper; do
    # 直接提取整行 arguments，不拆分
    args=$(grep '^#!arguments *= *' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments *= *//' || true)
    if [ -n "$args" ]; then
        [ -n "$args_list" ] && args_list="$args_list,$args" || args_list="$args"
    fi
    
    # arguments-desc 带模块名前缀
    mod_name=$(grep '^#!name' "$TEMP_DIR/$mod.module" | head -1 | sed 's/^#!name *= *//')
    desc=$(grep '^#!arguments-desc *= *' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments-desc *= *//' || true)
    if [ -n "$desc" ]; then
        args_desc_list="${args_desc_list}\\n\\n[$mod_name]\\n$desc"
    fi
    
    awk '/^\[Script\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/script.tmp" 2>/dev/null || true
    awk '/^\[General\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/general.tmp" 2>/dev/null || true
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/rule.tmp" 2>/dev/null || true
    awk '/^\[Header Rewrite\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/header_rewrite.tmp" 2>/dev/null || true
    awk '/^\[URL Rewrite\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/url_rewrite.tmp" 2>/dev/null || true
    awk '/^\[Map Local\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/map_local.tmp" 2>/dev/null || true
    awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true
done

for f in script general rule header_rewrite url_rewrite map_local; do
    sort -u "$TEMP_DIR/$f.tmp" -o "$TEMP_DIR/$f.tmp"
done

mitm_hosts=$(sed 's/hostname = %APPEND% //' "$TEMP_DIR/mitm.tmp" | tr ',' '\n' | sed 's/^ *//;s/ *$//' | awk 'NF' | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')

cat > "$OUTPUT_MODULE" << 'HEADER'
#!name=📺 BiliBili增强合集
#!desc=合并BiliUniverse五大模块 (自动追随上游更新)\n⚙️ Enhanced: UI自定义\n🌐 Global: 全区搜索\n🔀 Redirect: CDN重定向\n🛡️ ADBlock: 去广告\n🛠️ Helper: 禁P2P
#!author=VirgilClyne, ClydeTime, Maasea
#!icon=https://github.com/BiliUniverse/Enhanced/raw/main/src/assets/icon_rounded.png
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!tag=BiliBili, 增强, 合并
#!openUrl=http://boxjs.com/#/app/BiliBili.Enhanced
HEADER

echo "#!date=$(date +%Y-%m-%d\ %H:%M:%S)" >> "$OUTPUT_MODULE"
[ -n "$args_list" ] && echo "#!arguments = $args_list" >> "$OUTPUT_MODULE"
[ -n "$args_desc_list" ] && echo "#!arguments-desc = $args_desc_list" >> "$OUTPUT_MODULE"
echo "" >> "$OUTPUT_MODULE"

[ -s "$TEMP_DIR/general.tmp" ] && { echo "[General]"; cat "$TEMP_DIR/general.tmp"; echo ""; } >> "$OUTPUT_MODULE"
[ -s "$TEMP_DIR/rule.tmp" ] && { echo "[Rule]"; cat "$TEMP_DIR/rule.tmp"; echo ""; } >> "$OUTPUT_MODULE"
[ -s "$TEMP_DIR/header_rewrite.tmp" ] && { echo "[Header Rewrite]"; cat "$TEMP_DIR/header_rewrite.tmp"; echo ""; } >> "$OUTPUT_MODULE"
[ -s "$TEMP_DIR/url_rewrite.tmp" ] && { echo "[URL Rewrite]"; cat "$TEMP_DIR/url_rewrite.tmp"; echo ""; } >> "$OUTPUT_MODULE"
[ -s "$TEMP_DIR/map_local.tmp" ] && { echo "[Map Local]"; cat "$TEMP_DIR/map_local.tmp"; echo ""; } >> "$OUTPUT_MODULE"

echo "[Script]" >> "$OUTPUT_MODULE"
echo "# BiliUniverse + Maasea Scripts" >> "$OUTPUT_MODULE"
cat "$TEMP_DIR/script.tmp" >> "$OUTPUT_MODULE"

cat >> "$OUTPUT_MODULE" << EOF

[MITM]
hostname = %APPEND% $mitm_hosts
h2 = true
EOF

rm -rf "$TEMP_DIR"
script_count=$(grep -c '^[a-zA-Z📺].*= *type=' "$OUTPUT_MODULE" 2>/dev/null || echo "0")
log_success "BiliBili模块合并完成: $script_count 脚本"
