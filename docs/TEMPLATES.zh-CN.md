# 配置模板使用指南

[English](TEMPLATES.md) · [简体中文](TEMPLATES.zh-CN.md)

## 解锁示例

打开[桌面应用](../crates/processor/README.zh-CN.md)，进入 **资源库 → 配置模板 → 打开模板库**，默认选中 **当前内置模板**。把下面整行粘贴到 **密码** 框，点击 **解锁**。查看示例不需要账号、令牌或订阅地址。

```text
tTUa!Acdb7!vhUpSUD1Rl9g*^bm@S#Xahrj1TK72KrA!TMty$lhh6zl1aIbOvAODzM#N2SxzG53iEgVRu571DNGU9rLWijO!0jZCDGPDQmLu%cmnC&QT9vOfqZ75V7*m
```

内置模板库以密文保存。公开密码只作为手动门槛，不提供保密或可靠的人机验证。不要用于私用库。

## 修改并使用

解锁后可选择 Surge、小火箭、FlClash 或 sing-box。各版本以同一份内置 Surge 配置的分流为基准，只使用各客户端支持的规则格式。GitHub 归入 `Microsoft_proxy`；已导出的配置如果还用退役链接，请重新导出。原始加密备份不再作为模板入口，私用库需要主动打开。已有文件不会被改写。

小火箭启用 IPv6，DIRECT DNS 使用阿里云和 DNSPod 的 DoH。Surge 和小火箭的直连测速使用 [微软 NCSI](https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/troubleshoot-ncsi-guidance)（HTTP 200），代理和测速组使用 [GrapheneOS HTTPS 探测端点](https://grapheneos.org/faq#default-connections)（204）。直连探测是明文 HTTP，两个服务都能看到各自探测的出口地址。

FlClash 和 sing-box 接入当前原生 AdBlock 规则提供方。不执行 Surge 模块或脚本，也不做 TLS 中间人解密；没有原生等价能力的规则不宣称覆盖。导出会按 App 内置目录校验本项目的规则集引用，不检查第三方地址的实时可用性。

1. 选择对应客户端的模板，勾选 **显示完整模板**，即可阅读原文。
2. 点击 **编辑模板 → 应用修改**，可自行调整策略组、规则顺序等；也可复制一份到自己的文本编辑器修改。若想在应用内保留改动，在项目可用时点 **保存加密模板库**。这只会写入本地副本，不会替换公开示例。切到其他应用前请先保存；自动锁定会丢弃未保存的改动。
3. 按客户端需要填写订阅地址，确认明文提示后，点 **复制配置**，或选仓库外的路径并点 **导出配置**。将结果作为新配置导入客户端，先保留旧配置，验证后再切换。

订阅地址只在复制或导出时填入，Script Hub 不会请求该地址。小火箭的订阅要单独添加。FlClash／Mihomo 模板需填写 Clash 兼容的订阅提供方槽位，再将导出的 `.yaml` 作为本地配置导入。sing-box 1.14 或更新版本使用导出的 `.json`；请在 `127.0.0.1:1080` 运行本地 SOCKS 代理，或替换 `script-hub-proxy` 出站并保留其标签。sing-box 模板不接受订阅 URL。

点 **复制配置** 或 **导出配置** 生成的 FlClash YAML 是客户端可直接导入的未加密配置，包含你填写的订阅地址。Script Hub 的密码输入框会遮蔽内容，磁盘上的模板库继续以密文保存。要以加密形式保留模板改动，请使用 **保存加密模板库**。

注释列出的可选模块需单独安装。不要把明文配置或订阅地址提交到 Git。
