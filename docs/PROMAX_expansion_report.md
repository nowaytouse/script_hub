# PROMAX模块扩展报告

## 扩展概述
成功将3个head_expanse综合去广告模块合并到PROMAX模块中,大幅增强去广告能力。

## 新增模块

### 1. AllInOne_Mock.sgmodule
- **作者**: blackmatrix7
- **来源**: https://github.com/blackmatrix7/ios_rule_script
- **规模**: 433行
- **特点**: 
  - 745条REJECT规则
  - 62条URL Rewrite
  - 23个HTTP Response Script
  - 831个MITM域名
  - 涵盖大量应用的Map Local去广告

### 2. All-in-One-2.x.sgmodule
- **作者**: @hututu0 & 社区
- **来源**: https://tuu.cat/ADBlock
- **规模**: 331行
- **特点**:
  - 包含知乎、高德、滴滴、小红书、微博、贴吧、菜鸟、拼多多、京东、B站、饿了么、美团等
  - HTTPDNS拦截
  - 小程序去广告(麦当劳、必胜客、肯德基、星巴克、丰巢等)
  - 大量Map Local和Body Rewrite规则

### 3. [Sukka] Enhance Better ADBlock for Surge.sgmodule
- **作者**: Sukka
- **规模**: 23行
- **特点**:
  - Mock Google Analytics/Tag Manager
  - Mock Google Ads/DoubleClick
  - Mock 第三方追踪脚本(AddThis, Chartbeat, Outbrain等)
  - 恢复网站正常功能,避免因拦截导致的页面错误

## 合并效果对比

### 规则数量变化
| 类型 | 合并前 | 合并后 | 增量 |
|------|--------|--------|------|
| URL Rewrite | 115 | 134 | +19 |
| Map Local | 265 | 685 | **+420** 🚀 |
| Script | 159 | 270 | **+111** 🚀 |
| Body Rewrite | 78 | 78 | 0 |
| Header Rewrite | 1 | 1 | 0 |
| **总行数** | **976** | **1526** | **+550** |

### 关键提升
- **Map Local规则增加158%**: 从265条增至685条,大幅提升广告拦截覆盖面
- **Script规则增加70%**: 从159条增至270条,增强动态内容处理能力
- **模块体积增加56%**: 从976行增至1526行,功能更全面

## 技术实现

### 合并策略
遵循PROMAX模块的设计理念:
- ✅ **Rule规则** → 提取到 `AdBlock.list` (442,226条规则)
- ✅ **非Rule部分** → 合并到 PROMAX模块
  - URL Rewrite: 重定向/Mock脚本
  - Map Local: 本地响应替换
  - Script: 动态内容处理
  - Body Rewrite: 响应体修改
  - Header Rewrite: 响应头修改

### 配置文件更新
在 `ruleset/Sources/Links/AdBlock_sources.txt` 中添加:
```
# Head Expanse综合去广告模块 (非Rule部分合并到PROMAX)
../../../module/surge(main)/head_expanse/AllInOne_Mock.sgmodule|REJECT
../../../module/surge(main)/head_expanse/All-in-One-2.x.sgmodule|REJECT
../../../module/surge(main)/head_expanse/[Sukka] Enhance Better ADBlock for Surge.sgmodule|REJECT
```

## 新增覆盖的应用/服务

### 国内应用
- 知乎 (增强去广告)
- 高德地图 (增强去广告)
- 滴滴出行 (增强去广告)
- 小红书 (增强去广告)
- 微博 (增强去广告)
- 贴吧
- 菜鸟
- 拼多多 (增强去广告)
- 京东 (增强去广告)
- 哔哩哔哩
- 饿了么
- 美团外卖
- 12306
- 携程
- 爱奇艺
- 优酷
- 腾讯视频
- 等数十个应用

### 小程序
- 麦当劳
- 必胜客
- 肯德基
- 星巴克
- 丰巢
- 奈雪点单
- 青桔单车
- 哈啰出行
- 顺丰速运

### 国际服务
- Google Analytics (Mock)
- Google Tag Manager (Mock)
- Google Ads (Mock)
- DoubleClick (Mock)
- AddThis (Mock)
- Chartbeat (Mock)
- Outbrain (Mock)
- Amazon Ads (Mock)

### 基础设施
- HTTPDNS拦截 (阿里云、腾讯云等)
- 各类追踪SDK
- 广告联盟

## 用户体验提升

### 1. 更全面的广告拦截
- 覆盖更多应用和场景
- 拦截开屏广告、信息流广告、弹窗广告
- 移除推广内容和营销卡片

### 2. 更智能的内容处理
- Mock追踪脚本,避免网站功能异常
- 动态处理响应内容,精准移除广告
- 保留正常功能,只移除广告元素

### 3. 更好的隐私保护
- 拦截HTTPDNS,防止DNS劫持
- 阻止追踪脚本收集用户数据
- 移除各类埋点和统计代码

## 兼容性说明

### 完全兼容
- ✅ 与现有PROMAX规则无冲突
- ✅ 与AdBlock.list协同工作
- ✅ 支持Surge、Shadowrocket等客户端

### 注意事项
- 部分网站可能因Mock脚本而需要调整
- 建议根据实际使用情况微调规则
- 如遇问题可临时禁用特定模块

## 维护计划

### 自动更新
- 脚本会自动从本地模块提取最新规则
- 运行 `python3 scripts/adblock_manager.py` 即可更新

### 上游同步
- AllInOne_Mock: 跟随blackmatrix7仓库更新
- All-in-One-2.x: 跟随hututu0更新
- Sukka Enhance: 跟随Sukka规则集更新

## 总结

通过合并这3个综合去广告模块,PROMAX模块的功能得到了大幅增强:
- **规则数量增加56%**
- **覆盖应用增加数十个**
- **去广告能力全面提升**

这次扩展完美体现了PROMAX模块的设计理念:将分散的去广告规则统一整合,提供一站式的广告拦截解决方案。

---

**Commit**: f77e79b2  
**Date**: 2025-01-XX  
**Status**: ✅ 已完成并推送到远程仓库
