#!/bin/bash
# =============================================================================
# 添加缺失的规则集定义
# 功能: 为 Singbox 配置添加所有缺失的规则集定义
# 创建: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           添加缺失的规则集定义                               ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "配置文件不存在: $SINGBOX_CONFIG"
    exit 1
fi

# 备份原配置
cp "$SINGBOX_CONFIG" "${SINGBOX_CONFIG}.backup.$(date +%Y%m%d_%H%M%S)"
log_info "已备份配置文件"

# 使用 Python 添加缺失的规则集
python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys

config_file = sys.argv[1]

# 读取配置
with open(config_file, 'r', encoding='utf-8') as f:
    config = json.load(f)

# 获取现有规则集
existing_rule_sets = config.get('route', {}).get('rule_set', [])
existing_tags = {rs['tag'] for rs in existing_rule_sets}

# 定义需要添加的规则集
additional_rule_sets = []

# GitHub base URL
github_base = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox"

# 规则集映射：将缺失的规则集映射到本地已有的规则集
ruleset_mapping = {
    # 已合并到现有规则集的映射
    "adblock-merged": "AdBlock",           # 广告拦截已合并
    "cnsite": "ChinaDirect",               # 中国站点已合并到ChinaDirect
    "cngames": "Gaming",                   # 中国游戏已合并到Gaming
    "gfw": "GlobalProxy",                  # GFW列表已合并到GlobalProxy
    
    # geosite 映射到本地规则集
    "geosite-cn": "ChinaDirect",           # 中国站点
    "geosite-private": "LAN",              # 私有网络
    "geosite-geolocation-!cn": "GlobalProxy",  # 非中国站点
    "geosite-advertising": "AdBlock",      # 广告
    "geosite-hijacking": "AdBlock",        # 劫持
    "geosite-openai": "AI",                # OpenAI
    "geosite-anthropic": "AI",             # Anthropic
    "geosite-google-gemini": "AI",         # Google Gemini
    "geosite-disney": "Disney",            # Disney
    
    # geoip 映射（使用ChinaIP作为基础）
    "geoip-jp": "ChinaIP",                 # 日本IP（暂用ChinaIP，实际需要时再添加）
    "geoip-us": "ChinaIP",                 # 美国IP（暂用ChinaIP，实际需要时再添加）
    "geoip-kr": "ChinaIP",                 # 韩国IP（暂用ChinaIP，实际需要时再添加）
    
    # sukka 规则集映射
    "sukka-apple-cdn": "Apple",            # Apple CDN
    "sukka-cdn": "CDN",                    # CDN
    "sukka-speedtest": "Speedtest",        # Speedtest
    
    # Manual 规则集（修正标签名）
    "surge-manual_global": "Manual_Global",
    "surge-manual_jp": "Manual_JP",
    "surge-manual_us": "Manual_US",
    "surge-manual_west": "Manual_West",
}

# 生成规则集定义
all_additional = []
for missing_tag, local_name in ruleset_mapping.items():
    all_additional.append({
        "tag": missing_tag,
        "url": f"{github_base}/{local_name}_Singbox.srs"
    })

# 添加缺失的规则集
added_count = 0
for rs in all_additional:
    if rs['tag'] not in existing_tags:
        rule_set_def = {
            "tag": rs['tag'],
            "type": "remote",
            "format": rs.get('format', 'binary'),
            "url": rs['url'],
            "download_detour": "direct-select",
            "update_interval": "24h"
        }
        existing_rule_sets.append(rule_set_def)
        existing_tags.add(rs['tag'])
        added_count += 1
        print(f"  ✅ 添加: {rs['tag']}")

# 更新配置
config['route']['rule_set'] = existing_rule_sets

# 保存配置
with open(config_file, 'w', encoding='utf-8') as f:
    json.dump(config, f, indent=2, ensure_ascii=False)

print(f"\n✅ 成功添加 {added_count} 个规则集定义")
print(f"   总规则集数: {len(existing_rule_sets)}")

PYTHON_SCRIPT

log_success "规则集定义已更新"
echo ""

# 验证配置
log_info "验证配置完整性..."
./merge_sync/validate_singbox_config.sh

echo ""
log_success "完成！现在可以尝试启动 Singbox"
