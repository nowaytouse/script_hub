# PROMAX模块缺失规则报告

## 问题概述
在将narrow_pierce模块合并到PROMAX模块时，发现**高德地图去广告模块**的Body Rewrite规则存在缺失。

## 详细分析

### 高德地图去广告模块
**缺失规则数**: 5条

#### 缺失的Body Rewrite规则：

1. **搜索业务营销结构 - commonMaterial**
```
http-response-jq ^https://m5.amap.com/ws/shield/search_business/process/marketingOperationStructured? 'delpaths([["data","commonMaterial"]])'
```

2. **搜索业务营销结构 - tipsOperationLocation**
```
http-response-jq ^https://m5.amap.com/ws/shield/search_business/process/marketingOperationStructured? 'delpaths([["data","tipsOperationLocation"]])'
```

3. **搜索业务营销结构 - resourcePlacement**
```
http-response-jq ^https://m5.amap.com/ws/shield/search_business/process/marketingOperationStructured? 'delpaths([["data","resourcePlacement"]])'
```

4. **搜索POI首页 - history_tags**
```
http-response-jq ^https://m5.amap.com/ws/shield/search_poi/homepage? 'delpaths([["history_tags"]])'
```

5. **共享出行订单详情 - 车辆提示弹窗**
```
http-response-jq ^https://m5-zb.amap.com/ws/sharedtrip/taxi/order_detail_car_tips? 'delpaths([["data","carTips","data","popupInfo"]])'
```

## 影响范围
这些缺失的规则会导致高德地图App中：
- 搜索结果页面的营销内容无法被移除
- 首页历史标签广告仍然显示
- 打车订单详情页的推广弹窗无法屏蔽

## 修复建议
需要将这5条Body Rewrite规则添加到PROMAX模块的`[Body Rewrite]`部分。

## 其他模块检查结果
- ✅ 拼多多去广告: 完整
- ✅ 淘宝去广告: 完整  
- ✅ 京东去广告: 完整
- ✅ 知乎去广告: 完整
- ✅ 小红书去广告: 完整
- ❌ 高德地图去广告: 缺失5条规则
- ✅ 滴滴出行去广告: 完整
