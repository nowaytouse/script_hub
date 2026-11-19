# FlClash RS 与 Surge 共用模块

[English](FLCLASH_RS.md) · [简体中文](FLCLASH_RS.zh-CN.md)

在 Script Hub 的**模块 → Surge / FlClash**复制 Surge `.sgmodule` 链接，再从 FlClash RS 的 **MITM → 模块 → 添加**导入。启用前检查生效和忽略的条目，确认规则集已下载。需要 HTTPS 重写时，按客户端流程信任证书并审核脚本权限。

共用 PROMAX 模块小于 1 MB，其广告规则集单独加载。将原有 PROMAX 条目替换为共用链接，只启用一份 PROMAX。以后在 FlClash RS 中对模块执行**更新**即可刷新。[共用 Surge PROMAX](https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/surge/head_expanse/github/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule)

配置分流时使用 [FlClash YAML 模板](TEMPLATES.zh-CN.md)。需要在配置分流中使用 217heidai 完整源时，加载[跨平台导出](COMPATIBILITY.zh-CN.md#217heidai-完整源)中的 Mihomo 规则提供器。

[FlClash RS 构建](https://github.com/chen08209/FlClash/actions/runs/44287831009) · [规则集导入测试](https://github.com/chen08209/FlClash/blob/e06994ed6e15c6e80f8691cc4f933d34d8d60819/test/common/mitm_module_test.dart#L466-L525)
