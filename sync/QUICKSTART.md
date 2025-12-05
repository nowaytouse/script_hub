# 快速开始

## 🎯 最简单的方式

```bash
./sync_all_rules.sh
```

按Enter开始，脚本会自动：
1. ✅ 扫描并提取所有模块的规则
2. ✅ 去重合并到规则集
3. ✅ 转换SRS规则（Sing-box）
4. ✅ 询问是否同步到iCloud

## 📦 处理单个模块

```bash
./merge_adblock_modules.sh your_module.sgmodule
```

## 🗑️ 提取后删除模块

```bash
./merge_adblock_modules.sh -d old_module.sgmodule
```

## 🔍 交互式选择

```bash
./merge_adblock_modules.sh -i
```

会显示所有可用模块，输入序号选择（如：1 3 5）

## 📊 结果

规则会自动合并到：
- `ruleset/Surge(Shadowkroket)/AdBlock_Merged.list` (REJECT规则)
- `ruleset/Surge(Shadowkroket)/ChinaDirect.list` (DIRECT规则)
- `ruleset/SingBox/*.srs` (Sing-box规则)

## ⚡ 提示

- 所有规则自动去重
- 原文件会自动备份（.backup.日期时间）
- 支持Surge/Shadowrocket/Clash/Quantumult X/Loon格式
