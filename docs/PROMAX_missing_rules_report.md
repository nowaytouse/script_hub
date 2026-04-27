# PROMAX模块缺失规则报告

## ✅ 问题已解决

### 问题概述
在将narrow_pierce模块合并到PROMAX模块时,发现**高德地图去广告模块**的Body Rewrite规则存在缺失。

### 根本原因
脚本`scripts/adblock_manager.py`在执行过程中调用了不存在的`Logger.debug()`方法,导致脚本在生成模块前崩溃,使得PROMAX模块生成不完整。

### 修复方案
1. 移除了脚本中所有`Logger.debug()`调用(Logger类只支持info/success/warn/error/section方法)
2. 重新运行`python3 scripts/adblock_manager.py`生成完整的PROMAX模块
3. 验证所有Body Rewrite规则已正确合并

### 验证结果
✅ **所有5条高德地图Body Rewrite规则现已包含在PROMAX模块中**:

1. **搜索业务营销结构 - commonMaterial** (Line 966)
```
http-response-jq ^https:\/\/m5\.amap\.com\/ws\/shield\/search_business\/process\/marketingOperationStructured\? 'delpaths([["data","commonMaterial"]])'
```

2. **搜索业务营销结构 - tipsOperationLocation** (Line 968)
```
http-response-jq ^https:\/\/m5\.amap\.com\/ws\/shield\/search_business\/process\/marketingOperationStructured\? 'delpaths([["data","tipsOperationLocation"]])'
```

3. **搜索业务营销结构 - resourcePlacement** (Line 967)
```
http-response-jq ^https:\/\/m5\.amap\.com\/ws\/shield\/search_business\/process\/marketingOperationStructured\? 'delpaths([["data","resourcePlacement"]])'
```

4. **搜索POI首页 - history_tags** (Line 969)
```
http-response-jq ^https:\/\/m5\.amap\.com\/ws\/shield\/search_poi\/homepage\? 'delpaths([["history_tags"]])'
```

5. **共享出行订单详情 - 车辆提示弹窗** (Line 965)
```
http-response-jq ^https:\/\/m5-zb\.amap\.com\/ws\/sharedtrip\/taxi\/order_detail_car_tips\? 'delpaths([["data","carTips","data","popupInfo"]])'
```

### 统计数据
- **Body Rewrite规则总数**: 78条
- **URL Rewrite规则**: 115条
- **Map Local规则**: 265条
- **Script规则**: 159条
- **Header Rewrite规则**: 1条

### 其他模块检查结果
- ✅ 拼多多去广告: 完整
- ✅ 淘宝去广告: 完整  
- ✅ 京东去广告: 完整
- ✅ 知乎去广告: 完整
- ✅ 小红书去广告: 完整
- ✅ 高德地图去广告: **已修复,完整**
- ✅ 滴滴出行去广告: 完整
- ✅ 闲鱼去广告: 完整

### 已提交更改
- Commit: `0660caf8`
- 已推送到: `origin/master`
- 修改文件:
  - `scripts/adblock_manager.py` (修复Logger调用)
  - `module/surge(main)/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule` (重新生成)
  - `ruleset/Surge(Shadowkroket)/AdBlock.list` (更新规则集)
  - `docs/PROMAX_missing_rules_report.md` (本报告)

## 结论
问题已在脚本层面彻底修复,避免了"治标不治本"的手动修补方案。所有应用去广告模块的Body Rewrite规则现已正确合并到PROMAX模块中。
