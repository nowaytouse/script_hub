#!/opt/homebrew/bin/bash
# =============================================================================
# Surge 配置文件规则同步脚本
# 功能: 将完整规则集同步到 Surge 配置文件
# 更新: 2025-12-07
# 
# 配置文件结构:
#   [General] ... [Proxy] ... [Proxy Group] ...
#   [Rule]
#   # ============ 以上为新增 ============  <-- 用户手动添加的规则在此标记之前
#   # ============ 去广告规则 ============  <-- 自动同步的规则从这里开始
#   ...
#   [Host] ... [MITM] ... [WireGuard] ...
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# 配置文件路径
SURGE_ICLOUD_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents"
TEMPLATE_FILE="${PROJECT_ROOT}/ruleset/Sources/surge_rules_complete.conf"
TARGET_FILE="${SURGE_ICLOUD_DIR}/NyaMiiKo Pro Max plus👑_fixed.conf"

# 标记
USER_RULES_MARKER="# ============ 以上为新增 ============"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Surge 配置文件规则同步工具                             ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ============================================================
# 第0部分: 先吸取用户新增的规则 (注释关键词智能分类)
# ============================================================
INGEST_SCRIPT="${SCRIPT_DIR}/ingest_from_surge.sh"
if [ -f "$INGEST_SCRIPT" ]; then
    log_info "检查用户新增规则 (注释关键词智能分类)..."
    if bash "$INGEST_SCRIPT" --execute --no-backup 2>/dev/null; then
        log_success "用户规则已吸取并分类"
    else
        log_info "没有新增规则需要吸取"
    fi
    echo ""
fi

# 检查文件
if [ ! -f "$TEMPLATE_FILE" ]; then
    log_error "模板文件不存在: $TEMPLATE_FILE"
    exit 1
fi

if [ ! -f "$TARGET_FILE" ]; then
    log_error "目标配置文件不存在: $TARGET_FILE"
    exit 1
fi

# 备份原文件
BACKUP_FILE="${TARGET_FILE}.backup.$(date +%Y%m%d_%H%M%S)"
cat "$TARGET_FILE" > "$BACKUP_FILE"
log_info "已备份原文件"

# 创建临时文件
TEMP_FILE=$(mktemp)
trap "rm -f $TEMP_FILE" EXIT

# ============================================================
# 第1部分: 提取 [Rule] 之前的所有内容 (包括 [General], [Proxy], [Proxy Group] 等)
# ============================================================
log_info "提取配置文件头部 (到 [Rule] 之前)..."
sed -n '1,/^\[Rule\]/p' "$TARGET_FILE" > "$TEMP_FILE"

# ============================================================
# 第2部分: 提取用户手动添加的规则 ([Rule] 到标记之间)
# ============================================================
log_info "提取用户手动添加的规则..."
# 找到 [Rule] 行号
RULE_LINE=$(grep -n "^\[Rule\]" "$TARGET_FILE" | head -1 | cut -d: -f1)
# 找到标记行号
MARKER_LINE=$(grep -n "以上为新增" "$TARGET_FILE" | head -1 | cut -d: -f1)

if [ -n "$MARKER_LINE" ] && [ "$MARKER_LINE" -gt "$RULE_LINE" ]; then
    # 提取 [Rule] 到标记之间的用户规则 (不包括 [Rule] 行本身)
    USER_RULES_START=$((RULE_LINE + 1))
    USER_RULES_END=$((MARKER_LINE))
    sed -n "${USER_RULES_START},${USER_RULES_END}p" "$TARGET_FILE" >> "$TEMP_FILE"
    log_info "  用户规则行数: $((USER_RULES_END - USER_RULES_START + 1))"
else
    # 没有标记，添加空的用户规则区域
    echo "" >> "$TEMP_FILE"
    echo "$USER_RULES_MARKER" >> "$TEMP_FILE"
    log_warning "  未找到用户规则标记，已添加"
fi

# ============================================================
# 第3部分: 添加自动同步的规则 (从模板文件)
# ============================================================
log_info "添加自动同步的规则..."
echo "" >> "$TEMP_FILE"
# 跳过模板文件中的 [Rule] 行（如果有的话）
grep -v "^\[Rule\]" "$TEMPLATE_FILE" >> "$TEMP_FILE"

# ============================================================
# 第4部分: 提取 [Host] 及之后的所有内容
# ============================================================
log_info "提取配置文件尾部 ([Host] 及之后)..."
echo "" >> "$TEMP_FILE"
sed -n '/^\[Host\]/,$p' "$TARGET_FILE" >> "$TEMP_FILE"

# ============================================================
# 写入最终文件
# ============================================================
cat "$TEMP_FILE" > "$TARGET_FILE"

# 统计
RULE_COUNT=$(grep -c "^RULE-SET" "$TARGET_FILE" 2>/dev/null || echo "0")
GEOIP_COUNT=$(grep -c "^GEOIP" "$TARGET_FILE" 2>/dev/null || echo "0")
DOMAIN_COUNT=$(grep -c "^DOMAIN" "$TARGET_FILE" 2>/dev/null || echo "0")

echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    同步完成统计                              ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"
printf "${BLUE}║  ${GREEN}RULE-SET 规则:${NC}  %-5s                                   ${BLUE}║${NC}\n" "$RULE_COUNT"
printf "${BLUE}║  ${GREEN}GEOIP 规则:${NC}     %-5s                                   ${BLUE}║${NC}\n" "$GEOIP_COUNT"
printf "${BLUE}║  ${GREEN}DOMAIN 规则:${NC}    %-5s                                   ${BLUE}║${NC}\n" "$DOMAIN_COUNT"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

log_success "配置文件已更新"
echo ""
log_warning "请在 Surge 中重新加载配置文件以生效"
