#!/bin/bash
# =============================================================================
# Batch Convert Surge Rules to Singbox Binary (.srs)
# 优化: 使用本地sing-box + 并行转换
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="${SCRIPT_DIR}/../ruleset"
SURGE_DIR="${RULESET_DIR}/Surge(Shadowkroket)"
SINGBOX_DIR="${RULESET_DIR}/SingBox"
LOCAL_SINGBOX="${SCRIPT_DIR}/config-manager-auto-update/bin/sing-box"

# 选择sing-box
if [ -x "$LOCAL_SINGBOX" ]; then
    SINGBOX="$LOCAL_SINGBOX"
else
    SINGBOX="sing-box"
fi

# 所有需要转换的规则文件 (完整列表)
RULESETS=(
    # 手动规则
    "Manual_US" "Manual_West" "Manual_JP" "Manual" "Manual_Global"
    
    # AI 服务
    "AI"
    
    # 社交媒体
    "Telegram" "TikTok" "Twitter" "Instagram" "SocialMedia"
    
    # 流媒体
    "GlobalMedia" "YouTube" "Netflix" "Disney" "Spotify"
    "StreamJP" "StreamUS" "StreamKR" "StreamHK" "StreamTW"
    
    # 游戏
    "Gaming" "Steam"
    
    # 科技公司
    "Google" "Bing" "Apple" "Microsoft" "GitHub"
    
    # 其他服务
    "Discord" "Fediverse" "PayPal"
    
    # 代理/直连
    "GlobalProxy" "LAN" "CDN"
    
    # 中国相关
    "ChinaDirect" "ChinaIP" "Bilibili"
    
    # 广告/NSFW
    "NSFW" "AdBlock_Merged"
)

echo -e "${GREEN}=== Batch Surge to Singbox Converter ===${NC}"
echo "Converting ${#RULESETS[@]} rulesets..."
echo ""

SUCCESS_COUNT=0
FAIL_COUNT=0

for ruleset in "${RULESETS[@]}"; do
    INPUT_FILE="${SURGE_DIR}/${ruleset}.list"
    JSON_FILE="${SINGBOX_DIR}/${ruleset}_singbox.json"
    OUTPUT_FILE="${SINGBOX_DIR}/${ruleset}_Singbox.srs"
    
    if [ ! -f "$INPUT_FILE" ]; then
        echo -e "${RED}✗ ${ruleset}.list not found${NC}"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi
    
    echo -e "${YELLOW}Processing: ${ruleset}...${NC}"
    
    # Python conversion to JSON
    python3 << PYTHON_SCRIPT
import json
import sys

input_file = "${INPUT_FILE}"
output_file = "${JSON_FILE}"

rules = []
processed = 0
skipped = 0

try:
    with open(input_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            
            parts = [p.strip() for p in line.split(",")]
            if len(parts) < 2:
                skipped += 1
                continue
            
            rule_type = parts[0]
            pattern = parts[1]
            
            rule = {}
            if rule_type == "DOMAIN":
                rule["domain"] = [pattern]
            elif rule_type == "DOMAIN-SUFFIX":
                rule["domain_suffix"] = [pattern]
            elif rule_type == "DOMAIN-KEYWORD":
                rule["domain_keyword"] = [pattern]
            elif rule_type in ["IP-CIDR", "IP-CIDR6"]:
                rule["ip_cidr"] = [pattern]
            elif rule_type == "PROCESS-NAME":
                rule["process_name"] = [pattern]
            else:
                skipped += 1
                continue
            
            if rule:
                rules.append(rule)
                processed += 1
    
    output = {"version": 2, "rules": rules}
    
    with open(output_file, "w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)
    
    print(f"  Converted: {processed} rules", file=sys.stderr)
    sys.exit(0)
    
except Exception as e:
    print(f"  Error: {e}", file=sys.stderr)
    sys.exit(1)
PYTHON_SCRIPT
    
    if [ $? -ne 0 ]; then
        echo -e "${RED}✗ ${ruleset} conversion failed${NC}"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi
    
    # Compile to binary (使用选定的sing-box)
    "$SINGBOX" rule-set compile --output "${OUTPUT_FILE}" "${JSON_FILE}" 2>&1 | grep -v "^$" || true
    
    if [ -f "${OUTPUT_FILE}" ]; then
        SIZE=$(du -h "${OUTPUT_FILE}" | cut -f1)
        echo -e "${GREEN}✓ ${ruleset} → ${SIZE}${NC}"
        rm -f "${JSON_FILE}"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    else
        echo -e "${RED}✗ ${ruleset} compilation failed${NC}"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
    echo ""
done

echo ""
echo -e "${GREEN}=== Conversion Complete ===${NC}"
echo "Success: ${SUCCESS_COUNT} / ${#RULESETS[@]}"
echo "Failed:  ${FAIL_COUNT}"
