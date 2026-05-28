# AdBlock 分片规则集

> 自动生成于 `2026-05-28 15:56:50` · 每片 ≤ 100,000 条 · 详见 [`catalog.json`](catalog.json)

## 用途分类

| 分类 | 说明 |
|------|------|
| **应用定制** (`Local`) | LocalModules / local_sources / 仓库内 App 专项规则 |
| **通用广告** (`Advertising`) | blackmatrix7 / ACL4SSR / geekdada / 广告联盟等通用域名 |
| **隐私追踪** (`Privacy`) | Privacy / tracking-protection / 防追踪列表 |
| **安全威胁** (`Security`) | 劫持 / 钓鱼 / BanProgram / HTTPDNS / skk reject |
| **Anti-AD** (`AntiAD`) | privacy-protection-tools anti-AD 主列表与 discretion |
| **威胁情报·终极** (`ThreatIntel_Ultimate`) | HaGeZi DNS-Blocklists ultimate |
| **威胁情报·TIF** (`ThreatIntel_TIF`) | HaGeZi TIF medium 威胁情报 |
| **威胁情报·补充** (`ThreatIntel`) | HaGeZi / skk 其它威胁与钓鱼域名集 |
| **其他补充** (`Other`) | 社区 sgmodule / 未归类远程源 |

## 当前分片

| 用途 | 文件 | 规则数 | CDN |
|------|------|--------|-----|
| 应用定制 | `ruleset/AdBlock/AdBlock_Local.list` | 125 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Local.list) |
| 通用广告 | `ruleset/AdBlock/AdBlock_Advertising_01.list` | 100,000 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Advertising_01.list) |
| 通用广告 | `ruleset/AdBlock/AdBlock_Advertising_02.list` | 100,000 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Advertising_02.list) |
| 通用广告 | `ruleset/AdBlock/AdBlock_Advertising_03.list` | 78,738 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Advertising_03.list) |
| 隐私追踪 | `ruleset/AdBlock/AdBlock_Privacy.list` | 5,617 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Privacy.list) |
| 安全威胁 | `ruleset/AdBlock/AdBlock_Security.list` | 26,338 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Security.list) |
| Anti-AD | `ruleset/AdBlock/AdBlock_AntiAD.list` | 4,996 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_AntiAD.list) |
| 其他补充 | `ruleset/AdBlock/AdBlock_Other.list` | 3,784 | [link](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/ruleset/AdBlock/AdBlock_Other.list) |

## 安装模块（引用上述分片，勿重复安装 local_sources）

- **Surge PROMAX**: [https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/module/surge%28main%29/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule](https://fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/module/surge%28main%29/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule)
- **Shadowrocket PROMAX**: [https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.module](https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.module)
