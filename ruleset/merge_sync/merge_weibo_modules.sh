#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# Weibo Module Merge Script - 微博模块合并脚本
# 
# 上游源:
# - fmz200/wool_scripts: 微博去广告&净化
# - iab0x00: 微博国际版去广告
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_weibo_merge"
OUTPUT_MODULE="$PROJECT_ROOT/module/surge(main)/narrow_pierce/🐦 微博去广告合集.sgmodule"
CACHE_DIR="$PROJECT_ROOT/.cache"

# 上游URL
WEIBO_MAIN_URL="https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/weibo.module"
WEIBO_INTL_URL="https://raw.githubusercontent.com/iab0x00/Surge/main/Module/WeiboIntl.sgmodule"

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }

rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR" "$CACHE_DIR"

log_info "下载上游微博模块..."

# 下载微博主版
if curl -sL "$WEIBO_MAIN_URL" -o "$TEMP_DIR/weibo_main.module" 2>/dev/null; then
    log_success "微博主版下载成功"
else
    log_error "微博主版下载失败: $WEIBO_MAIN_URL"
    exit 1
fi

# 下载微博国际版 (可能失败，使用备用)
if curl -sL "$WEIBO_INTL_URL" -o "$TEMP_DIR/weibo_intl.module" 2>/dev/null && [ -s "$TEMP_DIR/weibo_intl.module" ]; then
    log_success "微博国际版下载成功"
else
    log_info "微博国际版使用内置规则"
    cat > "$TEMP_DIR/weibo_intl.module" << 'INTL_EOF'
[URL Rewrite]
^https?:\/\/weibointl\.api\.weibo\.cn\/portal\.php\?(a=get_searching_info&|ct=feed&a=search_topic&) - reject-dict
^https?:\/\/api\.weibo\.cn\/2\/ad\/weibointl\? - reject-dict

[Body Rewrite]
http-response-jq ^https?:\/\/api\.weibo\.cn\/2\/statuses\/unread_hot_timeline$ 'del(.ad, .advertises, .trends) | if .statuses then .statuses |= map(select(((.promotion.type == "ad") or (.mblogtypename | IN("广告", "廣告", "热推", "熱推"))) | not)) end'
http-response-jq ^https?:\/\/weibointl\.api\.weibo\.cn\/portal\.php\?a=get_coopen_ads& '.data |= . + {"ad_list":[],"pic_ad":[],"gdt_video_ad_ios":[],"display_ad":0,"ad_ios_id":null,"app_ad_ios_id":null,"reserve_ad_ios_id":"","reserve_app_ad_ios_id":"","ad_duration":604800,"ad_cd_interval":604800}'
http-response-jq ^https?:\/\/weibointl\.api\.weibo\.cn\/portal\.php\?a=trends& 'if .data.order then .data.order = ["search_topic"] end'
http-response-jq ^https?:\/\/weibointl\.api\.weibo\.cn\/portal\.php\?a=search_topic& 'if .data.search_topic.cards[0].type == "searchtop" then del(.data.search_topic.cards[0]) end'
http-response-jq ^https?:\/\/weibointl\.api\.weibo\.cn\/portal\.php\?a=user_center& 'if .data.cards then .data.cards[].items |= map(select(.type != "personal_vip")) | .data.cards |= map(select((.items | length) > 0)) end'

[MITM]
hostname = %APPEND% weibointl.api.weibo.cn
INTL_EOF
fi

log_info "合并模块..."

# 提取各部分
extract_section() {
    local file="$1" section="$2" output="$3"
    awk -v sec="$section" '
        /^\[/ { in_section = ($0 ~ "^\\[" sec "\\]") }
        in_section && !/^\[/ && !/^#!/ && NF { print }
    ' "$file" >> "$output" 2>/dev/null || true
}

# 初始化临时文件
for f in rules url_rewrite body_rewrite map_local script mitm; do
    touch "$TEMP_DIR/$f.tmp"
done

# 提取微博主版
extract_section "$TEMP_DIR/weibo_main.module" "Rule" "$TEMP_DIR/rules.tmp"
extract_section "$TEMP_DIR/weibo_main.module" "URL Rewrite" "$TEMP_DIR/url_rewrite.tmp"
extract_section "$TEMP_DIR/weibo_main.module" "Body Rewrite" "$TEMP_DIR/body_rewrite.tmp"
extract_section "$TEMP_DIR/weibo_main.module" "Map Local" "$TEMP_DIR/map_local.tmp"
extract_section "$TEMP_DIR/weibo_main.module" "Script" "$TEMP_DIR/script.tmp"
awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/weibo_main.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true

# 提取微博国际版
extract_section "$TEMP_DIR/weibo_intl.module" "URL Rewrite" "$TEMP_DIR/url_rewrite.tmp"
extract_section "$TEMP_DIR/weibo_intl.module" "Body Rewrite" "$TEMP_DIR/body_rewrite.tmp"
awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /hostname/' "$TEMP_DIR/weibo_intl.module" >> "$TEMP_DIR/mitm.tmp" 2>/dev/null || true

# 去重
for f in rules url_rewrite body_rewrite map_local script; do
    sort -u "$TEMP_DIR/$f.tmp" -o "$TEMP_DIR/$f.tmp" 2>/dev/null || true
done

# 合并MITM hostname
mitm_hosts=$(cat "$TEMP_DIR/mitm.tmp" | sed 's/hostname = %APPEND% //' | sed 's/hostname = //' | tr ',' '\n' | sed 's/^ *//' | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')

# 生成合并模块
cat > "$OUTPUT_MODULE" << EOF
#!name=🐦 微博去广告合集
#!desc=合并微博+微博国际版去广告 (自动追随上游更新)\\n\\n上游: fmz200/wool_scripts, iab0x00
#!author=fmz200, iab0x00, kokoryh, zmqcherish
#!icon=https://raw.githubusercontent.com/fmz200/wool_scripts/main/icons/apps/Weibo-00.png
#!category=『 🎯 Narrow Pierce › 窄域穿刺 』
#!tag=去广告, 微博, 合并, 自动更新
#!date=$(date +%Y-%m-%d\ %H:%M:%S)

[Rule]
$(cat "$TEMP_DIR/rules.tmp")

[URL Rewrite]
$(cat "$TEMP_DIR/url_rewrite.tmp")

[Body Rewrite]
$(cat "$TEMP_DIR/body_rewrite.tmp")

[Map Local]
$(cat "$TEMP_DIR/map_local.tmp")

[Script]
$(cat "$TEMP_DIR/script.tmp")

[MITM]
hostname = %APPEND% $mitm_hosts
EOF

rm -rf "$TEMP_DIR"

rule_count=$(grep -c "^DOMAIN\|^URL-REGEX" "$OUTPUT_MODULE" 2>/dev/null || echo "0")
script_count=$(grep -c "^微博" "$OUTPUT_MODULE" 2>/dev/null || echo "0")

log_success "微博模块合并完成: $rule_count 规则, $script_count 脚本"
