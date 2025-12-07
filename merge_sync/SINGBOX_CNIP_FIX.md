# Singbox cnip 规则集修复文档

## 问题描述

**错误信息**:
```
FATAL[0000] create service: initialize inbound[0]: parse route_exclude_address_set: rule-set not found: cnip
```

**根本原因**:
- Singbox配置文件在两处引用了`cnip`规则集：
  1. `inbounds[0].route_exclude_address_set = "cnip"` (第243行)
  2. `route.rules[].rule_set = "cnip"` (第1268行)
- 但在`route.rule_set`数组中没有定义`cnip`规则集

## 修复方案

### 1. 添加 cnip 规则集定义

在`substore/Singbox_substore_1.13.0+.json`的`route.rule_set`数组中添加：

```json
{
  "tag": "cnip",
  "type": "remote",
  "format": "binary",
  "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/ChinaIP_Singbox.srs",
  "download_detour": "direct-select",
  "update_interval": "24h"
}
```

**插入位置**: 在`surge-chinadirect`之后，`surge-globalproxy`之前（约第962行）

### 2. 更新同步脚本

修改`merge_sync/sync_all_configs.sh`，确保未来同步时包含`cnip`规则集：

```python
# 在 ChinaDirect 之后添加
{"tag": "cnip", "type": "remote", "format": "binary",
 "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/ChinaIP_Singbox.srs"},
```

## 验证修复

### 方法1: 使用验证脚本

```bash
./merge_sync/validate_singbox_config.sh
```

**预期输出**:
```
✅ cnip 规则集状态检查:
  定义: ✅ 已定义
  inbound引用: ✅ 已引用
  rules引用: ✅ 已引用

✅ cnip 规则集配置完整！
```

### 方法2: 手动验证

```bash
# 检查 cnip 定义
grep -A 5 '"tag": "cnip"' substore/Singbox_substore_1.13.0+.json

# 检查 cnip 引用
grep 'cnip' substore/Singbox_substore_1.13.0+.json
```

### 方法3: 启动 Singbox 测试

修复后，Singbox应该能够正常启动，不再报错。

## cnip 规则集说明

**用途**: 中国IP地址段规则集

**功能**:
1. **DNS防泄漏**: 在`route_exclude_address_set`中使用，排除中国IP地址段，防止DNS泄漏
2. **路由规则**: 在路由规则中使用，将中国IP流量直连

**数据来源**: 
- GitHub仓库: `nowaytouse/script_hub`
- 文件路径: `ruleset/SingBox/ChinaIP_Singbox.srs`
- 格式: Singbox二进制规则集 (SRS)

**更新频率**: 24小时

## 相关文件

- `substore/Singbox_substore_1.13.0+.json` - Singbox配置文件
- `merge_sync/sync_all_configs.sh` - 配置同步脚本
- `merge_sync/validate_singbox_config.sh` - 配置验证脚本
- `ruleset/SingBox/ChinaIP_Singbox.srs` - ChinaIP规则集文件

## 注意事项

1. **规则集标签**: 使用`cnip`而不是`surge-chinaip`，因为配置文件中引用的是`cnip`
2. **同时存在**: 配置中同时有`cnip`和`surge-chinaip`两个标签，都指向同一个ChinaIP规则集
3. **DNS防泄漏**: `cnip`主要用于`route_exclude_address_set`，这是DNS防泄漏的关键配置

## 修复日期

2025-12-07

## 修复人员

Kiro AI Assistant
