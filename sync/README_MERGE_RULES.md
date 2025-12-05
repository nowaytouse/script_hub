# 规则同步脚本使用说明

## 🚀 一键执行（推荐）

```bash
./sync_all_rules.sh
```

**自动完成：**
1. 提取模块规则（REJECT + DIRECT）
2. 去重合并到规则集
3. 转换SRS规则（Sing-box）
4. 同步到iCloud（可选）

## 📋 单独使用合并脚本

```bash
# 自动扫描所有模块
./merge_adblock_modules.sh

# 指定模块
./merge_adblock_modules.sh module.sgmodule

# 交互式选择
./merge_adblock_modules.sh -i

# 自动删除已处理模块
./merge_adblock_modules.sh -d module.sgmodule
```

## 🎯 支持的格式

- Surge (.sgmodule, .conf)
- Shadowrocket (.module, .conf)
- Clash (.yaml, .yml)
- Quantumult X (.conf, .snippet)
- Loon (.plugin)

## 📊 规则提取

| 规则类型 | 目标文件 |
|---------|---------|
| REJECT | `AdBlock_Merged.list` |
| DIRECT | `ChinaDirect.list` |
| URL Rewrite | 保留在模块 |
| Host | 保留在模块 |

## ⚙️ 参数说明

| 参数 | 说明 |
|------|------|
| `-i` | 交互式选择模块 |
| `-l` | 仅更新规则列表 |
| `-d` | 自动删除已处理模块 |
| `-h` | 显示帮助 |

## 📝 使用示例

### 场景1：处理新下载的模块
```bash
./sync_all_rules.sh
# 选择模块 → 提取规则 → 转换SRS → 完成
```

### 场景2：批量处理并删除
```bash
./merge_adblock_modules.sh -d *.sgmodule
```

### 场景3：仅更新规则列表
```bash
./merge_adblock_modules.sh -l module.sgmodule
```
