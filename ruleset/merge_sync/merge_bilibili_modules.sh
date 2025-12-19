#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# BiliBili Module Merge Script - 哔哩哔哩模块合并脚本
# 上游源: VirgilClyne/BiliUniverse (GitHub Releases)
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_bilibili_merge"
OUTPUT_MODULE="$PROJECT_ROOT/module/surge(main)/amplify_nexus/📺 BiliBili增强合集.sgmodule"

# 上游URL (GitHub Releases)
ENHANCED_URL="https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"
GLOBAL_URL="https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"
REDIRECT_URL="https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }

rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

log_info "下载上游BiliBili模块..."

# 下载模块
for mod in Enhanced Global Redirect; do
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
touch "$TEMP_DIR/general.tmp" "$TEMP_DIR/script.tmp" "$TEMP_DIR/mitm.tmp"
touch "$TEMP_DIR/arguments.tmp" "$TEMP_DIR/arguments_desc.tmp"

# 提取 #!arguments (参数定义)
for mod in enhanced global redirect; do
    grep '^#!arguments = ' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments = //' >> "$TEMP_DIR/arguments.tmp" || true
done

# 提取 #!arguments-desc (参数描述)
for mod in enhanced global redirect; do
    # 提取模块名作为前缀
    mod_name=$(grep '^#!name' "$TEMP_DIR/$mod.module" | head -1 | sed 's/^#!name *= *//')
    desc=$(grep '^#!arguments-desc = ' "$TEMP_DIR/$mod.module" 2>/dev/null | sed 's/^#!arguments-desc = //' || true)
    if [ -n "$desc" ]; then
        echo "[$mod_name]\\n$desc" >> "$TEMP_DIR/arguments_desc.tmp"
    fi
done

# 提取 [Script] 部分
for mod in enhanced global redirect; do
    awk '/^\[Script\]/{f=1;next}/^\[/{f=0}f && /^📺/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/script.tmp" 2>/dev/null || true
done

# 提取 [General] 部分
for mod in enhanced global redirect; do
    awk '/^\[General\]/{f=1;next}/^\[/{f=0}f && NF && !/^#/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/general.tmp" 2>/dev/null || true
done

# 提取 MITM hostname
for mod in enhanced global redirect; do
    awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/$mod.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true
done

# 去重
sort -u "$TEMP_DIR/script.tmp" -o "$TEMP_DIR/script.tmp"
sort -u "$TEMP_DIR/general.tmp" -o "$TEMP_DIR/general.tmp"

# 合并MITM hostname
mitm_hosts=$(cat "$TEMP_DIR/mitm.tmp" | sed 's/hostname = %APPEND% //' | tr ',' '\n' | sed 's/^ *//' | sed 's/ *$//' | grep -v '^$' | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')

# 合并 arguments (用逗号分隔)
merged_args=$(cat "$TEMP_DIR/arguments.tmp" | tr ',' '\n' | sed 's/^ *//' | sed 's/ *$//' | grep -v '^$' | sort -u | tr '\n' ',' | sed 's/,$//')

# 合并 arguments-desc (用换行分隔)
merged_args_desc=$(cat "$TEMP_DIR/arguments_desc.tmp" | tr -d '\n' | sed 's/\[📺/\\n\\n[📺/g')

# 生成合并模块头部
cat > "$OUTPUT_MODULE" << EOF
#!name=📺 BiliBili增强合集
#!desc=合并BiliUniverse三大模块 (自动追随上游更新)\\n⚙️ Enhanced: UI自定义\\n🌐 Global: 全区搜索\\n🔀 Redirect: CDN重定向
#!author=VirgilClyne
#!icon=https://github.com/BiliUniverse/Enhanced/raw/main/src/assets/icon_rounded.png
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!tag=BiliBili, 增强, 合并
#!openUrl=http://boxjs.com/#/app/BiliBili.Enhanced
#!date=$(date +%Y-%m-%d\ %H:%M:%S)
EOF

# 添加合并的 arguments
if [ -n "$merged_args" ]; then
    echo "#!arguments = $merged_args" >> "$OUTPUT_MODULE"
fi

# 添加合并的 arguments-desc
if [ -s "$TEMP_DIR/arguments_desc.tmp" ]; then
    echo "#!arguments-desc = $merged_args_desc" >> "$OUTPUT_MODULE"
fi

echo "" >> "$OUTPUT_MODULE"

# 添加 [General]
if [ -s "$TEMP_DIR/general.tmp" ]; then
    echo "[General]" >> "$OUTPUT_MODULE"
    cat "$TEMP_DIR/general.tmp" >> "$OUTPUT_MODULE"
    echo "" >> "$OUTPUT_MODULE"
fi

# 添加 [Script]
echo "[Script]" >> "$OUTPUT_MODULE"
echo "# BiliUniverse Scripts (Enhanced + Global + Redirect)" >> "$OUTPUT_MODULE"
cat "$TEMP_DIR/script.tmp" >> "$OUTPUT_MODULE"

# 添加 [MITM]
cat >> "$OUTPUT_MODULE" << EOF

[MITM]
hostname = %APPEND% $mitm_hosts
h2 = true
EOF

rm -rf "$TEMP_DIR"

script_count=$(grep -c "^📺" "$OUTPUT_MODULE" 2>/dev/null || echo "0")
log_success "BiliBili模块合并完成: $script_count 脚本"
