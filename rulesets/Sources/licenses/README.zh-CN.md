# 来源与许可

[English](README.md) · [简体中文](README.zh-CN.md)

| 来源 | 许可 |
| --- | --- |
| [app2smile/rules](https://github.com/app2smile/rules) | [MIT](app2smile-MIT.txt) |
| [AdGuard CNAME](https://github.com/AdguardTeam/cname-trackers) | [MIT](AdGuard-CNAME-MIT.txt) |
| [Steven Black — 仅作者原始 hosts](https://github.com/StevenBlack/hosts/tree/master/data/StevenBlack) | [MIT](StevenBlack-MIT.txt) |
| [MetaCubeX](https://github.com/MetaCubeX/meta-rules-dat) | [GPL-3.0](https://github.com/MetaCubeX/meta-rules-dat/blob/master/LICENSE) |
| [HaGeZi](https://github.com/hagezi/dns-blocklists) | [GPL-3.0](https://github.com/hagezi/dns-blocklists/blob/main/LICENSE) |
| [V2Fly 私有设备名称](https://github.com/v2fly/domain-list-community/blob/master/data/private) | [MIT](V2Fly-MIT.txt) |
| [Brave 第一方过滤器 — 仅已审核广告主机](https://github.com/brave/adblock-lists/blob/master/brave-lists/brave-firstparty.txt) | [MPL-2.0](https://github.com/brave/adblock-lists/blob/master/LICENSE) |
| [uBlock Origin uAssets — 已审核隐私主机及 Badware 网络子集](https://github.com/uBlockOrigin/uAssets/tree/master/filters) | [GPL-3.0](https://github.com/uBlockOrigin/uAssets/blob/master/LICENSE) |
| [Greek AdBlock Filter — 仅已审核广告主机](https://github.com/kargig/greek-adblockplus-filter/blob/master/void-gr-filters.txt) | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |

接入范围以[源清单](../Links/)为准。转换、去重、分类与安全隔离不改变原许可；App 参数保留普通视频帖，CNAME clickthrough / microsites 全集不启用。

## Brave 浏览器选项

[Brave 官方目录](https://github.com/brave/adblock-resources/blob/master/filter_lists/list_catalog.json)、[uBO 官方目录](https://github.com/gorhill/uBlock/blob/master/assets/assets.json)与 [AdGuard DNS 来源清单](https://github.com/AdguardTeam/AdGuardSDNSFilter#dns-filter-structure)列出各自产品的过滤能力，并非可以互换的代理策略。

| 已勾选选项 | 网络规则处理方式 |
| --- | --- |
| Brave 默认去广告、默认隐私和第一方规则 | 现有 AdGuard DNS 聚合源包含 EasyList 和 EasyPrivacy；清单新增已审核的 Brave 第一方广告主机及 uBO 隐私主机，uBO Badware 归入独立威胁类别。未整表导入 Brave/uBO 默认包或 Brave 第一方地区规则。 |
| 希腊；AdGuard 中文、荷兰语、法语、日语、俄语、西语／葡语、土耳其语；EasyList 德语、希伯来语、意大利语；ABPindo、List-KR、瑞典语、波斯语 | 希腊规则仅提取已审核广告主机。现有 DNS 聚合源已包含其他所列语言的相关组成项，不等同于浏览器规则全集。 |
| 阿拉伯语、保加利亚语、中文烦扰、捷克语／斯洛伐克语、爱沙尼亚语、芬兰语、印地语、匈牙利语、冰岛语、拉脱维亚语、立陶宛语、马其顿语、北欧语、波兰语、罗马尼亚语、RU AdList、斯洛文尼亚语、EasyList 西语、泰语、越南语 ABPVN | 未新增整表网络导入。浏览器订阅可能含路径、上下文例外、非广告类别或另有许可；波兰语源有非商业限制。 |
| X/Twitter 嵌入；Cookie 提示、烦扰、AI 建议、订阅弹窗、移动端推广、社交组件、聊天应用及实验过滤 | 保留在浏览器。嵌入例外不会转成拦截或 `DIRECT`；页面元素操作不会转成网络拒绝。 |
| YouTube Shorts、移动端推荐、自动配音、片尾元素、会员内容推广、移动端干扰元素；URL 跟踪保护 | 保留在浏览器。元素选择器、脚本片段与查询参数删除不转成主机拒绝规则。 |
| 付费墙过滤 | 未接入。 |

希腊规则归属 Greek AdBlock Filter 贡献者，见[原始清单](https://github.com/kargig/greek-adblockplus-filter/blob/master/void-gr-filters.txt)。生成内容经过筛选及格式转换，源衍生数据保留 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) 许可。

仅部分来源已核对，不保证未来维护状态。[完整声明](../../../THIRD_PARTY_NOTICES.zh-CN.md)
