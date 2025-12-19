#!/usr/bin/env bash
# YouTube Module Merge Script - YouTube模块合并脚本
# 上游源: Maasea/sgmodule

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_youtube_merge"
OUTPUT_MODULE="$PROJECT_ROOT/module/surge(main)/amplify_nexus/📺 YouTube增强合集.sgmodule"

# 上游URL (Maasea)
ENHANCE_URL="https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.Enhance.sgmodule"
ADBLOCK_URL="https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.ADBlock.sgmodule"

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }

rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

log_info "下载上游YouTube模块..."

for mod in Enhance ADBlock; do
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

for f in script mitm rule map_local; do
    touch "$TEMP_DIR/$f.tmp"
done

args_list=""
args_desc_list=""

for mod in enhance adblock; do
    args=$(grep '^#!arguments *= *' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments *= *//' || true)
    [ -n "$args" ] && { [ -n "$args_list" ] && args_list="$args_list,$args" || args_list="$args"; }
    
    mod_name=$(grep '^#!name' "$TEMP_DIR/$mod.module" | head -1 | sed 's/^#!name *= *//')
    desc=$(grep '^#!arguments-desc *= *' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments-desc *= *//' || true)
    [ -n "$desc" ] && args_desc_list="${args_desc_list}\\n\\n[$mod_name]\\n$desc"
    
    awk '/^\[Script\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/script.tmp" 2>/dev/null || true
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/rule.tmp" 2>/dev/null || true
    awk '/^\[Map Local\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/map_local.tmp" 2>/dev/null || true
    awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true
done

for f in script rule map_local; do
    sort -u "$TEMP_DIR/$f.tmp" -o "$TEMP_DIR/$f.tmp"
done

mitm_hosts=$(sed 's/hostname = %APPEND% //' "$TEMP_DIR/mitm.tmp" | tr ',' '\n' | sed 's/^ *//;s/ *$//' | awk 'NF' | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')

cat > "$OUTPUT_MODULE" << 'HEADER'
#!name=📺 YouTube增强合集
#!desc=合并YouTube增强+去广告 (自动追随上游更新)\n🎬 Enhance: 画中画/后台播放/字幕翻译\n🛡️ ADBlock: 去广告
#!author=Maasea
#!icon=https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/YouTube.png
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!tag=YouTube, 增强, 去广告
HEADER

echo "#!date=$(date +%Y-%m-%d\ %H:%M:%S)" >> "$OUTPUT_MODULE"
[ -n "$args_list" ] && echo "#!arguments = $args_list" >> "$OUTPUT_MODULE"
[ -n "$args_desc_list" ] && echo "#!arguments-desc = $args_desc_list" >> "$OUTPUT_MODULE"
echo "" >> "$OUTPUT_MODULE"

[ -s "$TEMP_DIR/rule.tmp" ] && { echo "[Rule]"; cat "$TEMP_DIR/rule.tmp"; echo ""; } >> "$OUTPUT_MODULE"
[ -s "$TEMP_DIR/map_local.tmp" ] && { echo "[Map Local]"; cat "$TEMP_DIR/map_local.tmp"; echo ""; } >> "$OUTPUT_MODULE"

echo "[Script]" >> "$OUTPUT_MODULE"
cat "$TEMP_DIR/script.tmp" >> "$OUTPUT_MODULE"

cat >> "$OUTPUT_MODULE" << EOF

[MITM]
hostname = %APPEND% $mitm_hosts
EOF

rm -rf "$TEMP_DIR"
script_count=$(grep -c '= *type=' "$OUTPUT_MODULE" 2>/dev/null || echo "0")
log_success "YouTube模块合并完成: $script_count 脚本"
