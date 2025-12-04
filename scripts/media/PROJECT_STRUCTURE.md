# Script Hub 项目结构说明 (2025-12-04更新)

## 📁 新的文件夹组织结构

为了更好的组织和维护，所有脚本已重新分类到以下目录：

### 📷 scripts/media/ - 媒体处理脚本
专注于图片、视频、动画等媒体文件的格式转换和处理：
- `convert_incompatible_media.sh` - 批量转换不兼容媒体格式（HEIC→PNG, MP4→GIF/WebP）
- `heic_to_png.sh` - HEIC/HEIF 图片转 PNG（无损）
- `imganim_to_vvc.sh` - 动画图片转 H.266/VVC 视频
- `jpeg_to_jxl.sh` - JPEG 转 JXL 格式（高质量压缩）
- `png_to_jxl.sh` - PNG 转 JXL 格式（数学无损）
- `merge_xmp.sh` - XMP 元数据合并到媒体文件
- `media_health_check.sh` - 媒体文件健康检查模块
- `video_to_hq_gif.sh` - 视频转高质量 GIF

### 🌐 scripts/network/ - 网络配置脚本
用于代理配置、规则转换等网络相关操作：
- `batch_convert_to_singbox.sh` - 批量转换规则到 Sing-box 格式
- `convert_to_module.sh` - 配置转换为模块格式
- `ruleset_merger.sh` - 规则集合并工具

### 🛠️ scripts/utils/ - 通用工具脚本
其他实用工具类脚本：
- `archive_500MB.sh` - 500MB 分卷压缩归档工具

## 📝 Substore 目录

包含 Sub-Store 订阅管理工具的高级规则脚本：

### 🔐 节点优化脚本（Node Rules）
6个JavaScript脚本用于不同场景的节点优化：
- `node_rules_entrance.js` - 入口节点优化（v3.6.0）
- `node_rules_landing.js` - 落地节点优化（v3.6.0）
- `node_rules_relay.js` - 中继节点优化（v3.6.0）
- `node_rules_singbox_entrance.js` - Sing-box 入口节点优化
- `node_rules_singbox_landing.js` - Sing-box 落地节点优化
- `node_rules_singbox_relay.js` - Sing-box 中继节点优化

**✅ 最新更新 (2025-12-04)**:
所有脚本已更正关于 `curve_preferences` 的注释。Sing-box 1.13.0+ **完全支持** `curve_preferences` 字段（自1.13.0-alpha.16引入），用于配置椭圆曲线偏好以增强TLS安全性。

**核心特性**:
- 🎭 Chrome 131 完整浏览器指纹伪装
- 🔒 TLS 1.3 独占模式（所有协议强制）
- 🛡️ AES-GCM 专用加密（智能ChaCha20场景支持）
- 🌍 智能地区CDN映射（6大提供商）
- 🔐 Reality 7层保护 + XTLS 多重检测
- 🚫 QUIC 智能屏蔽（保护原生QUIC协议）

### 📋 Sing-box 配置文件
- `Singbox_substore_1.13.0+.json` - Sing-box 1.13.0+ 完整配置
  - ✅ 使用 `curve_preferences` 字段配置椭圆曲线
  - ✅ DNS over H3 配置（TLS 1.3）
  - ✅ 完整的 DNS 泄漏防护
  - ✅ Binary ruleset 格式支持

## 🔍 兼容性验证 (基于官方文档)

### Sing-box 1.13.0+ 配置兼容性
- ✅ `curve_preferences` - 完全支持（P256, P384, P521, X25519, X25519MLKEM768）
- ✅ `min_version` / `max_version` - TLS版本控制
- ✅ `alpn` - 应用层协议协商
- ✅ Binary ruleset - `.srs` 格式规则集
- ✅ uTLS 指纹伪装 - Chrome/Firefox/Safari等

**官方文档参考**:
- TLS配置: https://sing-box.sagernet.org/configuration/shared/tls/
- curve_preferences: 自 sing-box 1.13.0-alpha.16 引入

## 📚 使用指南

### 媒体处理脚本使用示例
```bash
# 转换整个文件夹的HEIC为PNG（无损）
./scripts/media/heic_to_png.sh --in-place ~/Pictures/iPhone

# 批量JPEG转JXL（高质量压缩）
./scripts/media/jpeg_to_jxl.sh ~/Photos/JPEG

# 检查媒体文件健康状态
./scripts/media/media_health_check.sh ~/Videos/
```

### 网络配置脚本使用示例
```bash
# 批量转换规则到Sing-box格式
./scripts/network/batch_convert_to_singbox.sh ~/rules/

# 合并多个规则集
./scripts/network/ruleset_merger.sh rule1.list rule2.list
```

## ⚙️ 配置最佳实践

### Sing-box TLS安全配置
```json
{
  "tls": {
    "enabled": true,
    "min_version": "1.3",
    "max_version": "1.3",
    "curve_preferences": ["x25519", "p256", "p384"],
    "alpn": ["h2", "http/1.1"],
    "utls": {
      "enabled": true,
      "fingerprint": "chrome"
    }
  }
}
```

### Node Rules脚本配置
脚本内置智能优化，默认配置已经很好，但您可以通过修改脚本头部的`cfg`对象来自定义：
- `enableBoost: true/false` - 启用/禁用性能增强
- `blockQuic: true/false` - 启用/禁用QUIC屏蔽
- `forceTls: true/false` - 强制TLS加密

## 🔄 更新日志

### 2025-12-04
- ✨ 重组scripts文件夹为media/network/utils分类
- ✅ 更正所有JS文件中关于curve_preferences的过时注释
- ✅ 验证Sing-box 1.13.0+配置兼容性（基于官方文档）
- ✅ 所有脚本通过语法检查（shellcheck, node --check, jq）
- 📝 更新项目文档说明新的文件夹结构

## 🎯 项目原则

- **完整元数据保留**: 所有脚本全力保留内部（EXIF, XMP）和系统元数据（时间戳）
- **健康检查验证**: 转换脚本验证输出文件可查看/播放后才删除原文件
- **白名单处理**: 仅处理指定格式，忽略其他文件以保证安全
- **英文输出**: 所有脚本使用英文+Emoji输出，国际兼容性好
- **安全优先**: 危险操作需显式标志（`--in-place`, `--delete-source`）
- **有据可查**: 所有配置兼容性验证都基于官方文档，确保准确性
