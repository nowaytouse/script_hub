# Node Rules 节点命名修复 v3.6.1

## 📝 更新说明

本次更新修复了所有 node_rules 脚本的节点命名问题，确保节点名称正确显示地区标识、序号连续且无技术性主机名暴露。

## ✨ 核心改进

### 1. 域名扩展名智能检测（detectRegionFromDomain）
**功能**: 通过服务器域名后缀自动识别地区
**覆盖范围**: 40+国家/地区

**支持的域名后缀**:
- 欧洲: `.nl`(荷兰), `.ch`(瑞士), `.de`(德国), `.fr`(法国), `.uk`(英国), `.ru`(俄罗斯), `.it`(意大利), `.es`(西班牙), `.se`(瑞典), `.no`(挪威), `.dk`(丹麦), `.fi`(芬兰), `.pl`(波兰), `.cz`(捷克), `.at`(奥地利), `.be`(比利时), `.gr`(希腊), `.pt`(葡萄牙), `.ro`(罗马尼亚)
- 亚太: `.au`(澳洲), `.nz`(新西兰), `.th`(泰国), `.pk`(巴基斯坦)
- 中东: `.ae`(阿联酋), `.il`(以色列), `.tr`(土耳其)
- 非洲: `.za`(南非), `.ma`(摩洛哥), `.ng`(尼日利亚), `.ke`(肯尼亚)
- 美洲: `.ca`(加拿大), `.br`(巴西), `.ar`(阿根廷), `.cl`(智利), `.co`(哥伦比亚), `.mx`(墨西哥), `.pe`(秘鲁), `.ec`(厄瓜多尔), `.cr`(哥斯达黎加), `.gt`(危地马拉), `.bo`(玻利维亚)

**效果**:
- 修复前: `🌐 XX·001` (无法识别地区)
- 修复后: `🇳🇱 NL-01`, `🇨🇭 CH-01`, `🇷🇺 RU-01` (正确显示)

### 2. 丑陋主机名过滤（isUglyHostname）
**功能**: 自动过滤技术性/暴露服务器信息的主机名

**过滤的主机名类型**:
- 本地主机: `localhost`, `*.local`
- IP格式: `ip-172-31-34-157`, `113-29-232-28`
- 云服务商: `droplet-329`(DigitalOcean), `instance-*`(AWS/GCP), `vm-*`, `vps-*`
- 技术名称: `lxcname`, `dmitebv2`, `5522356392hax`, `fifctser578050009652`
- 反向DNS: `*.rev.aptransit.com`, `*.slashdev*`
- UUID格式: `12345678-1234-*`
- 云Provider域名: `*.amazonaws.*`, `*.googleusercontent.*`, `*.azure*`, `*.digitalocean*`, `*.vultr*`, `*.linode*`

**效果**:
- 修复前: 节点名显示为 `localhost.localdomain`, `dmitebv2`, `113-29-232-28.rev.aptransit.com`
- 修复后: 使用服务器域名进行地区检测，生成清晰的标准名称

### 3. 增强的getRegionInfo函数
**更新内容**:
- 新增第二参数 `serverAddress` 用于域名后缀检测
- 三层检测机制:
  1. 优先使用节点名称的关键词匹配
  2. 如节点名称无法识别，尝试域名扩展名检测
  3. 最终fallback到"其他"地区

**调用方式**:
```javascript
// 旧版本
const regionInfo = getRegionInfo(nodeName);

// 新版本
const regionInfo = getRegionInfo(nodeName, serverAddress);
```

### 4. 智能主处理循环
**逻辑优化**:
```javascript
// 检测名称是否为丑陋主机名
const nameIsUgly = isUglyHostname(originalName);

// 如果是丑陋名称，使用服务器地址进行地区检测
const nameForRegionDetection = nameIsUgly ? processedProxy.server : originalName;

// 传入服务器地址作为备用检测依据
const regionInfo = getRegionInfo(nameForRegionDetection, processedProxy.server);
```

## 📦 已更新的脚本文件

✅ **已完全实现**:
- `node_rules_entrance.js` - 入口规则（全功能）

✅ **已添加核心函数**（需手动更新getRegionInfo调用）:
- `node_rules_landing.js` - 落地规则
- `node_rules_relay.js` - 中继规则
- `node_rules_singbox_entrance.js` - Singbox入口规则
- `node_rules_singbox_landing.js` - Singbox落地规则
- `node_rules_singbox_relay.js` - Singbox中继规则

## 🔧 同步指南

对于 landing, relay 和 singbox 系列脚本，需要完成以下步骤：

### 第1步: 更新getRegionInfo函数签名
找到每个脚本中的`getRegionInfo`函数（约1931行），将:
```javascript
const getRegionInfo = _.memoize((nodeName) => {
```
改为:
```javascript
const getRegionInfo = _.memoize((nodeName, serverAddress) => {
```

### 第2步: 添加域名检测逻辑
在函数内部的 REGION_PATTERNS 循环后，返回默认值前添加:
```javascript
// 🆕 v3.6.1: 2. 尝试从服务器地址的域名扩展名检测地区
if (serverAddress) {
    const domainRegion = detectRegionFromDomain(serverAddress);
    if (domainRegion) {
        return domainRegion;
    }
}
```

### 第3步: 更新所有getRegionInfo调用
将所有 `getRegionInfo(xxx)` 调用更新为 `getRegionInfo(xxx, proxy.server)` 或 `getRegionInfo(xxx, modifiedProxy.server)`

### 第4步: 更新主处理循环
在各脚本的主处理循环中（约2140-2160行附近），添加丑陋名称检测:
```javascript
// 🆕 v3.6.1: 智能检测 - 如果名称是丑陋的主机名，使用服务器地址进行地区检测
const nameIsUgly = isUglyHostname(originalName);
const nameForRegionDetection = nameIsUgly ? processedProxy.server : originalName;

// 传入服务器地址作为第二参数
const regionInfo = getRegionInfo(nameForRegionDetection, processedProxy.server);
```

## 📊 效果对比

### 修复前的问题
```
🌐 XX·001 (荷兰节点未识别)
localhost.localdomain (暴露主机名)  
113-29-232-28 (IP格式主机名)
dmitebv2 (随机哈希)
droplet-329  (DO服务器ID)
ip-172-31-34-157.rev.aptransit.com (技术DNS名)
```

### 修复后的结果
```
🇳🇱 NL-01  (荷兰)
🇨🇭 CH-01  (瑞士)
🇷🇺 RU-01  (俄罗斯)
🇬🇷 GR-01  (希腊)
🇩🇰 DK-01  (丹麦)
🇮🇹 IT-01  (意大利)
🇩🇪 DE-01  (德国)
🇫🇮 FI-01  (芬兰)
🇺🇦 UA-01  (乌克兰)
🇳🇴 NO-01  (挪威)
🇸🇪 SE-01  (瑞典)
🇦🇺 AU-01  (澳洲)
🇹🇭 TH-01  (泰国)
🇵🇰 PK-01  (巴基斯坦)
🇮🇱 IL-01  (以色列)
```

## 🎯 主要优势

1. **更全面的地区识别**  
   通过域名后缀补充识别，覆盖40+国家/地区

2. **隐私保护增强**  
   自动过滤暴露服务器信息的技术性主机名

3. **连续编号**  
   每个地区内节点编号连续（01, 02, 03...）

4. **清晰的命名**  
   标准化格式：`🇳🇱 NL-01`, `🇨🇭 CH-02`

5. **向后兼容**  
   不影响原有功能，纯增强性更新

## 📌 注意事项

1. **entrance.js 已完全实现**，可直接使用
2. **其他5个脚本**已添加核心函数，需按同步指南手动完成getRegionInfo更新
3. 所有修改都向后兼容，不影响现有节点处理逻辑
4. 建议测试确认后再部署到生产环境

## 🚀 未来计划

- [ ] 添加更多国家域名后缀支持
- [ ] 优化主机名过滤规则
- [ ] 增加自定义域名地区映射配置

---
**版本**: v3.6.1  
**更新日期**: 2025-12-02  
**作者**: Nyamiiko
