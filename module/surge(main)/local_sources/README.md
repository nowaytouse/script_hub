# Local Sources Directory

## 目录说明

此目录存放**已融入PROMAX模块**的源文件。

这些模块的内容已经完全合并到 `head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule` 中。

## 为什么保留这些文件?

1. **自动更新**: `scripts/adblock_manager.py` 脚本会读取这些源文件,自动更新PROMAX模块
2. **版本追踪**: 保留原始模块便于追踪上游更新
3. **避免重复**: 将源文件与用户可安装的模块分离,避免用户重复安装

## 目录内容

### 1. AllInOne_Mock.sgmodule
- **作者**: blackmatrix7
- **来源**: https://github.com/blackmatrix7/ios_rule_script
- **内容**: 745条REJECT规则 + 62条URL Rewrite + 23个Script
- **特点**: 覆盖大量应用的Map Local去广告

### 2. All-in-One-2.x.sgmodule
- **作者**: @hututu0 & 社区
- **来源**: https://tuu.cat/ADBlock
- **内容**: 知乎、高德、滴滴、小红书、微博、拼多多、京东、B站等
- **特点**: HTTPDNS拦截 + 小程序去广告 + 大量Body Rewrite规则

### 3. [Sukka] Enhance Better ADBlock for Surge.sgmodule
- **作者**: Sukka
- **内容**: Mock Google Analytics/Ads/DoubleClick等追踪脚本
- **特点**: 恢复网站正常功能,避免因拦截导致的页面错误

## 使用说明

### 用户
**请勿直接安装此目录下的模块!**

这些模块的功能已包含在PROMAX模块中,直接安装PROMAX即可:
```
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge(main)/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule
```

### 开发者
更新PROMAX模块:
```bash
python3 scripts/adblock_manager.py
```

脚本会自动:
1. 读取此目录下的源文件
2. 提取非Rule部分(URL Rewrite, Map Local, Script, Body Rewrite等)
3. 合并到PROMAX模块
4. 提取Rule部分到AdBlock.list

## 目录结构对比

```
module/surge(main)/
├── amplify_nexus/          # 功能增强模块 (用户可安装)
├── head_expanse/           # 广告拦截模块 (用户可安装)
│   ├── 🚫 PROMAX.sgmodule  # ← 合并后的最终模块
│   └── 🔥 Firewall.sgmodule
├── narrow_pierce/          # App专项去广告 (用户可安装)
└── local_sources/          # PROMAX源文件 (仅供脚本读取)
    ├── AllInOne_Mock.sgmodule
    ├── All-in-One-2.x.sgmodule
    └── [Sukka] Enhance Better ADBlock for Surge.sgmodule
```

## 更新日志

- **2026-04-28**: 创建local_sources目录,分离已融入PROMAX的源文件
- **2026-04-28**: 初始包含3个源模块
