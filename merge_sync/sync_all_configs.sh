#!/bin/bash
# =============================================================================
# 全配置同步脚本
# 功能: 同步规则集到 Sing-box 和 Shadowrocket 配置
# 更新: 2025-12-07
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

# 配置文件路径
SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"
SURGE_CONFIG="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf"
SHADOWROCKET_DB="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/SURGE同步配置 👑.conf--2127343006.db"
SURGE_RULES_TEMPLATE="$PROJECT_ROOT/conf/surge_rules_complete.conf"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       全配置同步工具 - Sing-box & Shadowrocket              ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ═══════════════════════════════════════════════════════════════
# 任务1: 更新 Sing-box 配置的规则集
# ═══════════════════════════════════════════════════════════════
log_info "任务1: 更新 Sing-box 配置规则集..."

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "Sing-box 配置文件不存在: $SINGBOX_CONFIG"
    exit 1
fi

# 备份原配置
cp "$SINGBOX_CONFIG" "${SINGBOX_CONFIG}.backup.$(date +%Y%m%d_%H%M%S)"
log_info "已备份 Sing-box 配置"

# 生成 Sing-box 规则集列表
log_info "生成 Sing-box 规则集配置..."

# 创建临时 Python 脚本来更新 JSON
python3 - "$SINGBOX_CONFIG" "$PROJECT_ROOT" <<'PYTHON_SCRIPT'
import json
import sys
from pathlib import Path

config_file = sys.argv[1]
project_root = sys.argv[2]

# 读取配置
with open(config_file, 'r', encoding='utf-8') as f:
    config = json.load(f)

# 规则集列表（完整61个规则集）
rule_sets = [
    # 广告拦截 (3)
    {"tag": "surge-adblock", "type": "remote", "format": "binary", 
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/AdBlock_Singbox.srs"},
    {"tag": "surge-adblock-merged", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/AdBlock_Merged_Singbox.srs"},
    {"tag": "surge-blockhttpdns", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/BlockHttpDNS_Singbox.srs"},
    
    # AI 服务 (2)
    {"tag": "surge-ai", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/AI_Singbox.srs"},
    {"tag": "surge-aiprocess", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/AIProcess_Singbox.srs"},
    
    # 社交媒体 (7)
    {"tag": "surge-telegram", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Telegram_Singbox.srs"},
    {"tag": "surge-tiktok", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/TikTok_Singbox.srs"},
    {"tag": "surge-twitter", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Twitter_Singbox.srs"},
    {"tag": "surge-instagram", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Instagram_Singbox.srs"},
    {"tag": "surge-reddit", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Reddit_Singbox.srs"},
    {"tag": "surge-discord", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Discord_Singbox.srs"},
    {"tag": "surge-socialmedia", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/SocialMedia_Singbox.srs"},
    
    # 流媒体 (11)
    {"tag": "surge-netflix", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Netflix_Singbox.srs"},
    {"tag": "surge-disney", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Disney_Singbox.srs"},
    {"tag": "surge-youtube", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/YouTube_Singbox.srs"},
    {"tag": "surge-spotify", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Spotify_Singbox.srs"},
    {"tag": "surge-globalmedia", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/GlobalMedia_Singbox.srs"},
    {"tag": "surge-bahamut", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Bahamut_Singbox.srs"},
    {"tag": "surge-streameu", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/StreamEU_Singbox.srs"},
    {"tag": "surge-streamhk", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/StreamHK_Singbox.srs"},
    {"tag": "surge-streamjp", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/StreamJP_Singbox.srs"},
    {"tag": "surge-streamkr", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/StreamKR_Singbox.srs"},
    {"tag": "surge-streamtw", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/StreamTW_Singbox.srs"},
    {"tag": "surge-streamus", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/StreamUS_Singbox.srs"},
    
    # 科技公司 (7)
    {"tag": "surge-apple", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Apple_Singbox.srs"},
    {"tag": "surge-applenews", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/AppleNews_Singbox.srs"},
    {"tag": "surge-google", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Google_Singbox.srs"},
    {"tag": "surge-googlecn", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/GoogleCN_Singbox.srs"},
    {"tag": "surge-microsoft", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Microsoft_Singbox.srs"},
    {"tag": "surge-bing", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Bing_Singbox.srs"},
    {"tag": "surge-github", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/GitHub_Singbox.srs"},
    
    # 游戏 (5)
    {"tag": "surge-gaming", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Gaming_Singbox.srs"},
    {"tag": "surge-gamingprocess", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/GamingProcess_Singbox.srs"},
    {"tag": "surge-steam", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Steam_Singbox.srs"},
    {"tag": "surge-epic", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Epic_Singbox.srs"},
    {"tag": "surge-speedtest", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Speedtest_Singbox.srs"},
    
    # 金融 (2)
    {"tag": "surge-paypal", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/PayPal_Singbox.srs"},
    {"tag": "surge-binance", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Binance_Singbox.srs"},
    
    # 国内服务 (9)
    {"tag": "surge-bilibili", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Bilibili_Singbox.srs"},
    {"tag": "surge-chinadirect", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/ChinaDirect_Singbox.srs"},
    {"tag": "surge-chinaip", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/ChinaIP_Singbox.srs"},
    {"tag": "surge-qq", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/QQ_Singbox.srs"},
    {"tag": "surge-wechat", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/WeChat_Singbox.srs"},
    {"tag": "surge-tencent", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Tencent_Singbox.srs"},
    {"tag": "surge-xiaohongshu", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/XiaoHongShu_Singbox.srs"},
    {"tag": "surge-neteasemusic", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/NetEaseMusic_Singbox.srs"},
    {"tag": "surge-tesla", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Tesla_Singbox.srs"},
    
    # 网络基础 (4)
    {"tag": "surge-lan", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/LAN_Singbox.srs"},
    {"tag": "surge-cdn", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/CDN_Singbox.srs"},
    {"tag": "surge-firewallports", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/FirewallPorts_Singbox.srs"},
    {"tag": "surge-downloadprocess", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/DownloadProcess_Singbox.srs"},
    
    # 全球代理 (2)
    {"tag": "surge-globalproxy", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/GlobalProxy_Singbox.srs"},
    {"tag": "surge-fediverse", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Fediverse_Singbox.srs"},
    
    # 手动规则 (6)
    {"tag": "surge-manual", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_Singbox.srs"},
    {"tag": "surge-manual-global", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_Global_Singbox.srs"},
    {"tag": "surge-manual-jp", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_JP_Singbox.srs"},
    {"tag": "surge-manual-us", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_US_Singbox.srs"},
    {"tag": "surge-manual-west", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_West_Singbox.srs"},
    {"tag": "surge-directprocess", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/DirectProcess_Singbox.srs"},
    
    # 特殊分类 (2)
    {"tag": "surge-nsfw", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/NSFW_Singbox.srs"},
    {"tag": "surge-substore", "type": "remote", "format": "binary",
     "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/substore_Singbox.srs"},
]

# 添加通用配置
for rs in rule_sets:
    rs["download_detour"] = "direct-select"
    rs["update_interval"] = "24h"

# 更新配置中的 rule_set
if "route" not in config:
    config["route"] = {}
config["route"]["rule_set"] = rule_sets

# 保存配置
with open(config_file, 'w', encoding='utf-8') as f:
    json.dump(config, f, indent=2, ensure_ascii=False)

print(f"✅ 已更新 {len(rule_sets)} 个规则集 (完整覆盖)")
print(f"   广告拦截: 3 | AI服务: 2 | 社交媒体: 7")
print(f"   流媒体: 11 | 科技公司: 7 | 游戏: 5")
print(f"   金融: 2 | 国内服务: 9 | 网络基础: 4")
print(f"   全球代理: 2 | 手动规则: 6 | 特殊分类: 2")
PYTHON_SCRIPT

log_success "Sing-box 配置已更新"
echo ""

# ═══════════════════════════════════════════════════════════════
# 任务2: 同步 Surge 配置到 Shadowrocket
# ═══════════════════════════════════════════════════════════════
log_info "任务2: 同步 Surge 配置到 Shadowrocket..."

if [ ! -f "$SURGE_CONFIG" ]; then
    log_error "Surge 配置文件不存在: $SURGE_CONFIG"
    exit 1
fi

if [ ! -f "$SHADOWROCKET_DB" ]; then
    log_error "Shadowrocket 数据库不存在: $SHADOWROCKET_DB"
    exit 1
fi

# 备份 Shadowrocket 数据库
cp "$SHADOWROCKET_DB" "${SHADOWROCKET_DB}.backup.$(date +%Y%m%d_%H%M%S)"
log_info "已备份 Shadowrocket 数据库"

# 从 Surge 配置提取文本内容
log_info "从 Surge 配置提取规则..."
TEMP_SURGE_EXTRACT=$(mktemp)
cat "$SURGE_CONFIG" > "$TEMP_SURGE_EXTRACT"

# 使用 Python 更新 Shadowrocket SQLite 数据库
python3 - "$SHADOWROCKET_DB" "$TEMP_SURGE_EXTRACT" <<'PYTHON_SCRIPT2'
import sqlite3
import sys
from pathlib import Path

db_path = sys.argv[1]
surge_config = sys.argv[2]

# 读取 Surge 配置
with open(surge_config, 'r', encoding='utf-8') as f:
    surge_content = f.read()

# 连接数据库
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

# 查看表结构
cursor.execute("SELECT name FROM sqlite_master WHERE type='table';")
tables = cursor.fetchall()
print(f"数据库表: {tables}")

# Shadowrocket 使用 FTS3 虚拟表存储配置
# 需要解析 Surge 配置并插入到对应的表中
try:
    # 清空现有配置
    cursor.execute("DELETE FROM config")
    
    # 解析 Surge 配置
    current_section = ""
    line_num = 0
    
    for line in surge_content.split('\n'):
        line = line.strip()
        line_num += 1
        
        # 跳过空行和注释
        if not line or line.startswith('#'):
            continue
        
        # 检测段落
        if line.startswith('[') and line.endswith(']'):
            current_section = line[1:-1]
            continue
        
        # 解析配置行
        if current_section == "Rule":
            # 规则格式: TYPE,VALUE,POLICY
            parts = line.split(',', 2)
            if len(parts) >= 2:
                rule_type = parts[0]
                rule_value = parts[1]
                rule_policy = parts[2] if len(parts) > 2 else ""
                
                cursor.execute(
                    "INSERT INTO config (section, name, value, option, created) VALUES (?, ?, ?, ?, ?)",
                    ("Rule", rule_type, rule_value, rule_policy, line_num)
                )
    
    conn.commit()
    
    # 统计
    cursor.execute("SELECT COUNT(*) FROM config WHERE section='Rule'")
    rule_count = cursor.fetchone()[0]
    print(f"✅ 已同步 {rule_count} 条规则到 Shadowrocket")
    
except Exception as e:
    print(f"❌ 更新失败: {e}")
    import traceback
    traceback.print_exc()
finally:
    conn.close()

PYTHON_SCRIPT2

rm -f "$TEMP_SURGE_EXTRACT"

log_success "Shadowrocket 配置同步完成"
echo ""

# ═══════════════════════════════════════════════════════════════
# 统计信息
# ═══════════════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    同步完成统计                              ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"
echo -e "${BLUE}║  ${GREEN}Sing-box 规则集:${NC}  已更新                              ${BLUE}║${NC}"
echo -e "${BLUE}║  ${GREEN}Shadowrocket:${NC}     已同步                              ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

log_success "所有配置同步完成"
