# FlClash 预览版原生模块

[English](FLCLASH_PREVIEW.md) · [简体中文](FLCLASH_PREVIEW.zh-CN.md)

**专为带原生 MITM／模块管理功能的 FlClash 测试预览版研发**，目标设备版本为 `0.9.4+2026111517`。这里交付的是原生 `.sgmodule` 模块，不是订阅配置或配置扩展脚本；没有该功能入口的公开版本不能按此方式使用。

## 安装入口

- **PROMAX 去广告模块适配**：[ScriptHub.AdBlock.Preview.sgmodule](https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/flclash/head_expanse/github/ScriptHub.AdBlock.Preview.sgmodule)。首次导入和即时更新优先用此 GitHub 直连地址。
- **可选 `ScriptHub.PROMAX.*.sgmodule` 路由分片**：按源声明顺序，从[目录](../modules/flclash/catalog.json)选择各项 `github_install_url`。不含脚本或 CA 材料；域名／IP 规则无需解密，HTTPS URL 重写仍需应用自身的 CA 流程。
- **其他预览版模块与兼容报告**：[modules/flclash](../modules/flclash/)。应用增强合集按需单独选择，不要把全部模块一键全开。

在当前配置的**进阶配置 → MITM → 模块 → 添加**中选择 URL 导入；也可先下载，再从模块页按文件导入。使用原生模块页，不要导入到订阅／配置页。保留原配置供回滚；`FlClash-Surge.yaml` 继承 Surge 分流，不额外添加广告规则提供方。

带脚本的新模块会先禁用，需按应用提示审核脚本权限。导入后先查看**有效条目、忽略条目、资源下载和内核校验问题**，再决定启用；不能只凭“导入成功”判断生效，因为解析器将“全部被忽略”的模块也视为非空。脚本声明或内容变化后，应用可能再次禁用并要求重新审核，这是预览版自身的授权机制。

预览版保存无脚本路由模块时会默认启用。PROMAX 分片应按需逐个导入，不要一键导入整套；规则量大，已有主配置中的等价规则也可能重复。仓库发布更新不会替设备导入或启用模块。

HTTPS 响应修改还需要预览版自己的 HTTPS 解密开关和 CA 信任流程。本仓库不包含 CA／私钥，不安装证书、不替用户接受脚本权限。主去广告合集与重复的单应用去广告脚本不应同时安装，以免同一响应被处理多次。

保留源模块的参数定义、说明和已声明占位符，使用预览版原生参数编辑界面；默认禁用的任务也保持原意。启用前先核对参数值，未声明的占位符会明确报告，不静默发布。

## 适配范围

Rust 生成器读取包括 PROMAX 在内的当前公开 Surge 模块，生成独立的 FlClash Preview 产物。保留受支持的 URL／Header／Body Rewrite、Map Local、脚本和 MITM 主机范围；支持的内联路由规范化后保留。不兼容的节、参数或策略会进入兼容报告。

原生解析器不能加载远程 `RULE-SET`。源 PROMAX 显式声明的 23 份本仓库公开列表中，22 份生成独立可选路由分片；另 1 份只有 3 条不支持的 `USER-AGENT`，不发布空模块。分片只用 GitHub 直连地址，不复制 CDN 变体。目录记录源顺序、不支持的匹配器及丢失的 Surge 选项（包括 `pre`／`extended-matching`），不宣称完全等价。模板不会增加广告规则提供方。

发布保护上限为每片 8 MiB、总量 64 MiB，不代表设备容量或性能保证，尚未做设备压力测试。`[IP Rewrite]`、`[General]`、任意代理分组策略、其他远程规则集，以及无法表示的 jq／结构化 body／条件操作继续报告为遗漏。已有等价的内联防火墙／IP fallback 可以保留。

模块不会替换订阅、整个配置、系统 DNS、TUN 或全局 TLS／安全选项。原生模块路由只支持解析器接受的 DIRECT／REJECT 系列；这些已支持的规则仍会影响匹配流量。兼容报告列出来源和适配／遗漏数量，遗漏不代表语义等价。

## 持续更新

正常 Rust 更新在完成上游模块生成后重新制作预览版产物。只重生成此适配、不刷新全部远程来源时：

```sh
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin update -- --root /path/to/script_hub --execute --flclash-only
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin validate -- --root /path/to/script_hub
```

GitHub 直连变体不依赖 CDN 的分支缓存；设备上仍需执行模块自身的“更新”，必要时重新审核脚本权限。推送新文件不会绕过应用授权，也不会自动在现有设备上启用模块。

兼容契约固定为 FlClash dev [`5e73f3fb673a0012cc7c7312a52da33b76b78295`](https://github.com/chen08209/FlClash/tree/5e73f3fb673a0012cc7c7312a52da33b76b78295)：[模块解析器](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/common/mitm_module.dart)、[官方兼容测试](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/test/common/mitm_module_test.dart)、[原生新建模块模板](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/features/mitm/module_editor.dart)。适配器记录 parser/test 的 blob。私有预览内核可能有额外限制；静态兼容和仓库测试不等于所有上游脚本都已在该内核运行通过。

## 设备验收

1. 核对测试预览版和内核版本，确认原生 MITM／模块入口存在。
2. 先导入一个模块，核对有效／忽略条目、资源下载和内核问题；资源下载失败后使用旧缓存不算本次更新完成。
3. 需要时由用户按应用流程确认脚本权限和 CA 信任，不为消除错误而扩大主机范围或关闭证书校验。
4. 对启用的重写／脚本类型分别用合成请求响应验收，再检查日常应用与模块更新；以实际响应验证，不保证去掉所有广告。
5. 停用时在应用中禁用对应模块，保留旧主配置；全局证书信任、DNS 和 TUN 是独立设置。

普通路由规则与 sing-box 的边界见[客户端兼容性](COMPATIBILITY.zh-CN.md)。
