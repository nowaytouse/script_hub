# FlClash 预览版原生模块

[English](FLCLASH_PREVIEW.md) · [简体中文](FLCLASH_PREVIEW.zh-CN.md)

**专为带原生 MITM／模块管理功能的 FlClash 测试预览版研发**，目标设备版本为 `0.9.4+2026111517`。这里交付的是原生 `.sgmodule` 模块，不是订阅配置或配置扩展脚本；没有该功能入口的公开版本不能按此方式使用。

## 安装入口

- [PROMAX 唯一安装入口](https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/flclash/head_expanse/github/ScriptHub.AdBlock.Preview.sgmodule)。模块约 31 MB，包含广告、隐私和防钓鱼域名、防火墙端口规则及精选的应用 HTTPS 处理规则。
- **其他预览版模块与兼容报告**：[modules/flclash](../modules/flclash/)。应用增强合集按需单独选择，不要把全部模块一键全开。

已安装 PROMAX 的话，用上面的 GitHub Raw 地址更新原条目。该模块超过 jsDelivr 对 GitHub 文件的默认单文件 20 MB 限制。域名和端口规则可以独立生效。要对 HTTPS 请求应用 URL Rewrite、Map Local 和 Body Rewrite，请在 FlClash 中开启 MITM 并按应用流程信任 CA。PROMAX 使用内联响应数据，无需授予脚本权限。 [jsDelivr 限制](https://www.jsdelivr.com/documentation#restrictions)

在当前配置的**进阶配置 → MITM → 模块 → 添加**中选择 URL 导入；也可先下载，再从模块页按文件导入。使用原生模块页，不要导入到订阅／配置页。保留原配置供回滚；`FlClash-Surge.yaml` 继承 Surge 分流，不额外添加广告规则提供方。

带脚本的新模块会先禁用，需按应用提示审核脚本权限。导入后先查看**有效条目、忽略条目、资源下载和内核校验问题**，再决定启用；不能只凭“导入成功”判断生效，因为解析器将“全部被忽略”的模块也视为非空。脚本声明或内容变化后，应用可能再次禁用并要求重新审核，这是预览版自身的授权机制。

如果已安装旧的 `ScriptHub.PROMAX.*.sgmodule` 路由分片，请在 FlClash 中停用或移除，再更新上面的主 PROMAX 模块。只保留一份 PROMAX，避免重复处理。仓库推送不会自动删除、替换或启用设备中已有的模块。

HTTPS 响应修改还需要预览版自己的 HTTPS 解密开关和 CA 信任流程。本仓库不包含 CA／私钥，不安装证书、不替用户接受脚本权限。主去广告合集与重复的单应用去广告脚本不应同时安装，以免同一响应被处理多次。

保留源模块的参数定义、说明和已声明占位符，使用预览版原生参数编辑界面；默认禁用的任务也保持原意。启用前先核对参数值，未声明的占位符会明确报告，不静默发布。

## 适配范围

### 217heidai 完整上游的本地导出

从[官方完整 DNS 发行版](https://github.com/217heidai/adblockfilters/releases/download/rule/adblockdns.txt)生成单个本地 PROMAX。导出器读取完整发行版并核对声明的条数，在新增规则中保留来源的放行语义，再应用共用服务保护与现有直连清单。导出沿用现有 PROMAX 和加载上限。

~~~sh
cargo run --locked --package rust_processor --bin promax -- --root . --export-heidai-full /path/outside/repository/ScriptHub.AdBlock.Heidai.Full.sgmodule
~~~

在原生模块页按文件导入，用于替换原 PROMAX 条目。再次导出并导入即可刷新。[来源目录](../rulesets/Sources/sgmodule/AdBlock_sources_manual.list)登记了各客户端的官方完整发行版，原生直订带有上游原始拦截，其中包含 Apple 服务域名。

Rust 生成器读取包括 PROMAX 在内的当前公开 Surge 模块，生成独立的 FlClash Preview 产物。保留受支持的 URL／Header／Body Rewrite、Map Local、脚本和 MITM 主机范围；支持的内联路由规范化后保留。不兼容的节、参数或策略会进入兼容报告。

PROMAX 使用 Local、Anti-AD、Privacy、完整的 Advertising 和 HaGeZi TIF Medium 分片，以及补充威胁列表中的合规精确域名和域名后缀。模块沿用共享的服务保护检查，并纳入已合并的防火墙端口规则。精选 URL Rewrite、内联 Map Local 和原生 Body Rewrite 限定在精确的 MITM 主机上。发布上限为 36 MiB 和 850,000 条有效条目。来源和省略项见[兼容目录](../modules/flclash/catalog.json)。

其他预览模块仍可使用受支持的 HTTPS 处理条目。`[IP Rewrite]`、`[General]`、任意代理分组策略、其他远程规则集，以及无法表示的 jq／结构化 body／条件操作继续报告为遗漏。

模块不会替换订阅、整个配置、系统 DNS、TUN 或全局 TLS／安全选项。原生模块路由只支持解析器接受的 DIRECT／REJECT 系列；这些已支持的规则仍会影响匹配流量。兼容报告列出来源和适配／遗漏数量，遗漏不代表语义等价。

## 持续更新

正常 Rust 更新在完成上游模块生成后重新制作预览版产物。只重生成此适配、不刷新全部远程来源时：

```sh
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin update -- --root /path/to/script_hub --execute --flclash-only
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin validate -- --root /path/to/script_hub
```

在应用内对原模块执行“更新”可下载新版本。其他模块如使用脚本，则按应用提示审核权限。

兼容契约固定为 FlClash dev [`5e73f3fb673a0012cc7c7312a52da33b76b78295`](https://github.com/chen08209/FlClash/tree/5e73f3fb673a0012cc7c7312a52da33b76b78295)：[模块解析器](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/common/mitm_module.dart)、[官方兼容测试](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/test/common/mitm_module_test.dart)、[原生新建模块模板](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/features/mitm/module_editor.dart)。适配器记录解析器和测试的 Git 对象。预览版使用私有内核，实际 HTTPS 处理效果需在该客户端验证。

## 设备验收

1. 核对测试预览版和内核版本，确认原生 MITM／模块入口存在。
2. 先导入一个模块，核对有效／忽略条目、资源下载和内核问题；资源下载失败后使用旧缓存不算本次更新完成。
3. 需要时由用户按应用流程确认脚本权限和 CA 信任，不为消除错误而扩大主机范围或关闭证书校验。
4. 对启用的重写／脚本类型分别用合成请求响应验收，再检查日常应用与模块更新；以实际响应验证，不保证去掉所有广告。
5. 停用时在应用中禁用对应模块，保留旧主配置；全局证书信任、DNS 和 TUN 是独立设置。

普通路由规则与 sing-box 的边界见[客户端兼容性](COMPATIBILITY.zh-CN.md)。
